/**
 * Buscador de centro con el cabezal (M48): el centro de un objeto redondo a partir de tres puntos
 * de su borde. Dos formas de conseguir los puntos:
 *
 * - **Con sonda** (`G38.3`, solo GRBL): el cabezal avanza despacio hacia el objeto con el láser
 *   apagado hasta que la sonda o placa de contacto cierra el circuito, y ese punto se guarda.
 * - **Con el puntero**: la persona lleva el puntero rojo (el cabezal) a un punto del borde y
 *   confirma. No necesita sonda.
 *
 * El círculo sale de `camera/center.ts` (tres puntos o ajuste por mínimos cuadrados) y de ahí se
 * mueve la cabeza o se fija el origen con las mismas acciones del buscador por cámara.
 */
import { create } from 'zustand';
import { getApi } from '@/api';
import type { Point } from '@/api/types';
import { centerFromPoints } from '@/features/camera/center';
import { moveHeadToCenter, setOriginAtCenter, useCenter } from '@/features/camera/centerFinder';
import { t, type I18nKey } from '@/i18n';
import { workToDoc } from '@/lib/device';
import { errorMessage } from '@/lib/errors';
import { selectActiveDevice, useStore, type AppState } from '@/store';
import { jogBlockedReason, selectHeadDoc } from '@/store/machine';
import { stepDelta, type JogDir } from './jogMath';

export type ProbeMode = 'probe' | 'pointer';

/** Valores por omisión de la sonda: despacio, poco recorrido y un retroceso corto. */
export const PROBE_DEFAULTS = { feed: 100, travel: 30, retract: 2 } as const;
/** Mismos topes que el motor (`machine/probe.rs`): el motor los vuelve a comprobar. */
export const PROBE_LIMITS = { feedMax: 500, travelMax: 100, retractMax: 20 } as const;

/** Las cuatro direcciones desde las que se toca: el cabezal se mueve hacia ellas. */
export const PROBE_DIRS = {
  right: { x: 1, y: 0 },
  left: { x: -1, y: 0 },
  up: { x: 0, y: 1 },
  down: { x: 0, y: -1 },
} as const satisfies Record<string, JogDir>;
export type ProbeDirName = keyof typeof PROBE_DIRS;

/** Controladoras de la familia GRBL (las únicas con `G38.x` y `[PRB:...]` en este programa). */
const GRBL_KINDS = new Set(['grbl', 'grbl_m3', 'grbl_hal', 'fluid_nc', 'grbl_lpc']);

/** Parámetros de un toque ya acotados a los límites. */
export interface ProbeParams {
  feed: number;
  travel: number;
  retract: number;
}

/** Clave del problema de los parámetros, o `null` si valen. */
export function probeParamsProblem(p: ProbeParams): I18nKey | null {
  if (!(Number.isFinite(p.feed) && p.feed > 0 && p.feed <= PROBE_LIMITS.feedMax)) return 'machine.probe.problem.feed';
  if (!(Number.isFinite(p.travel) && p.travel >= 0.5 && p.travel <= PROBE_LIMITS.travelMax)) return 'machine.probe.problem.travel';
  if (!(Number.isFinite(p.retract) && p.retract >= 0 && p.retract <= PROBE_LIMITS.retractMax)) return 'machine.probe.problem.retract';
  return null;
}

/**
 * Por qué no se puede usar la sonda ahora (clave del motivo) o `null`. Hace falta poder mover el
 * cabezal (conectada, en reposo, sin trabajo) y una controladora GRBL; con rotativo la Y no es una
 * cama plana.
 */
export function probeBlockedReason(s: Pick<AppState, 'connection' | 'jobState' | 'status' | 'devices' | 'activeDeviceId'>): I18nKey | null {
  const jog = jogBlockedReason(s);
  if (jog) return jog;
  const device = selectActiveDevice(s);
  if (!GRBL_KINDS.has(device.kind)) return 'machine.probe.reason.family';
  if (device.rotary.enabled) return 'machine.probe.reason.rotary';
  return null;
}

/** Recorrido de un toque en mm de trabajo (el del eje X/Y de la máquina), según el origen del perfil. */
export function touchDelta(device: Parameters<typeof stepDelta>[0], dir: ProbeDirName, travel: number): Point {
  return stepDelta(device, PROBE_DIRS[dir], travel);
}

/** Un punto del borde ya medido, en mm del documento. */
export interface ProbePoint {
  doc: Point;
  /** Cómo se obtuvo: tocando con la sonda o confirmado a mano con el puntero. */
  via: ProbeMode;
}

