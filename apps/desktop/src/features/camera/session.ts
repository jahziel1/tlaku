/**
 * Sesión de cámara: una sola para toda la app. Vive fuera de React para que la cámara y el fondo
 * del lienzo sigan activos aunque el usuario cambie de pestaña del panel.
 *
 * Flujo por cuadro: video -> textura WebGL -> shader de Brown-Conrady -> canvas corregido. Nada de
 * video cruza el IPC. El fondo del lienzo comparte ese canvas (`preserveDrawingBuffer`) y se
 * republica a ~12 fps como máximo.
 */
import { getApi } from '@/api';
import { errorMessage } from '@/lib/errors';
import { getPref, useStore } from '@/store';
import type { CameraBackdrop } from '@/store/camera';
import { adjustMatrix, adjustOf } from './adjust';
import { backdropCorners, type Quad } from './geometry';
import { headCorners } from './head';
import { selectHeadDoc } from '@/store/machine';
import { buildConstraints, classifyCameraError, pickDevice, videoInputs, type CameraProblem } from './capture';
import { createCorrector, type Corrector } from './corrector';
import { toBedAt } from './height';
import { correctedSize, lensFitsStream } from './lens';
import { persistCameraSetup } from './actions';
import { drawNetworkFrame, runNetworkLoop } from './netSource';
import { parseNetworkUrl } from './networkUrl';

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
/** Cámara de red en marcha: su canvas de origen (reemplaza al video) y la URL. */
let net: { canvas: HTMLCanvasElement; url: string } | null = null;
/** Mosaico de la cámara del cabezal (fondo fijo) con sus esquinas en la cama. */
let mosaic: { canvas: HTMLCanvasElement; corners: Quad } | null = null;
/** Esquinas del cuadro congelado cuando la cámara va en el cabezal (la imagen ya no se mueve). */
let frozenCorners: Quad | null = null;

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

/** ¿Hay una fuente de imagen en marcha (cámara local o de red)? */
function hasSource(): boolean {
  return stream !== null || net !== null;
}

/** Cuadro crudo disponible: el video, o el canvas donde se dibujan los cuadros de la cámara de red. */
function rawSource(): { el: HTMLVideoElement | HTMLCanvasElement; w: number; h: number } | null {
  if (net) return net.canvas.width > 0 ? { el: net.canvas, w: net.canvas.width, h: net.canvas.height } : null;
  const v = ensureVideo();
  return v.readyState < 2 || v.videoWidth === 0 ? null : { el: v, w: v.videoWidth, h: v.videoHeight };
}

/** La vista cruda para el asistente de calibración: el video, o el canvas de la cámara de red. */
export function getRawView(): HTMLVideoElement | HTMLCanvasElement {
  return net ? net.canvas : ensureVideo();
}

/** Devuelve la vista cruda a su sitio (al cerrar el asistente). */
export function parkRawView(): void {
  if (net) net.canvas.remove();
  parkVideo();
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

/** Dibuja el cuadro actual del video corregido; `gray` pasa el resultado a escala de grises. */
function drawFrame(gray: boolean): boolean {
  const src = rawSource();
  if (!src) return false;
  const s = useStore.getState();
  const size: [number, number] = [src.w, src.h];
  const lens = s.cameraSetup.calibration;
  const useLens = lens && lensFitsStream(lens, size[0], size[1]) ? lens : null;
  const maxW = getPref('cameraFrameWidth');
  const [w, h] = useLens ? correctedSize(useLens, maxW) : [Math.min(size[0], maxW), Math.round((Math.min(size[0], maxW) * size[1]) / size[0])];
  const t0 = performance.now();
  ensureCorrector().render(src.el, { width: w, height: h, lens: useLens, gray });
  stats.renderMs += performance.now() - t0;
  stats.frames++;
  return true;
}

function renderFrame(): void {
  const src = rawSource();
  if (!src) return;
  const s = useStore.getState();
  const size: [number, number] = [src.w, src.h];
  if (!s.cameraStreamSize || s.cameraStreamSize[0] !== size[0] || s.cameraStreamSize[1] !== size[1]) {
    s.setCameraState({ cameraStreamSize: size });
  }
  if (s.cameraFrozen) return;
  if (!drawFrame(s.cameraSetup.prefs.grayscale === true)) return;
  frameCount++;
  publishBackdrop(false);
}

/** Cuadros dibujados hasta ahora (sirve para esperar al siguiente). */
export function framesRendered(): number {
  return frameCount;
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
  if (s.cameraSetup.head?.enabled) return publishHeadBackdrop(force);
  const { alignment, prefs } = s.cameraSetup;
  const source = s.cameraFrozen && snapshot ? snapshot : hasSource() && corrector ? corrector.canvas : null;
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
    // La homografía vale a la altura del material actual (corrección por grosor), con el ajuste manual.
    corners: backdropCorners(adjustMatrix(toBedAt(alignment, prefs.material_height_mm ?? 0), alignment.image_size, adjustOf(prefs)), alignment.image_size),
    opacity: prefs.opacity,
    frame: ++frameCount,
  };
  s.setCameraBackdrop(backdrop);
}

