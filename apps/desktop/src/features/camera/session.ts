/**
 * Sesión de cámara: una sola para toda la app. Vive fuera de React para que la cámara y el fondo
 * del lienzo sigan activos aunque el usuario cambie de pestaña del panel.
 *
 * Flujo por cuadro: video -> textura WebGL -> shader de Brown-Conrady -> canvas corregido. Nada de
 * video cruza el IPC. El fondo del lienzo comparte ese canvas (`preserveDrawingBuffer`) y se
 * republica a ~12 fps como máximo.
 */
import { useStore } from '@/store';
import type { CameraBackdrop } from '@/store/camera';
import { backdropCorners } from './geometry';
import { buildConstraints, classifyCameraError, pickDevice, videoInputs, type CameraProblem } from './capture';
import { createCorrector, type Corrector } from './corrector';
import { toBedAt } from './height';
import { correctedSize, lensFitsStream } from './lens';
import { persistCameraSetup } from './actions';

/** Intervalo mínimo entre publicaciones al lienzo (~12 fps). */
export const BACKDROP_MIN_INTERVAL_MS = 80;

let video: HTMLVideoElement | null = null;
let holder: HTMLDivElement | null = null;
let corrector: Corrector | null = null;
let stream: MediaStream | null = null;
let snapshot: HTMLCanvasElement | null = null;
let frameCallback = 0;
let rafId = 0;
let lastPublish = 0;
let frameCount = 0;
let startToken = 0;
let subscribed = false;

/** Medidas del último tramo de cuadros (para el diagnóstico de rendimiento). */
export const stats = { frames: 0, renderMs: 0 };

function ensureVideo(): HTMLVideoElement {
  if (video) return video;
  video = document.createElement('video');
  video.muted = true;
  video.playsInline = true;
  video.autoplay = true;
  video.setAttribute('aria-hidden', 'true');
  // Fuera de pantalla pero dentro del documento: algunos motores no reproducen videos sueltos.
  holder = document.createElement('div');
  holder.style.cssText = 'position:fixed;left:-9999px;top:0;width:1px;height:1px;overflow:hidden;opacity:0;pointer-events:none';
  holder.appendChild(video);
  document.body.appendChild(holder);
  return video;
}

function ensureCorrector(): Corrector {
  corrector ??= createCorrector();
  return corrector;
}

/** El elemento de video crudo, para mostrarlo en el asistente de calibración. */
export function getVideo(): HTMLVideoElement {
  return ensureVideo();
}

/** Devuelve el video a su contenedor oculto (al cerrar el asistente). */
export function parkVideo(): void {
  if (video && holder && video.parentElement !== holder) holder.appendChild(video);
}

/** Canvas con la imagen corregida (para mostrarlo en el panel). */
export function getCorrectedCanvas(): HTMLCanvasElement {
  return ensureCorrector().canvas;
}

export function usesGpu(): boolean {
  return ensureCorrector().gpu;
}

/** Tamaño del espacio de la imagen corregida: el de la calibración, o el del video sin lente. */
export function correctedSpace(): [number, number] | null {
  const { cameraSetup, cameraStreamSize } = useStore.getState();
  const c = cameraSetup.calibration;
  if (c) return [c.width, c.height];
  return cameraStreamSize;
}

/** ¿La calibración se puede aplicar al video actual? (misma proporción) */
export function lensApplies(): boolean {
  const { cameraSetup, cameraStreamSize } = useStore.getState();
  const c = cameraSetup.calibration;
  return !!c && !!cameraStreamSize && lensFitsStream(c, cameraStreamSize[0], cameraStreamSize[1]);
}

function renderFrame(): void {
  const v = ensureVideo();
  if (v.readyState < 2 || v.videoWidth === 0) return;
  const s = useStore.getState();
  const size: [number, number] = [v.videoWidth, v.videoHeight];
  if (!s.cameraStreamSize || s.cameraStreamSize[0] !== size[0] || s.cameraStreamSize[1] !== size[1]) {
    s.setCameraState({ cameraStreamSize: size });
  }
  if (s.cameraFrozen) return;
  const lens = s.cameraSetup.calibration;
  const useLens = lens && lensFitsStream(lens, size[0], size[1]) ? lens : null;
  const [w, h] = useLens ? correctedSize(useLens) : [Math.min(size[0], 1280), Math.round((Math.min(size[0], 1280) * size[1]) / size[0])];
  const t0 = performance.now();
  ensureCorrector().render(v, { width: w, height: h, lens: useLens });
  stats.renderMs += performance.now() - t0;
  stats.frames++;
  frameCount++;
  publishBackdrop(false);
}

