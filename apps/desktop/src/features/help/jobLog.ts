import { create } from 'zustand';
import { readPref, writePref } from '@/lib/platform';

/**
 * Registro de trabajos (M51): lista local de lo que se envió a una máquina, con fecha, máquina,
 * tiempo estimado y real y resultado. Vive en `localStorage` (clave `tlaku.jobLog`): es un dato
 * de esta computadora, no del proyecto, y las ventanas de la app lo comparten. Lo alimenta el
 * slice de la máquina con los eventos `machine://job` que ya existen (ver `noteJobEvent`).
 */

export type JobResult = 'finished' | 'cancelled' | 'error';

/**
 * Un trabajo que no terminó y del que el motor guardó un punto de control («continuar un trabajo
 * interrumpido», `store/machine/resume.ts`). `id` es el del punto de control: mientras siga siendo el
 * que existe, el registro ofrece «Continuar».
 */
export interface JobInterruption {
  id: string;
  /** 0 a 100, según los `ok` recibidos. */
  percent: number;
  /** Líneas confirmadas y total. */
  line: number;
  total: number;
}

export interface JobLogEntry {
  id: string;
  /** Cuándo empezó el trabajo (ms desde 1970). */
  at: number;
  /** Nombre de la máquina (perfil) a la que se envió. */
  machine: string;
  /** Archivo del proyecto; `null` si no estaba guardado. */
  document: string | null;
  /** Tiempo estimado por la vista previa al empezar (s); `null` si no había una al día. */
  estimatedS: number | null;
  /** Tiempo real, sin contar las pausas (s). */
  actualS: number;
  result: JobResult;
  /** Texto del motor al terminar (el motivo, si falló). */
  message: string;
  /** Solo si quedó punto de control para continuarlo. */
  interrupted?: JobInterruption;
}

/** Cuántos trabajos se guardan; al pasarse se descartan los más viejos. */
export const JOB_LOG_MAX = 200;
export const JOB_LOG_KEY = 'jobLog';
/** Cuánto después de terminar un trabajo se le puede marcar como interrumpido. */
const INTERRUPTION_WINDOW_MS = 30 * 60 * 1000;

const RESULTS: readonly JobResult[] = ['finished', 'cancelled', 'error'];
const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);

function sanitizeInterruption(raw: unknown): JobInterruption | undefined {
  const r = raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : null;
  if (!r || typeof r.id !== 'string' || !r.id || !num(r.percent) || !num(r.line) || !num(r.total)) return undefined;
  return { id: r.id.slice(0, 80), percent: Math.min(100, Math.max(0, r.percent)), line: Math.max(0, r.line), total: Math.max(0, r.total) };
}

/** Entradas válidas a partir de cualquier dato (lo dañado se descarta), de la más nueva a la más vieja. */
export function sanitizeJobLog(raw: unknown): JobLogEntry[] {
  if (!Array.isArray(raw)) return [];
  const out: JobLogEntry[] = [];
  for (const item of raw) {
    const r = item && typeof item === 'object' ? (item as Record<string, unknown>) : null;
    if (!r || !num(r.at) || !num(r.actualS) || typeof r.machine !== 'string' || !RESULTS.includes(r.result as JobResult)) continue;
    const interrupted = sanitizeInterruption(r.interrupted);
    out.push({
      id: typeof r.id === 'string' && r.id ? r.id : `${r.at}`,
      at: r.at,
      machine: r.machine.slice(0, 200),
      document: typeof r.document === 'string' ? r.document.slice(0, 300) : null,
      estimatedS: num(r.estimatedS) && r.estimatedS >= 0 ? r.estimatedS : null,
      actualS: Math.max(0, r.actualS),
      result: r.result as JobResult,
      message: typeof r.message === 'string' ? r.message.slice(0, 500) : '',
      ...(interrupted ? { interrupted } : {}),
    });
  }
  return out.sort((a, b) => b.at - a.at).slice(0, JOB_LOG_MAX);
}