/**
 * Fondo con la cámara en el cabezal: el mosaico si ya se armó (fijo en la cama) o, si no, el cuadro
 * en vivo, que se mueve con el láser (sus esquinas salen de la calibración del cabezal y de la
 * posición actual de la máquina).
 */
function publishHeadBackdrop(force: boolean): void {
  const s = useStore.getState();
  const { prefs, head } = s.cameraSetup;
  const cal = head?.calibration ?? null;
  const hide = () => {
    if (s.cameraBackdrop) s.setCameraBackdrop(null);
  };
  if (!prefs.visible || !cal) return hide();
  const now = performance.now();
  if (!force && now - lastPublish < BACKDROP_MIN_INTERVAL_MS) return;
  if (mosaic) {
    lastPublish = now;
    s.setCameraBackdrop({
      source: mosaic.canvas,
      width: mosaic.canvas.width,
      height: mosaic.canvas.height,
      corners: mosaic.corners,
      opacity: prefs.opacity,
      frame: ++frameCount,
    });
    return;
  }
  const source = s.cameraFrozen && snapshot ? snapshot : hasSource() && corrector ? corrector.canvas : null;
  const space = correctedSpace();
  const current = !!space && cal.image_size[0] === space[0] && cal.image_size[1] === space[1];
  const laser = selectHeadDoc(s);
  const corners = s.cameraFrozen && frozenCorners ? frozenCorners : laser ? headCorners(cal, laser) : null;
  if (!source || !current || !corners || (!s.cameraFrozen && !lensOk())) return hide();
  lastPublish = now;
  const canvas = source as HTMLCanvasElement;
  s.setCameraBackdrop({ source, width: canvas.width, height: canvas.height, corners, opacity: prefs.opacity, frame: ++frameCount });
}

/** Deja el mosaico armado como fondo fijo en la cama (`null` lo quita y vuelve el cuadro en vivo). */
export function setMosaicBackdrop(value: { canvas: HTMLCanvasElement; corners: Quad } | null): void {
  mosaic = value;
  publishBackdrop(true);
}