function scheduleNext(): void {
  const v = ensureVideo();
  const tick = () => {
    renderFrame();
    if (stream) scheduleNext();
  };
  if ('requestVideoFrameCallback' in v) frameCallback = v.requestVideoFrameCallback(tick);
  else rafId = requestAnimationFrame(tick);
}

/** ¿La alineación guardada corresponde a la imagen que se está mostrando? */
export function alignmentIsCurrent(): boolean {
  const { cameraSetup } = useStore.getState();
  const a = cameraSetup.alignment;
  const space = correctedSpace();
  return !!a && !!space && a.image_size[0] === space[0] && a.image_size[1] === space[1];
}

/**
 * Publica el fondo en el lienzo. `force` ignora el límite de cuadros por segundo (cambios de
 * opacidad, alineación o congelado).
 */
export function publishBackdrop(force: boolean): void {
  const s = useStore.getState();
  const { alignment, prefs } = s.cameraSetup;
  const source = s.cameraFrozen && snapshot ? snapshot : stream && corrector ? corrector.canvas : null;
  const visible = prefs.visible && alignment !== null && source !== null && alignmentIsCurrent() && (s.cameraFrozen || lensOk());
  if (!visible || !alignment || !source) {
    if (s.cameraBackdrop) s.setCameraBackdrop(null);
    return;
  }
  const now = performance.now();
  if (!force && now - lastPublish < BACKDROP_MIN_INTERVAL_MS) return;
  lastPublish = now;
  const canvas = source as HTMLCanvasElement;
  const backdrop: CameraBackdrop = {
    source,
    width: canvas.width,
    height: canvas.height,
    // La homografía vale a la altura del material actual (corrección por grosor).
    corners: backdropCorners(toBedAt(alignment, prefs.material_height_mm ?? 0), alignment.image_size),
    opacity: prefs.opacity,
    frame: ++frameCount,
  };
  s.setCameraBackdrop(backdrop);
}

/** Sin calibración vale cualquier video; con calibración debe tener su proporción. */
function lensOk(): boolean {
  const { cameraSetup } = useStore.getState();
  return !cameraSetup.calibration || lensApplies();
}

function subscribeOnce(): void {
  if (subscribed) return;
  subscribed = true;
  // Cambios de alineación, opacidad, visibilidad o congelado: se republica sin esperar al video.
  useStore.subscribe((s, prev) => {
    if (s.cameraSetup !== prev.cameraSetup || s.cameraFrozen !== prev.cameraFrozen) publishBackdrop(true);
  });
}

export async function refreshDevices(): Promise<void> {
  if (!navigator.mediaDevices?.enumerateDevices) return;
  try {
    const devices = videoInputs(await navigator.mediaDevices.enumerateDevices());
    useStore.getState().setCameraState({ cameraDevices: devices });
  } catch {
    /* sin lista: se muestra la cámara predeterminada */
  }
}

function setProblem(problem: CameraProblem): void {
  useStore.getState().setCameraState({ cameraStatus: 'off', cameraProblem: problem });
}

