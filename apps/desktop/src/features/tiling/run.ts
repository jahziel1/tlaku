import type { JobArgs } from '@/api/types';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { selectActiveDevice, useStore } from '@/store';
import { sendBlockedReason } from '@/store/machine';
import { feedLines, validateFeeder } from './feeder';
import { withAlignmentMarks, markIds } from './marks';
import { alignmentMarks, feedBetween, isSingleDirection, type MarkSegment, type TileSpec } from './plan';
import { isComplete, loadSession, mark, newSession, nextPending, saveSession, type TileSession } from './session';
import { plannedWithMargin, useTilingUi } from './store';

/** Espera a que el estado de la máquina sea `idle` (con tope), para no mandar el siguiente tramo en pleno movimiento. */
const IDLE_WAIT = { tries: 600, everyMs: 200 } as const;
const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

const ui = () => useTilingUi.getState();

/** Cruces de alineación de una sesión (vacío si se apagaron). */
export function sessionMarks(s: TileSession): MarkSegment[] {
  return s.config.marks ? alignmentMarks(s.design, s.config.size, s.config.overlap) : [];
}

/** Argumentos del trabajo de un tramo: el documento con las marcas, en modo absoluto y con `tile`. */
export function tileJobArgs(args: JobArgs, tile: TileSpec, marks: MarkSegment[]): JobArgs {
  const selection = args.options.selection ? [...args.options.selection, ...markIds(marks)] : null;
  return {
    ...args,
    doc: withAlignmentMarks(args.doc, marks),
    // El tramo ya viene colocado (recorte + desplazamiento): el origen es la cama, no el cabezal.
    options: { ...args.options, start_from: 'absolute', use_selection_origin: false, selection, tile },
  };
}

type JobEnd = 'finished' | 'cancelled' | 'error';

/** Promesa del fin del trabajo en curso. Hay que crearla ANTES de iniciarlo. `cancel` suelta la espera. */
function watchJobEnd(): { done: Promise<JobEnd | 'abandoned'>; cancel: () => void } {
  let unsub = () => {};
  let seen = false;
  let cancel = () => {};
  const done = new Promise<JobEnd | 'abandoned'>((resolve) => {
    cancel = () => {
      unsub();
      resolve('abandoned');
    };
    unsub = useStore.subscribe((s, prev) => {
      if (s.jobState === prev.jobState) return;
      if (s.jobState === 'running') seen = true;
      if (seen && (s.jobState === 'finished' || s.jobState === 'cancelled' || s.jobState === 'error')) {
        unsub();
        resolve(s.jobState);
      }
    });
  });
  return { done, cancel };
}

async function waitIdle(): Promise<boolean> {
  // GRBL tarda un reporte en pasar de `run` a `idle`: se da un respiro antes de mirar.
  await sleep(IDLE_WAIT.everyMs);
  for (let i = 0; i < IDLE_WAIT.tries; i++) {
    const s = useStore.getState();
    if (!s.connection.connected) return false;
    if (!s.status || s.status.state === 'idle') return true;
    await sleep(IDLE_WAIT.everyMs);
  }
  return false;
}

function persist(session: TileSession): void {
  ui().set({ session });
  saveSession(session);
}

function fail(message: string): void {
  ui().set({ phase: 'failed', message });
}

/** Empieza una sesión nueva con el plan actual. La primera vez siempre pide confirmación. */
export function beginSession(): void {
  const { plan, design, config } = ui();
  if (!plan || !plan.ok || !design) return;
  const device = selectActiveDevice(useStore.getState());
  const specs = plannedWithMargin(plan.plan.tiles, config.margin).map((p) => p.spec);
  const feeder = device.feeder.enabled && isSingleDirection(plan.plan) && Object.keys(validateFeeder(device.feeder)).length === 0;
  const session = newSession(specs, design, config, device.id, feeder);
  persist(session);
  ui().set({ phase: 'place', message: null, saved: null });
}

/** Retoma la sesión guardada donde se quedó (el tramo que estaba en curso vuelve a pedir confirmación). */
export function resumeSession(): void {
  const saved = ui().saved ?? loadSession();
  if (!saved) return;
  ui().set({ session: saved, config: saved.config, design: saved.design, phase: 'place', message: null, saved: null });
}