export function hasMosaic(): boolean {
  return mosaic !== null;
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
    // Con la cámara en el cabezal el cuadro en vivo sigue al láser: cada reporte de posición reubica el fondo.
    else if (s.status !== prev.status && s.cameraSetup.head?.enabled && !mosaic) publishBackdrop(false);
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

function setProblem(problem: CameraProblem, detail: string | null = null): void {
  useStore.getState().setCameraState({ cameraStatus: 'off', cameraProblem: problem, cameraProblemDetail: detail });
}

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/**
 * Enciende la cámara de red: un bucle pide cuadros al motor y los dibuja en un canvas que hace de
 * "video" para el resto de la sesión. El primer fallo (URL, clave, fuera de la red local) se avisa
 * de inmediato.
 */
function startNetwork(token: number): void {
  const s = useStore.getState();
  const parsed = parseNetworkUrl(s.cameraSetup.prefs.network_url ?? '');
  if (!parsed.ok) return setProblem('noUrl');
  s.setCameraState({ cameraStatus: 'starting', cameraProblem: null, cameraProblemDetail: null });
  const mine = { canvas: document.createElement('canvas'), url: parsed.url };
  net = mine;
  const current = () => token === startToken && net === mine;
  void runNetworkLoop(
    parsed.url,
    { fetchFrame: (url) => getApi().fetchCameraFrame({ url }), decode: (f) => drawNetworkFrame(f, mine.canvas), sleep },
    {
      onFrame: (_size, first) => {
        if (first) useStore.getState().setCameraState({ cameraStatus: 'live', cameraProblem: null, cameraProblemDetail: null });
        renderFrame();
      },
      onFail: (message) => {
        if (!current()) return;
        net = null;
        setProblem('network', message);
        publishBackdrop(true);
      },
    },
    current,
  ).catch((e) => {
    if (current()) {
      net = null;
      setProblem('network', errorMessage(e));
    }
  });
}

/** Enciende la cámara con las preferencias guardadas. Los problemas quedan en el store. */
export async function startCamera(): Promise<void> {
  subscribeOnce();
  const token = ++startToken;
  const s = useStore.getState();
  stopStream();
  const prefs = s.cameraSetup.prefs;
  if (prefs.source === 'network') return startNetwork(token);
  if (!navigator.mediaDevices?.getUserMedia) return setProblem('unsupported');
  s.setCameraState({ cameraStatus: 'starting', cameraProblem: null, cameraProblemDetail: null });
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
  net = null;
  if (video) video.srcObject = null;
}

/** Cuadro corregido actual como canvas (copia), para armar mosaicos y fondos. */
export function currentCanvas(): HTMLCanvasElement | null {
  const src = useStore.getState().cameraFrozen && snapshot ? snapshot : corrector?.canvas;
  return src && src.width > 0 ? src : null;
}

export function stopCamera(): void {
  startToken++;
  stopStream();
  const s = useStore.getState();
  // Un cuadro congelado conserva su tamaño: sigue sirviendo de fondo con la cámara apagada.
  s.setCameraState({ cameraStatus: 'off', cameraProblem: null, cameraProblemDetail: null, ...(s.cameraFrozen ? {} : { cameraStreamSize: null }) });
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
    const { cameraSetup } = s;
    const laser = selectHeadDoc(s);
    frozenCorners = cameraSetup.head?.enabled && cameraSetup.head.calibration && laser ? headCorners(cameraSetup.head.calibration, laser) : null;
  } else {
    snapshot = null;
    frozenCorners = null;
  }
  s.setCameraState({ cameraFrozen: frozen });
}

/**
 * Cuadro crudo (sin corregir) reducido a `maxWidth` como PNG en base64, para detectar el tablero.
 * Devuelve también el factor de escala para volver a coordenadas del video.
 */
export async function grabRawPng(maxWidth: number): Promise<{ png: string; scale: number; width: number; height: number } | null> {
  const src = rawSource();
  if (!src) return null;
  const scale = Math.min(1, maxWidth / src.w);
  const w = Math.round(src.w * scale);
  const h = Math.round(src.h * scale);
  const cv = document.createElement('canvas');
  cv.width = w;
  cv.height = h;
  const ctx = cv.getContext('2d');
  if (!ctx) return null;
  ctx.drawImage(src.el, 0, 0, w, h);
  const blob = await new Promise<Blob | null>((res) => cv.toBlob(res, 'image/png'));
  if (!blob) return null;
  return { png: await blobToBase64(blob), scale, width: src.w, height: src.h };
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

/**
 * Como `grabCorrectedPng` pero siempre a color: con el fondo en escala de grises vuelve a dibujar
 * el cuadro actual sin gris, lo guarda y restaura el gris. Es lo que se guarda como imagen del
 * documento (el gris es solo para ver mejor el diseño encima).
 */
export async function grabCorrectedColorPng(): Promise<{ png: string; width: number; height: number } | null> {
  const s = useStore.getState();
  const gray = s.cameraSetup.prefs.grayscale === true;
  if (!gray || s.cameraFrozen || !hasSource()) return grabCorrectedPng();
  if (!drawFrame(false)) return grabCorrectedPng();
  const pending = grabCorrectedPng();
  drawFrame(true);
  return pending;
}

/**
 * Renueva el cuadro congelado con uno nuevo: suelta, espera al siguiente cuadro de la cámara y
 * vuelve a congelar. Sin cámara encendida no hace nada.
 */
export async function refreshSnapshot(timeoutMs = 2000): Promise<boolean> {
  const s = useStore.getState();
  if (!s.cameraFrozen || s.cameraStatus !== 'live') return false;
  const before = frameCount;
  setFrozen(false);
  const t0 = performance.now();
  while (frameCount === before && performance.now() - t0 < timeoutMs) await new Promise((r) => setTimeout(r, 40));
  const fresh = frameCount !== before;
  setFrozen(true);
  return fresh;
}

export function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onerror = () => reject(r.error);
    r.onload = () => resolve(String(r.result).split(',', 2)[1] ?? '');
    r.readAsDataURL(blob);
  });
}