/** Enciende la cámara con las preferencias guardadas. Los problemas quedan en el store. */
export async function startCamera(): Promise<void> {
  subscribeOnce();
  const token = ++startToken;
  const s = useStore.getState();
  stopStream();
  if (!navigator.mediaDevices?.getUserMedia) return setProblem('unsupported');
  s.setCameraState({ cameraStatus: 'starting', cameraProblem: null });
  const prefs = s.cameraSetup.prefs;
  const open = (deviceId: string | null) =>
    navigator.mediaDevices.getUserMedia(buildConstraints({ deviceId, width: prefs.width, height: prefs.height }));
  let opened: MediaStream;
  try {
    try {
      opened = await open(prefs.device_id);
    } catch (e) {
      // La cámara guardada ya no está (o cambió su id): se prueba con la predeterminada.
      const name = (e as { name?: string })?.name;
      if (prefs.device_id && (name === 'OverconstrainedError' || name === 'NotFoundError')) opened = await open(null);
      else throw e;
    }
  } catch (e) {
    if (token === startToken) setProblem(classifyCameraError(e));
    return;
  }
  if (token !== startToken) {
    opened.getTracks().forEach((tr) => tr.stop());
    return;
  }
  stream = opened;
  const v = ensureVideo();
  v.srcObject = opened;
  try {
    await v.play();
  } catch {
    stopStream();
    return setProblem('busy');
  }
  opened.getVideoTracks()[0]?.addEventListener('ended', () => {
    if (stream === opened) stopCamera();
  });
  // Con permiso ya hay etiquetas: se actualiza la lista y se recuerda la cámara elegida.
  await refreshDevices();
  const track = opened.getVideoTracks()[0];
  const settings = track?.getSettings();
  const devices = useStore.getState().cameraDevices;
  const used = pickDevice(devices, { id: settings?.deviceId, label: track?.label });
  useStore.getState().setCameraState({ cameraStatus: 'live', cameraProblem: null });
  if (used && (used.id !== prefs.device_id || used.label !== prefs.device_label)) {
    useStore.getState().setCameraPrefs({ device_id: used.id, device_label: used.label });
    void persistCameraSetup();
  }
  scheduleNext();
}

function stopStream(): void {
  if (video && 'cancelVideoFrameCallback' in video) video.cancelVideoFrameCallback(frameCallback);
  cancelAnimationFrame(rafId);
  stream?.getTracks().forEach((tr) => tr.stop());
  stream = null;
  if (video) video.srcObject = null;
}

export function stopCamera(): void {
  startToken++;
  stopStream();
  const s = useStore.getState();
  // Un cuadro congelado conserva su tamaño: sigue sirviendo de fondo con la cámara apagada.
  s.setCameraState({ cameraStatus: 'off', cameraProblem: null, ...(s.cameraFrozen ? {} : { cameraStreamSize: null }) });
  publishBackdrop(true);
}

/** Congela el cuadro actual (queda como fondo aunque se apague la cámara) o lo suelta. */
export function setFrozen(frozen: boolean): void {
  const s = useStore.getState();
  if (frozen) {
    const src = ensureCorrector().canvas;
    if (src.width === 0) return;
    snapshot = document.createElement('canvas');
    snapshot.width = src.width;
    snapshot.height = src.height;
    snapshot.getContext('2d')?.drawImage(src, 0, 0);
  } else {
    snapshot = null;
  }
  s.setCameraState({ cameraFrozen: frozen });
}

/**
 * Cuadro crudo (sin corregir) reducido a `maxWidth` como PNG en base64, para detectar el tablero.
 * Devuelve también el factor de escala para volver a coordenadas del video.
 */
export async function grabRawPng(maxWidth: number): Promise<{ png: string; scale: number; width: number; height: number } | null> {
  const v = ensureVideo();
  if (v.readyState < 2 || v.videoWidth === 0) return null;
  const scale = Math.min(1, maxWidth / v.videoWidth);
  const w = Math.round(v.videoWidth * scale);
  const h = Math.round(v.videoHeight * scale);
  const cv = document.createElement('canvas');
  cv.width = w;
  cv.height = h;
  const ctx = cv.getContext('2d');
  if (!ctx) return null;
  ctx.drawImage(v, 0, 0, w, h);
  const blob = await new Promise<Blob | null>((res) => cv.toBlob(res, 'image/png'));
  if (!blob) return null;
  return { png: await blobToBase64(blob), scale, width: v.videoWidth, height: v.videoHeight };
}

/**
 * El cuadro corregido de lente que se ve (el congelado, o el actual) como PNG en base64, con su
 * tamaño en píxeles: es la imagen sobre la que se mide la alineación, así que sus coordenadas son
 * las de `CameraAlignment`. `null` si aún no hay imagen.
 */
export async function grabCorrectedPng(): Promise<{ png: string; width: number; height: number } | null> {
  const frozen = useStore.getState().cameraFrozen;
  const src = frozen && snapshot ? snapshot : corrector?.canvas;
  if (!src || src.width === 0 || src.height === 0) return null;
  const blob = await new Promise<Blob | null>((res) => src.toBlob(res, 'image/png'));
  if (!blob) return null;
  return { png: await blobToBase64(blob), width: src.width, height: src.height };
}

export function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onerror = () => reject(r.error);
    r.onload = () => resolve(String(r.result).split(',', 2)[1] ?? '');
    r.readAsDataURL(blob);
  });
}