export function readJobLog(): JobLogEntry[] {
  try {
    return sanitizeJobLog(JSON.parse(readPref(JOB_LOG_KEY) ?? '[]'));
  } catch {
    return [];
  }
}

interface JobLogStore {
  entries: JobLogEntry[];
  /** Vuelve a leer el almacenamiento (otra ventana pudo anotar algo). */
  refresh: () => void;
  append: (entry: JobLogEntry) => void;
  /** Marca el trabajo más reciente (si no terminó bien y acaba de pasar) como interrumpido con punto de control. */
  markLatestInterrupted: (info: JobInterruption, now?: number) => void;
  clear: () => void;
}

/** Estado del registro. `append` lee antes de escribir para no pisar lo que anotó otra ventana. */
export const useJobLog = create<JobLogStore>()((set) => ({
  entries: readJobLog(),
  refresh: () => set({ entries: readJobLog() }),
  append: (entry) => {
    const entries = sanitizeJobLog([entry, ...readJobLog()]);
    writePref(JOB_LOG_KEY, JSON.stringify(entries));
    set({ entries });
  },
  markLatestInterrupted: (info, now = Date.now()) => {
    const entries = readJobLog();
    const latest = entries[0];
    // Solo el último trabajo, solo si no terminó bien y solo si acaba de terminar (no uno viejo).
    if (!latest || latest.result === 'finished' || now - (latest.at + latest.actualS * 1000) > INTERRUPTION_WINDOW_MS) return;
    entries[0] = { ...latest, interrupted: info };
    writePref(JOB_LOG_KEY, JSON.stringify(entries));
    set({ entries });
  },
  clear: () => {
    writePref(JOB_LOG_KEY, '[]');
    set({ entries: [] });
  },
}));

// ---- anotar desde los eventos de la máquina -------------------------------------------------------

/** Lo que el slice de la máquina sabe en el momento de un evento del trabajo. */
export interface JobLogContext {
  now: number;
  machine: string;
  document: string | null;
  estimatedS: number | null;
  /** Segundos de trabajo transcurridos (sin pausas); solo cuenta al terminar. */
  elapsedS: number;
}

interface Pending {
  at: number;
  machine: string;
  document: string | null;
  estimatedS: number | null;
}

let pending: Pending | null = null;
let seq = 0;

/** Olvida el trabajo en curso (para las pruebas). */
export function resetJobLogRecorder(): void {
  pending = null;
}

function finish(result: JobResult, message: string, ctx: JobLogContext): void {
  const start = pending;
  if (!start) return;
  pending = null;
  useJobLog.getState().append({
    id: `${start.at}-${++seq}`,
    at: start.at,
    machine: start.machine,
    document: start.document,
    estimatedS: start.estimatedS,
    actualS: Math.max(0, ctx.elapsedS),
    result,
    message,
  });
}

/**
 * Anota un evento `machine://job`. `wasBusy` dice si ya había un trabajo en curso antes del evento
 * (un «started» tras una pausa es una reanudación, no un trabajo nuevo). Al llegar un final
 * (terminado, cancelado, error) se agrega la entrada; pausas y reanudaciones no anotan nada.
 */
export function noteJobEvent(wasBusy: boolean, job: { state: 'started' | 'paused' | 'finished' | 'cancelled' | 'error'; message: string }, ctx: JobLogContext): void {
  if (job.state === 'started') {
    if (!wasBusy) pending = { at: ctx.now, machine: ctx.machine, document: ctx.document, estimatedS: ctx.estimatedS };
    return;
  }
  if (job.state === 'finished' || job.state === 'cancelled' || job.state === 'error') finish(job.state, job.message, ctx);
}

/** Un trabajo que falló sin evento propio (la máquina entró en alarma durante el trabajo). */
export function noteJobAlarm(message: string, ctx: JobLogContext): void {
  finish('error', message, ctx);
}
