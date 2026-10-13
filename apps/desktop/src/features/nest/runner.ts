import type { LaserApi } from '@/api';
import { errorMessage } from '@/lib/errors';
import type { NestRequest, NestStatus } from './types';

/** Cada cuánto se pregunta al motor por la mejor solución. */
export const NEST_POLL_MS = 250;
/** Intentos (y pausa) para esperar el último estado tras pedir que pare. */
const STOP_TRIES = 8;
const STOP_WAIT_MS = 40;

export type NestPhase = 'idle' | 'running' | 'stopping' | 'finished';

export interface NestRunState {
  phase: NestPhase;
  /** Último estado recibido (la mejor solución hasta ahora); `null` antes de empezar. */
  status: NestStatus | null;
  /** Mensaje del rechazo de `nestStart`, de un fallo al sondear o de `status.error`. */
  error: string | null;
}

export const IDLE_NEST_RUN: NestRunState = { phase: 'idle', status: null, error: null };

type NestApi = Pick<LaserApi, 'nestStart' | 'nestPoll' | 'nestCancel'>;

export interface NestRunner {
  get: () => NestRunState;
  /** Arranca un cálculo nuevo (cancela el anterior). */
  start: (request: NestRequest) => Promise<void>;
  /** Pide parar y espera el último estado; lo devuelve (o el que ya había si no corría nada). */
  stop: () => Promise<NestStatus | null>;
  /** Olvida el resultado (no hace nada si algo corre). */
  reset: () => void;
  /** Deja de escuchar y cancela en el motor lo que siga corriendo. */
  dispose: () => void;
}

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/**
 * Ciclo de vida de un acomodo sin depender de React: `nestStart`, sondeo con `setTimeout` mientras
 * `running` y `nestCancel` al parar o al descartar. Un estado que llega de un trabajo ya
 * reemplazado se descarta (contador de generación).
 */
export function createNestRunner(opts: { api: () => NestApi; onChange: (s: NestRunState) => void; pollMs?: number }): NestRunner {
  const pollMs = opts.pollMs ?? NEST_POLL_MS;
  let state = IDLE_NEST_RUN;
  let job: string | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let generation = 0;
  let alive = true;
  /** `nestStart` pedido y todavía sin respuesta. */
  let pending = false;

  const set = (patch: Partial<NestRunState>) => {
    state = { ...state, ...patch };
    opts.onChange(state);
  };
  const clearTimer = () => {
    if (timer) clearTimeout(timer);
    timer = null;
  };
  const current = (gen: number) => alive && gen === generation;
  const cancelQuietly = (id: string) => void opts.api().nestCancel({ job_id: id }).catch(() => undefined);
  const receive = (s: NestStatus) => set({ status: s, error: s.error });

  /** Sondea hasta que el motor deje de correr. */
  const loop = (id: string, gen: number) => {
    timer = setTimeout(async () => {
      try {
        const s = await opts.api().nestPoll({ job_id: id });
        if (!current(gen)) return;
        receive(s);
        if (s.running) loop(id, gen);
        else set({ phase: 'finished' });
      } catch (e) {
        if (!current(gen)) return;
        set({ error: errorMessage(e), phase: 'finished' });
      }
    }, pollMs);
  };

  return {
    get: () => state,

    async start(request) {
      clearTimer();
      const gen = ++generation;
      job = null;
      pending = true;
      set({ phase: 'running', status: null, error: null });
      try {
        const id = await opts.api().nestStart({ request });
        if (!current(gen)) {
          // Se pidió parar (o cerrar) mientras arrancaba: ese trabajo ya nadie lo va a leer.
          cancelQuietly(id);
          return;
        }
        pending = false;
        job = id;
        loop(id, gen);
      } catch (e) {
        if (!current(gen)) return;
        pending = false;
        set({ error: errorMessage(e), phase: 'idle' });
      }
    },

    async stop() {
      const id = job;
      if (!id) {
        if (pending) {
          generation++;
          pending = false;
          set({ phase: 'idle' });
        }
        return state.status;
      }
      if (state.status && !state.status.running) return state.status;
      clearTimer();
      const gen = ++generation;
      set({ phase: 'stopping' });
      try {
        await opts.api().nestCancel({ job_id: id });
        let s: NestStatus | null = null;
        for (let i = 0; i < STOP_TRIES; i++) {
          s = await opts.api().nestPoll({ job_id: id });
          if (!s.running) break;
          await sleep(STOP_WAIT_MS);
        }
        if (s && current(gen)) receive(s);
      } catch (e) {
        if (current(gen)) set({ error: errorMessage(e) });
      }
      if (current(gen)) set({ phase: 'finished' });
      return state.status;
    },

    reset() {
      if (pending || state.phase === 'running' || state.phase === 'stopping') return;
      generation++;
      clearTimer();
      job = null;
      if (state !== IDLE_NEST_RUN) {
        state = IDLE_NEST_RUN;
        opts.onChange(state);
      }
    },

    dispose() {
      alive = false;
      generation++;
      clearTimer();
      if (job && (state.status?.running ?? true)) cancelQuietly(job);
    },
  };
}
