/**
 * Alinear la cámara fija con el cabezal, sin grabar (Falcon2 Pro y cámaras de tapa parecidas).
 *
 * Con el láser APAGADO, el cabezal visita una rejilla de puntos conocidos de la cama (dentro del
 * área segura, con la guarda de la cama de `goToDocPoint`), se toma una foto en cada punto y el
 * motor (`camera_head_align_*`) localiza el cabezal en las fotos y ajusta el mapeo imagen -> cama.
 * Si la cámara solo ve el cabezal en parte de la cama, se afina la rejilla donde se lo vio: las
 * rondas siguen el mismo criterio que `run_head_align_flow` del motor (que usan las pruebas con
 * la máquina virtual y `hwtest`). Aquí solo se conecta con la máquina, la cámara y el estado.
 */
import { getApi } from '@/api';
import type { LaserApi } from '@/api/api';
import type { HeadAlignConfig, HeadAlignFrame, HeadAlignPlan, HeadAlignReport, LensCalibration } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { selectActiveDevice, useStore } from '@/store';
import type { AppState } from '@/store/types';
import { jogBlockedReason, positionLaserBlockedReason, selectHeadDoc } from '@/store/machine';
import { applyAlignment, persistCameraSetup } from './actions';
import type { Vec2 } from './geometry';
import { useHeadAlign, type HeadAlignSettings, type ShotInfo } from './headAlignState';
import { correctedSpace, framesRendered, grabCorrectedPng } from './session';

const app = () => useStore.getState();

/** Rondas de afinado como máximo tras la rejilla inicial, y puntos con los que se da por buena. */
export const HEAD_ALIGN_MAX_ROUNDS = 3;
export const HEAD_ALIGN_TARGET_POINTS = 9;
/**
 * Tiempos del recorrido: espera tras cada movimiento antes de la foto (vibración y exposición de
 * la cámara) y tiempo máximo para que el cabezal llegue y se detenga. Mutable para las pruebas.
 */
export const headAlignTiming = { settleMs: 900, stillTimeoutMs: 120_000 };

/** El usuario detuvo el recorrido. */
export class AlignCancelled extends Error {
  constructor() {
    super(t('camera.headAlign.cancelled'));
  }
}

/** Lo que el recorrido necesita del mundo: mover el cabezal, tomar una foto y hacer su miniatura. */
export interface AlignIo {
  /** Lleva el láser a `mm` (documento) sin encenderlo y espera a que se detenga y se asiente. */
  moveTo(mm: Vec2): Promise<void>;
  /** Foto corregida de lente (PNG en base64) del espacio de la alineación. */
  capture(): Promise<string>;
  thumbnail(png: string): Promise<string | null>;
}

type AlignApi = Pick<LaserApi, 'cameraHeadAlignPlan' | 'cameraHeadAlignRefine' | 'cameraHeadAlignSolve'>;

/** Motivo por el que todavía no se puede empezar, o `null`. */
export function headAlignBlockedReason(s: Pick<AppState, 'cameraStatus' | 'cameraFrozen' | 'connection' | 'jobState' | 'status' | 'devices' | 'activeDeviceId'> = app()): I18nKey | null {
  if (s.cameraStatus !== 'live') return 'camera.reason.notLive';
  if (s.cameraFrozen) return 'camera.headAlign.reason.frozen';
  if (!s.connection.connected) return 'reason.needMachine';
  return positionLaserBlockedReason(s);
}

/** Configuración del motor a partir de los ajustes y la lente vigente. */
export function headAlignConfig(settings: HeadAlignSettings, bed: [number, number], lens: LensCalibration | null, area: HeadAlignPlan['area'] | null): HeadAlignConfig {
  const { cameraHeightMm: cam, featureHeightMm: feat } = settings;
  return {
    bed,
    safe_area: area,
    detect: { method: settings.method },
    lens,
    estimate_lens: lens === null,
    parallax: cam !== null && feat !== null && cam > feat + 5 ? { camera_height_mm: cam, feature_height_mm: feat } : null,
    offset_mm: [settings.offset[0], settings.offset[1]],
  };
}

const near = (a: Vec2, b: Vec2) => Math.hypot(a[0] - b[0], a[1] - b[1]) < 1;

/** Posiciones del plan (más la referencia) que todavía no tienen foto. */
export function freshPositions(plan: HeadAlignPlan, frames: readonly HeadAlignFrame[]): Vec2[] {
  return [...plan.points, plan.reference].filter((p) => !frames.some((f) => near(f.machine_mm, p)));
}

/** ¿Se vio el cabezal en todo el borde de la rejilla? Entonces la cámara probablemente ve más allá. */
export function edgeAllSeen(plan: HeadAlignPlan, seen: readonly Vec2[]): boolean {
  const a = plan.area;
  const onEdge = (p: Vec2) => [p[0] - a[0], a[2] - p[0], p[1] - a[1], a[3] - p[1]].some((d) => Math.abs(d) < 1);
  return plan.points.filter(onEdge).every((p) => seen.some((s) => near(s, p)));
}

const key = (round: number, i: number) => `r${round}-${i}`;

