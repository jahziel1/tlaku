/**
 * Recorrido de la cámara del cabezal: mueve el láser por una cuadrícula con los comandos de jog
 * de la API (con el láser apagado), toma una foto en cada punto y arma el mosaico con el motor.
 * La confirmación del usuario se pide antes en la interfaz; aquí solo se ejecuta.
 */
import { getApi } from '@/api';
import type { BedImage, BedTile, HeadCalibration } from '@/api/types';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { objectBounds, unionBounds } from '@/lib/geometry';
import { selectActiveDevice, selectSelectedObjects, useStore } from '@/store';
import { selectHeadDoc } from '@/store/machine';
import { projectDir, projectStem } from '@/features/trace/bitmap';
import type { Quad, Vec2 } from './geometry';
import { SCAN_CROP, planScan, headTileToBed, type ScanPlan, type ScanStop } from './head';
import { useHeadUi } from './headState';
import { matrixForFrame } from './mapping';
import { correctedSpace, grabCorrectedColorPng, setMosaicBackdrop } from './session';

/** Reposo (ms) tras llegar a cada punto antes de la foto: que la imagen deje de vibrar. */
export const SETTLE_MS = 500;
/** Tolerancia (mm) para dar por llegada la cabeza. */
const ARRIVE_MM = 0.4;

export interface Frame {
  png: string;
  width: number;
  height: number;
}

/** Lo que el recorrido necesita de la máquina y de la cámara; las pruebas ponen las suyas. */
export interface ScanDeps {
  /** Lleva el láser al punto (mm de documento) y regresa cuando llegó y se detuvo. */
  moveTo(laser: Vec2): Promise<void>;
  /** Posición actual del láser, o `null` si no se conoce. */
  position(): Vec2 | null;
  capture(): Promise<Frame | null>;
  settle(ms: number): Promise<void>;
  cancelled(): boolean;
  onProgress(step: number, total: number): void;
}

export interface ScanShot {
  frame: Frame;
  /** Dónde estaba el láser al tomar la foto. */
  laser: Vec2;
  stop: ScanStop;
}

export class ScanError extends Error {}

/** Recorre las posiciones, en orden, tomando una foto en cada una. Se detiene si se cancela. */
export async function runScan(plan: ScanPlan, deps: ScanDeps, settleMs = SETTLE_MS): Promise<{ shots: ScanShot[]; cancelled: boolean }> {
  const shots: ScanShot[] = [];
  for (let i = 0; i < plan.stops.length; i++) {
    if (deps.cancelled()) return { shots, cancelled: true };
    const stop = plan.stops[i]!;
    deps.onProgress(i + 1, plan.stops.length);
    await deps.moveTo(stop.laser);
    if (deps.cancelled()) return { shots, cancelled: true };
    await deps.settle(settleMs);
    const frame = await deps.capture();
    if (!frame) throw new ScanError(t('camera.head.scan.noFrame'));
    shots.push({ frame, laser: deps.position() ?? stop.laser, stop });
  }
  return { shots, cancelled: false };
}

/** Fotos del recorrido con su transformación píxel -> mm (con la posición real del láser). */
export function shotsToTiles(shots: readonly ScanShot[], cal: HeadCalibration, space: readonly [number, number]): BedTile[] {
  return shots.map((s) => ({
    image_png_base64: s.frame.png,
    to_bed: matrixForFrame(headTileToBed(cal, s.laser), space, [s.frame.width, s.frame.height]),
  }));
}

type StoreLike = Pick<typeof useStore, 'getState' | 'subscribe'>;

/** Espera a que el láser llegue a `target` y la máquina esté en reposo. */
export function waitForArrival(store: StoreLike, target: Vec2, timeoutMs: number): Promise<void> {
  return new Promise((resolve, reject) => {
    const arrived = () => {
      const s = store.getState();
      const pos = selectHeadDoc(s);
      return s.status?.state === 'idle' && !!pos && Math.hypot(pos[0] - target[0], pos[1] - target[1]) <= ARRIVE_MM;
    };
    if (arrived()) return resolve();
    let off = () => {};
    const timer = setTimeout(() => {
      off();
      reject(new ScanError(t('camera.head.scan.timeout')));
    }, timeoutMs);
    off = store.subscribe(() => {
      if (!arrived()) return;
      clearTimeout(timer);
      off();
      resolve();
    });
  });
}

/** Dependencias reales: la máquina del store (jog con láser apagado) y la cámara de la sesión. */
export function machineDeps(store: StoreLike = useStore, onProgress?: ScanDeps['onProgress']): ScanDeps {
  return {
    async moveTo(laser) {
      const s = store.getState();
      const ok = await s.goToDocPoint(Math.round(laser[0] * 1000) / 1000, Math.round(laser[1] * 1000) / 1000);
      if (!ok) throw new ScanError(t('camera.head.scan.moveFailed'));
      const here = selectHeadDoc(store.getState());
      const dist = here ? Math.hypot(here[0] - laser[0], here[1] - laser[1]) : 400;
      // Tiempo de sobra: distancia a la velocidad de jog (con piso de 10 s) más un colchón.
      const speed = Math.max(60, store.getState().jogSpeed);
      await waitForArrival(store, laser, Math.max(10_000, (dist / speed) * 60_000 * 2 + 5000));
    },
    position: () => selectHeadDoc(store.getState()),
    capture: () => grabCorrectedColorPng(),
    settle: (ms) => new Promise((r) => setTimeout(r, ms)),
    cancelled: () => useHeadUi.getState().cancelRequested,
    onProgress: (step, total) => {
      useHeadUi.getState().setProgress({ step, total });
      onProgress?.(step, total);
    },
  };
}

