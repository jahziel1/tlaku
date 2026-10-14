/**
 * Buscador de centro (M48): estado y acciones. Se marcan 3 puntos del borde de un objeto redondo
 * en la imagen de la cámara (o se pide al motor que detecte el círculo) y se obtiene el centro en
 * mm de la cama; de ahí se mueve la cabeza (láser apagado) o se fija el origen del trabajo.
 */
import { create } from 'zustand';
import { getApi } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { docToWork } from '@/lib/device';
import { selectActiveDevice, useStore } from '@/store';
import { centerFromPoints, type Circle } from './center';
import type { Vec2 } from './geometry';
import { currentMapping, MAPPING_PROBLEM_KEY } from './mapping';
import { correctedSpace, grabCorrectedPng } from './session';

export type CenterMode = 'three' | 'auto';

export interface CenterPick {
  /** Píxel marcado en el espacio de la imagen corregida (para dibujarlo). */
  px: Vec2;
  /** El mismo punto en mm de la cama, calculado al marcarlo. */
  mm: Vec2;
}

export interface CenterResult {
  circle: Circle;
  /** `three`/`auto`: cámara; `probe`/`pointer`: el cabezal (`machine/probeCenter.ts`). */
  source: CenterMode | 'probe' | 'pointer';
}

interface CenterState {
  mode: CenterMode;
  picks: CenterPick[];
  result: CenterResult | null;
  busy: boolean;
  error: string | null;
  setMode: (m: CenterMode) => void;
  set: (p: Partial<Pick<CenterState, 'picks' | 'result' | 'busy' | 'error'>>) => void;
  reset: () => void;
}

export const useCenter = create<CenterState>()((set) => ({
  mode: 'three',
  picks: [],
  result: null,
  busy: false,
  error: null,
  setMode: (mode) => set({ mode, picks: [], result: null, error: null }),
  set: (p) => set(p),
  reset: () => set({ picks: [], result: null, error: null, busy: false }),
}));

const state = () => useCenter.getState();
const app = () => useStore.getState();

/** Un clic del modo de 3 puntos. El cuarto clic empieza de nuevo. */
export function pickPoint(px: Vec2): void {
  const m = currentMapping(app());
  if (!m.ok) {
    state().set({ error: t(MAPPING_PROBLEM_KEY[m.reason]) });
    return;
  }
  const kept = state().picks.length >= 3 ? [] : state().picks;
  const picks = [...kept, { px, mm: m.map(px) }];
  if (picks.length < 3) {
    state().set({ picks, result: null, error: null });
    return;
  }
  const r = centerFromPoints(picks.map((p) => p.mm));
  if ('error' in r) {
    state().set({ picks, result: null, error: t('camera.center.collinear') });
    return;
  }
  state().set({ picks, result: { circle: r.circle, source: 'three' }, error: null });
}

/** Detección automática: el motor busca el círculo cerca del punto marcado. */
export async function detectAt(px: Vec2): Promise<void> {
  const m = currentMapping(app());
  if (!m.ok) {
    state().set({ error: t(MAPPING_PROBLEM_KEY[m.reason]) });
    return;
  }
  state().set({ busy: true, error: null, result: null, picks: [] });
  try {
    const frame = await grabCorrectedPng();
    if (!frame) throw new Error(t('camera.capture.noImage'));
    const space = correctedSpace() ?? m.space;
    // La foto puede venir reducida (máximo 1280 px de ancho): se pasa de y hacia el espacio de la alineación.
    const kx = space[0] / frame.width;
    const ky = space[1] / frame.height;
    const fit = await getApi().detectCircle({ image_png_base64: frame.png, hint: { center: [px[0] / kx, px[1] / ky], radius: null } });
    const mm = fit.points.map(([x, y]): Vec2 => m.map([x * kx, y * ky]));
    const r = centerFromPoints(mm);
    if ('error' in r) throw new Error(t('camera.center.collinear'));
    state().set({
      busy: false,
      picks: [{ px: [fit.center[0] * kx, fit.center[1] * ky], mm: r.circle.center }],
      result: { circle: r.circle, source: 'auto' },
    });
  } catch (e) {
    state().set({ busy: false, error: t('camera.center.detectFailed', { error: errorMessage(e) }) });
  }
}

/** Lleva el láser (apagado) al centro. Devuelve si la máquina aceptó el movimiento. */
export async function moveHeadToCenter(): Promise<boolean> {
  const r = state().result;
  if (!r) return false;
  const [x, y] = r.circle.center;
  return app().goToDocPoint(Math.round(x * 1000) / 1000, Math.round(y * 1000) / 1000);
}

/** Fija el origen de usuario del trabajo en el centro. */
export function setOriginAtCenter(): boolean {
  const r = state().result;
  const s = app();
  if (!r) return false;
  const [wx, wy] = docToWork(selectActiveDevice(s), r.circle.center);
  const origin: [number, number] = [Math.round(wx * 1000) / 1000, Math.round(wy * 1000) / 1000];
  s.setJobOptions({ user_origin: origin });
  s.notify('success', t('camera.center.originSet', { x: origin[0], y: origin[1] }));
  return true;
}