/** Descarta la sesión y vuelve a las opciones. */
export function restartSession(): void {
  saveSession(null);
  ui().set({ session: null, phase: 'config', message: null, saved: null });
}

/** Cierra el asistente al terminar y borra el avance guardado. */
export function finishSession(): void {
  saveSession(null);
  ui().set({ session: null, phase: 'config', message: null, open: false });
}

/** Elige el tramo `index` (para repetirlo o ir a otro): pasa a pedir la colocación del material. */
export function selectTile(index: number): void {
  const s = ui().session;
  if (!s || index < 0 || index >= s.specs.length) return;
  if (ui().phase === 'sending' || ui().phase === 'feeding') return;
  persist({ ...mark(s, index, 'pending'), current: index });
  ui().set({ phase: 'place', message: null });
}

/** Salta el tramo actual y pasa al siguiente pendiente (o termina). */
export function skipCurrent(): void {
  const s = ui().session;
  if (!s || ui().phase === 'sending' || ui().phase === 'feeding') return;
  void advanceFrom(mark(s, s.current, 'skipped'), false);
}

/** Cancela el trabajo del tramo en curso; la espera lo ve como cancelación y deja el tramo pendiente. */
export async function cancelCurrent(): Promise<void> {
  if (ui().phase === 'sending') await useStore.getState().cancelJob();
}

/** Marca el avance y pasa al siguiente tramo pendiente: seguido (alimentador) o pidiendo confirmación. */
async function advanceFrom(session: TileSession, mayFeed: boolean): Promise<void> {
  const next = nextPending(session);
  if (next === null || isComplete(session)) {
    persist(session);
    saveSession(null);
    ui().set({ phase: 'finished', message: null });
    return;
  }
  const from = session.current;
  persist({ ...session, current: next });
  const device = selectActiveDevice(useStore.getState());
  // Alimentador: solo al pasar al tramo contiguo; si no, el material lo coloca la persona.
  if (mayFeed && session.feeder && next === from + 1) await feedAndSend(session.specs[from]!, session.specs[next]!, device.feeder);
  else ui().set({ phase: 'place', message: null });
}

async function feedAndSend(from: TileSpec, to: TileSpec, feeder: ReturnType<typeof selectActiveDevice>['feeder']): Promise<void> {
  ui().set({ phase: 'feeding', message: null });
  const mm = feedBetween(from, to) ?? feeder.feed_mm;
  const app = useStore.getState();
  for (const line of feedLines(feeder, mm)) {
    if (!(await app.sendLine(line, { confirmed: true }))) return fail(t('tiling.err.feed'));
  }
  if (!(await waitIdle())) return fail(t('tiling.err.feedWait'));
  await sendCurrent();
}

/** Envía el tramo actual y, al terminar, pasa al siguiente. */
export async function sendCurrent(): Promise<void> {
  const session = ui().session;
  if (!session) return;
  const app = useStore.getState();
  const why = sendBlockedReason(app);
  if (why) return ui().set({ phase: 'place', message: t(why) });
  ui().set({ phase: 'sending', message: null });
  const watcher = watchJobEnd();
  const spec = session.specs[session.current]!;
  try {
    await app.startJob((args) => tileJobArgs(args, spec, sessionMarks(session)));
  } catch (e) {
    watcher.cancel();
    return fail(errorMessage(e));
  }
  // `startJob` no lanza: guarda el rechazo del motor en `jobError`.
  const rejected = useStore.getState().jobError;
  if (rejected) {
    watcher.cancel();
    return fail(rejected);
  }
  const end = await watcher.done;
  if (end === 'abandoned') return;
  if (end !== 'finished') {
    return fail(end === 'cancelled' ? t('tiling.err.cancelled', { n: session.current + 1 }) : useStore.getState().jobMessage || t('tiling.err.job'));
  }
  // Se espera a que la cabeza termine de moverse antes de pedir el siguiente tramo.
  await waitIdle();
  const current = ui().session ?? session;
  await advanceFrom(mark(current, session.current, 'done'), true);
}

/** Para pruebas y para la interfaz: ¿hay un tramo en la máquina? */
export const isBusy = (): boolean => ui().phase === 'sending' || ui().phase === 'feeding';