export interface RunResult {
  report: HeadAlignReport;
  frames: HeadAlignFrame[];
  rounds: number;
  /** Área segura que recorrió la rejilla. */
  area: HeadAlignPlan['area'];
}

/**
 * Recorre la rejilla, ajusta y afina. Deja el avance en el store (`useHeadAlign`) y lanza
 * `AlignCancelled` si el usuario detiene el recorrido (entre dos movimientos: nunca a mitad de uno).
 */
export async function runHeadAlign(deps: {
  api: AlignApi;
  io: AlignIo;
  settings: HeadAlignSettings;
  bed: [number, number];
  lens: LensCalibration | null;
}): Promise<RunResult> {
  const { api, io, settings, bed, lens } = deps;
  const ui = useHeadAlign.getState();
  const checkCancel = () => {
    if (useHeadAlign.getState().cancelRequested) throw new AlignCancelled();
  };
  const frames: HeadAlignFrame[] = [];

  const shoot = async (round: number, positions: Vec2[]) => {
    ui.setRound(round);
    const added: ShotInfo[] = positions.map((mm, i) => ({ key: key(round, i), mm, round, state: 'pending', thumb: null }));
    ui.setShots([...useHeadAlign.getState().shots, ...added]);
    for (const [i, mm] of positions.entries()) {
      checkCancel();
      ui.patchShot(key(round, i), { state: 'moving' });
      await io.moveTo(mm);
      checkCancel();
      const png = await io.capture();
      frames.push({ png_base64: png, machine_mm: mm });
      ui.patchShot(key(round, i), { state: 'shot' });
      void io.thumbnail(png).then((thumb) => thumb && ui.patchShot(key(round, i), { thumb }));
    }
  };

  const plan = await api.cameraHeadAlignPlan({ options: { bed, grid: settings.grid, margin_mm: settings.marginMm, region: null } });
  const config = headAlignConfig(settings, bed, lens, plan.area);
  await shoot(0, freshPositions(plan, frames));
  let report = await api.cameraHeadAlignSolve({ frames, config });
  let round = 0;
  let last = plan;
  while (round < HEAD_ALIGN_MAX_ROUNDS && (report.detected < HEAD_ALIGN_TARGET_POINTS || edgeAllSeen(last, report.seen_mm))) {
    checkCancel();
    const next = await api.cameraHeadAlignRefine({ area: plan.area, seen: report.seen_mm, grid: 3 });
    const positions = next ? freshPositions(next, frames) : [];
    if (!next || positions.length === 0) break;
    round += 1;
    last = next;
    await shoot(round, positions);
    report = await api.cameraHeadAlignSolve({ frames, config });
  }
  return { report, frames, rounds: round, area: plan.area };
}

// ---------------------------------------------------------------------------
// Con la máquina y la cámara de la app
// ---------------------------------------------------------------------------

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/** Espera a que el cabezal llegue a `mm` y quede en reposo (dos reportes seguidos de la máquina). */
async function waitStill(mm: Vec2): Promise<void> {
  const t0 = Date.now();
  let last = app().status;
  let ok = 0;
  while (Date.now() - t0 < headAlignTiming.stillTimeoutMs) {
    await sleep(100);
    if (useHeadAlign.getState().cancelRequested) throw new AlignCancelled();
    const s = app();
    if (s.status === last) continue;
    last = s.status;
    if (s.status?.state === 'alarm') throw new Error(t('camera.headAlign.err.alarm'));
    const head = selectHeadDoc(s);
    ok = head && s.status?.state === 'idle' && Math.hypot(head[0] - mm[0], head[1] - mm[1]) < 0.4 ? ok + 1 : 0;
    if (ok >= 2) return;
  }
  throw new Error(t('camera.headAlign.err.timeout'));
}

/** Espera la vibración y a que la cámara entregue cuadros nuevos (exposición automática). */
async function settleCamera(): Promise<void> {
  await sleep(headAlignTiming.settleMs);
  const n0 = framesRendered();
  const t0 = Date.now();
  while (framesRendered() < n0 + 3 && Date.now() - t0 < 2500) await sleep(40);
}

/** Miniatura JPEG de ~160 px de una foto PNG; `null` si el entorno no puede dibujarla. */
export async function makeThumbnail(png: string, width = 160): Promise<string | null> {
  try {
    const img = new Image();
    img.src = `data:image/png;base64,${png}`;
    await img.decode();
    if (!img.naturalWidth) return null;
    const cv = document.createElement('canvas');
    cv.width = width;
    cv.height = Math.max(1, Math.round((img.naturalHeight * width) / img.naturalWidth));
    cv.getContext('2d')?.drawImage(img, 0, 0, cv.width, cv.height);
    return cv.toDataURL('image/jpeg', 0.65);
  } catch {
    return null;
  }
}