/** Zona a recorrer: la caja de la selección (si se pide) o toda la cama. */
export function scanRegion(useSelection: boolean): [number, number, number, number] | undefined {
  if (!useSelection) return undefined;
  const s = useStore.getState();
  const box = unionBounds(selectSelectedObjects(s).map(objectBounds));
  return box ? [box[0][0], box[0][1], box[1][0], box[1][1]] : undefined;
}

export interface ScanResult {
  image: BedImage;
  shots: number;
  partial: boolean;
  cancelled: boolean;
}

/**
 * Recorre la cama y compone el mosaico. Con `save` escribe el PNG para el documento; sin él solo
 * se queda la vista previa como fondo. Lanza un error con texto para el usuario.
 */
export async function scanBed(opts: { save: boolean; useSelection: boolean }, deps?: ScanDeps, store: StoreLike = useStore): Promise<ScanResult | null> {
  const s = store.getState();
  const cal = s.cameraSetup.head?.calibration;
  const space = correctedSpace();
  if (!cal) throw new ScanError(t('camera.mapping.noHeadCalibration'));
  if (!space || cal.image_size[0] !== space[0] || cal.image_size[1] !== space[1]) throw new ScanError(t('camera.mapping.staleHead'));
  const bed = selectActiveDevice(s).bed;
  const plan = planScan(bed, cal, scanRegion(opts.useSelection));
  if ('error' in plan) throw new ScanError(t(`camera.head.plan.${plan.error}`));
  const ui = useHeadUi.getState();
  ui.requestCancel(false);
  ui.setError(null);
  try {
    const { shots, cancelled } = await runScan(plan, deps ?? machineDeps(store));
    if (cancelled || shots.length === 0) return null;
    const image = await composeShots(shots, cal, space, plan, opts.save, store);
    return { image, shots: shots.length, partial: plan.partial, cancelled: false };
  } finally {
    ui.setProgress(null);
    ui.requestCancel(false);
  }
}

async function composeShots(shots: readonly ScanShot[], cal: HeadCalibration, space: readonly [number, number], plan: ScanPlan, save: boolean, store: StoreLike): Promise<BedImage> {
  const s = store.getState();
  const frameMin = Math.min(...shots.map((x) => Math.min(x.frame.width, x.frame.height)));
  const device = selectActiveDevice(s);
  return getApi().composeBedImage({
    tiles: shotsToTiles(shots, cal, space),
    bed: device.bed,
    px_per_mm: null,
    grayscale: false,
    feather_px: Math.round(frameMin * 0.1),
    crop_px: Math.round(frameMin * SCAN_CROP),
    rect: plan.region,
    dir: projectDir(s.filePath),
    stem: projectStem(s.filePath, t('doc.untitled')),
    save,
  });
}

/** Esquinas (arriba-izq., arriba-der., abajo-der., abajo-izq.) de una imagen compuesta en la cama. */
export function imageCorners(image: Pick<BedImage, 'origin_mm' | 'size_mm'>): Quad {
  const [x, y] = image.origin_mm;
  const [w, h] = image.size_mm;
  return [[x, y + h], [x + w, y + h], [x + w, y], [x, y]];
}

/** Decodifica un PNG en base64 a un canvas (solo en el navegador). */
export async function pngToCanvas(b64: string): Promise<HTMLCanvasElement> {
  const img = new Image();
  img.src = `data:image/png;base64,${b64}`;
  await img.decode();
  const cv = document.createElement('canvas');
  cv.width = img.naturalWidth;
  cv.height = img.naturalHeight;
  cv.getContext('2d')?.drawImage(img, 0, 0);
  return cv;
}

/** Deja el mosaico como fondo del lienzo, fijo en la cama. */
export async function showMosaic(image: BedImage, info: { shots: number; partial: boolean }): Promise<void> {
  const canvas = await pngToCanvas(image.preview_png_base64);
  setMosaicBackdrop({ canvas, corners: imageCorners(image) });
  useHeadUi.getState().setMosaic({ tiles: info.shots, coverage: image.coverage, partial: info.partial });
}

/** Quita el mosaico y vuelve al cuadro en vivo que sigue al láser. */
export function clearMosaic(): void {
  setMosaicBackdrop(null);
  useHeadUi.getState().setMosaic(null);
}

/** "Actualizar la superposición" de la cámara del cabezal: recorrido y mosaico como fondo. */
export async function updateHeadOverlay(): Promise<boolean> {
  const s = useStore.getState();
  try {
    const r = await scanBed({ save: false, useSelection: useHeadUi.getState().useSelection });
    if (!r) {
      s.notify('info', t('camera.head.scan.cancelled'));
      return false;
    }
    await showMosaic(r.image, r);
    s.notify('success', t('camera.head.scan.done', { n: r.shots }));
    if (r.partial) s.notify('warning', t('camera.head.scan.partial'));
    return true;
  } catch (e) {
    useHeadUi.getState().setError(errorMessage(e));
    s.notify('error', errorMessage(e));
    return false;
  }
}