interface ProbeCenterState {
  open: boolean;
  mode: ProbeMode;
  feed: number;
  travel: number;
  retract: number;
  points: ProbePoint[];
  /** Dirección pendiente de confirmar (se enseña el recorrido y la velocidad antes de mover). */
  confirm: ProbeDirName | null;
  busy: boolean;
  error: string | null;
  /** Último toque sin contacto (para decirlo sin tratarlo como error). */
  missed: boolean;
  openDialog: () => void;
  close: () => void;
  setMode: (m: ProbeMode) => void;
  setParam: (p: Partial<ProbeParams>) => void;
  set: (p: Partial<Pick<ProbeCenterState, 'points' | 'confirm' | 'busy' | 'error' | 'missed'>>) => void;
  reset: () => void;
}

const initial = {
  mode: 'probe' as ProbeMode,
  feed: PROBE_DEFAULTS.feed as number,
  travel: PROBE_DEFAULTS.travel as number,
  retract: PROBE_DEFAULTS.retract as number,
  points: [] as ProbePoint[],
  confirm: null as ProbeDirName | null,
  busy: false,
  error: null as string | null,
  missed: false,
};

export const useProbeCenter = create<ProbeCenterState>()((set) => ({
  open: false,
  ...initial,
  openDialog: () => set({ open: true, points: [], confirm: null, busy: false, error: null, missed: false }),
  close: () => set({ open: false, confirm: null }),
  setMode: (mode) => set({ mode, points: [], confirm: null, error: null, missed: false }),
  setParam: (p) => set(p),
  set: (p) => set(p),
  reset: () => set({ points: [], confirm: null, busy: false, error: null, missed: false }),
}));

const state = () => useProbeCenter.getState();
const app = () => useStore.getState();

/** Publica el círculo de los puntos en el buscador de centro (para mover la cabeza o fijar el origen). */
function publish(points: readonly ProbePoint[], mode: ProbeMode): void {
  const doc = points.map((p) => p.doc);
  const r = centerFromPoints(doc);
  if ('error' in r) {
    useCenter.getState().set({ result: null });
    state().set({ error: t(r.error === 'tooFew' ? 'machine.probe.tooFew' : 'camera.center.collinear') });
    return;
  }
  useCenter.getState().set({ result: { circle: r.circle, source: mode }, error: null });
}

/** Con 3 puntos hay círculo; el cuarto punto empieza de nuevo. */
function addPoint(p: ProbePoint): void {
  const kept = state().points.length >= 3 ? [] : state().points;
  const points = [...kept, p];
  state().set({ points, error: null });
  if (points.length >= 3) publish(points, p.via);
  else useCenter.getState().set({ result: null });
}

/** Pide confirmar un toque: no se mueve nada hasta `confirmTouch`. */
export function requestTouch(dir: ProbeDirName): void {
  const s = app();
  const why = probeBlockedReason(s);
  if (why) {
    state().set({ error: t(why) });
    return;
  }
  const bad = probeParamsProblem(state());
  if (bad) {
    state().set({ error: t(bad) });
    return;
  }
  state().set({ confirm: dir, error: null, missed: false });
}

export function cancelTouch(): void {
  state().set({ confirm: null });
}

/** Hace el toque confirmado: `G38.3` hacia la dirección elegida y guarda el punto si tocó. */
export async function confirmTouch(): Promise<void> {
  const { confirm, feed, travel, retract, busy } = state();
  if (!confirm || busy) return;
  const s = app();
  const why = probeBlockedReason(s);
  if (why) {
    state().set({ confirm: null, error: t(why) });
    return;
  }
  const device = selectActiveDevice(s);
  const [dx, dy] = touchDelta(device, confirm, travel);
  state().set({ busy: true, confirm: null, error: null, missed: false });
  try {
    const r = await getApi().probeTouch({ dx, dy, feed, retract, confirmed: true });
    if (!r.hit) {
      state().set({ busy: false, missed: true });
      return;
    }
    state().set({ busy: false });
    addPoint({ doc: workToDoc(device, r.work_pos), via: 'probe' });
  } catch (e) {
    state().set({ busy: false, error: errorMessage(e) });
  }
}

/**
 * Modo del puntero: guarda la posición actual del cabezal como un punto del borde. El puntero cae
 * `pointer_offset` mm del rayo; el centro se calcula en el espacio del puntero y los puntos se pasan
 * a coordenadas del rayo sumando ese desfase, así «Mover la cabeza al centro» deja el rayo en el centro.
 */
export function recordPointerPoint(): boolean {
  const s = app();
  const head = selectHeadDoc(s);
  if (!head) {
    state().set({ error: t('machine.probe.noPosition') });
    return false;
  }
  const [ox, oy] = selectActiveDevice(s).pointer_offset ?? [0, 0];
  addPoint({ doc: [head[0] + ox, head[1] + oy], via: 'pointer' });
  return true;
}

export function removeLastPoint(): void {
  const points = state().points.slice(0, -1);
  state().set({ points, error: null });
  useCenter.getState().set({ result: null });
  if (points.length >= 3) publish(points, points[points.length - 1]!.via);
}

export function restart(): void {
  state().reset();
  useCenter.getState().reset();
}

export { moveHeadToCenter, setOriginAtCenter };