/** El mundo real: `goToDocPoint` (con la guarda de la cama), el estado de la máquina y la cámara de la sesión. */
export function liveIo(): AlignIo {
  return {
    async moveTo(mm) {
      // Si algo cambió (alarma, otro trabajo), `goToDocPoint` avisa y devuelve false: se corta.
      if (jogBlockedReason(app())) throw new Error(t('camera.headAlign.err.move'));
      if (!(await app().goToDocPoint(mm[0], mm[1]))) throw new Error(t('camera.headAlign.err.move'));
      await waitStill(mm);
      await settleCamera();
    },
    async capture() {
      const frame = await grabCorrectedPng();
      if (!frame) throw new Error(t('camera.marks.noImage'));
      const space = correctedSpace();
      if (space && (frame.width !== space[0] || frame.height !== space[1])) throw new Error(t('camera.headAlign.err.size', { w: frame.width, h: frame.height, sw: space[0], sh: space[1] }));
      return frame.png;
    },
    thumbnail: (png) => makeThumbnail(png),
  };
}

/** Corre el recorrido completo con la máquina y la cámara de la app y deja el resultado en el store. */
export async function startHeadAlign(io: AlignIo = liveIo(), api: AlignApi = getApi()): Promise<boolean> {
  const ui = useHeadAlign.getState();
  const blocked = headAlignBlockedReason();
  if (blocked) {
    ui.setError(t(blocked));
    return false;
  }
  ui.setError(null);
  ui.requestCancel(false);
  ui.setShots([]);
  ui.setPhase('running');
  const s = app();
  try {
    const r = await runHeadAlign({ api, io, settings: ui.settings, bed: selectActiveDevice(s).bed, lens: s.cameraSetup.calibration });
    useHeadAlign.getState().setResult(r.report, r.frames, r.area);
    return true;
  } catch (e) {
    const cancelled = e instanceof AlignCancelled;
    useHeadAlign.getState().backToConfig();
    useHeadAlign.getState().setError(cancelled ? t('camera.headAlign.cancelledDetail') : t('camera.headAlign.err.run', { error: errorMessage(e) }));
    return false;
  }
}

/** Vuelve a ajustar con las mismas fotos y otro desfase / alturas (no mueve nada). */
export async function resolveHeadAlign(settings: HeadAlignSettings, api: AlignApi = getApi()): Promise<void> {
  const ui = useHeadAlign.getState();
  const s = app();
  const config = headAlignConfig(settings, selectActiveDevice(s).bed, s.cameraSetup.calibration, ui.area);
  try {
    const report = await api.cameraHeadAlignSolve({ frames: ui.frames, config });
    useHeadAlign.getState().setResult(report, ui.frames, ui.area);
  } catch (e) {
    ui.setError(t('camera.headAlign.err.run', { error: errorMessage(e) }));
  }
}

/** Por qué no se puede guardar el resultado, o `null`. */
export function saveBlockedReason(report: HeadAlignReport | null): 'noFit' | 'size' | null {
  if (!report?.alignment) return 'noFit';
  const space = correctedSpace();
  // Con lente estimada la imagen corregida pasa a tener el tamaño de las fotos.
  if (!report.lens && space && (report.alignment.image_size[0] !== space[0] || report.alignment.image_size[1] !== space[1])) return 'size';
  return null;
}

/**
 * Guarda la alineación como la de la cámara (mismo `camera.json`). Si el motor estimó una lente
 * aproximada (no había calibración), se guarda junto, porque la alineación vale sobre la imagen
 * corregida con ella; una lente nueva invalida la calibración de la cámara del cabezal.
 */
export function saveHeadAlignment(report: HeadAlignReport): boolean {
  if (saveBlockedReason(report) || !report.alignment) return false;
  if (report.lens) {
    const s = app();
    s.setCameraSetup({
      ...s.cameraSetup,
      calibration: report.lens,
      alignment: report.alignment,
      head: s.cameraSetup.head ? { ...s.cameraSetup.head, calibration: null } : s.cameraSetup.head,
    });
    void persistCameraSetup();
  } else {
    applyAlignment(report.alignment);
  }
  useHeadAlign.getState().markSaved();
  app().notify('success', t('camera.headAlign.saved'));
  return true;
}

/**
 * Ajuste fino con un único punto de referencia conocido: el usuario disparó una marca de prueba en
 * `bed` (mm de documento) y la marcó en la imagen (`px`). Mueve la alineación guardada para que ese
 * píxel caiga en esa posición. Devuelve el desplazamiento aplicado (mm) o `null` si falló.
 */
export async function applyReferencePoint(px: Vec2, bed: Vec2, api: Pick<LaserApi, 'cameraHeadAlignShift'> = getApi()): Promise<Vec2 | null> {
  const s = app();
  const alignment = s.cameraSetup.alignment;
  if (!alignment) {
    s.notify('error', t('camera.headAlign.ref.noAlignment'));
    return null;
  }
  try {
    const r = await api.cameraHeadAlignShift({ alignment, lens: s.cameraSetup.calibration, ref_px: px, ref_bed: bed });
    applyAlignment(r.alignment);
    s.notify('success', t('camera.headAlign.ref.applied', { dx: r.shift_mm[0].toFixed(2), dy: r.shift_mm[1].toFixed(2) }));
    return r.shift_mm;
  } catch (e) {
    s.notify('error', t('camera.headAlign.ref.failed', { error: errorMessage(e) }));
    return null;
  }
}
