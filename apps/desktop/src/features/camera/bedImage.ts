/**
 * Captura de la cama (M35): la foto corregida de lente y alineada se guarda como imagen del
 * documento, colocada exactamente donde está en la cama, y de ahí se puede vectorizar.
 *
 * Con la cámara fija es una sola foto; con la cámara en el cabezal es el mosaico de un recorrido
 * (el que arma `headScan.ts`).
 */
import { getApi } from '@/api';
import type { BedImage } from '@/api/types';
import { applyBitmapToDoc, projectDir, projectStem } from '@/features/trace/bitmap';
import { useTraceDialog } from '@/features/trace/traceStore';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { selectActiveDevice, useStore, type AppState } from '@/store';
import { currentMapping, matrixForFrame, MAPPING_PROBLEM_KEY } from './mapping';
import { grabCorrectedColorPng } from './session';
import { scanBed, ScanError } from './headScan';
import { useHeadUi, type HeadAction } from './headState';
import { refreshOverlayNow } from './refresh';

const app = () => useStore.getState();

/** Resolución (px/mm) con que se guarda la imagen de la cama: la de la foto, con tope. */
export const CAPTURE_PX_PER_MM = null;

/**
 * Pone la imagen en el documento (un solo paso del historial), la selecciona y devuelve su id.
 * Si la capa de imágenes no existe, la crea.
 */
export function insertBedImage(image: BedImage): string {
  const s = app();
  const out: { id: string | null } = { id: null };
  s.applyDocChange('camera.capture.history', (doc) => {
    const r = applyBitmapToDoc(doc, image, { sourceIds: [], replace: false });
    out.id = r.id;
    return r.doc;
  });
  if (out.id) s.select([out.id]);
  void s.computePreview();
  return out.id ?? '';
}

/** Foto fija: una sola imagen alineada con la cama. Lanza un error con texto para el usuario. */
async function captureFixed(): Promise<BedImage> {
  const s = app();
  const m = currentMapping(s);
  if (!m.ok) throw new Error(t(MAPPING_PROBLEM_KEY[m.reason]));
  const frame = await grabCorrectedColorPng();
  if (!frame) throw new Error(t('camera.capture.noImage'));
  return getApi().composeBedImage({
    tiles: [{ image_png_base64: frame.png, to_bed: matrixForFrame(m.matrix, m.space, [frame.width, frame.height]) }],
    bed: selectActiveDevice(s).bed,
    px_per_mm: CAPTURE_PX_PER_MM,
    grayscale: s.cameraSetup.prefs.grayscale === true,
    feather_px: null,
    crop_px: null,
    rect: null,
    dir: projectDir(s.filePath),
    stem: projectStem(s.filePath, t('doc.untitled')),
    save: true,
  });
}

/**
 * "Capturar imagen de la cama": guarda la foto corregida y alineada como imagen del documento.
 * Con la cámara en el cabezal hace el recorrido completo (el usuario ya confirmó el movimiento).
 * Devuelve el id de la imagen nueva o `null` si no se pudo o se canceló.
 */
export async function captureBedImage(): Promise<string | null> {
  const s = app();
  try {
    let image: BedImage;
    if (s.cameraSetup.head?.enabled) {
      const r = await scanBed({ save: true, useSelection: useHeadUi.getState().useSelection });
      if (!r) {
        s.notify('info', t('camera.head.scan.cancelled'));
        return null;
      }
      image = r.image;
      if (r.partial) s.notify('warning', t('camera.head.scan.partial'));
    } else {
      image = await captureFixed();
    }
    const id = insertBedImage(image);
    s.notify('success', t('camera.capture.done', { w: image.width_px, h: image.height_px, path: image.path }));
    if (image.in_temp_dir) s.notify('warning', t('camera.capture.temp'));
    return id;
  } catch (e) {
    const msg = e instanceof ScanError ? e.message : t('camera.capture.failed', { error: errorMessage(e) });
    useHeadUi.getState().setError(msg);
    s.notify('error', msg);
    return null;
  }
}

/** "Vectorizar desde la cámara": captura la cama y abre el diálogo de vectorizar sobre la imagen. */
export async function vectorizeFromCamera(): Promise<boolean> {
  const id = await captureBedImage();
  if (!id) return false;
  useTraceDialog.getState().open(id);
  return true;
}

/** ¿Se puede capturar ahora? `null` si sí; si no, la clave del motivo. */
export function captureBlockedReason(s: AppState = app()): I18nKey | null {
  if (s.cameraStatus !== 'live' && !s.cameraFrozen) return 'camera.reason.notLive';
  const m = currentMapping(s);
  return m.ok ? null : MAPPING_PROBLEM_KEY[m.reason];
}

/**
 * ¿Se puede actualizar el fondo? Con la cámara fija solo tiene sentido con el cuadro congelado (en
 * vivo ya se actualiza solo); con la cámara en el cabezal hace falta la calibración y la máquina.
 */
export function overlayBlockedReason(s: AppState = app()): I18nKey | null {
  if (s.cameraSetup.head?.enabled) {
    if (s.cameraStatus !== 'live') return 'camera.reason.notLive';
    const m = currentMapping(s);
    return m.ok ? null : MAPPING_PROBLEM_KEY[m.reason];
  }
  return s.cameraFrozen && s.cameraStatus === 'live' ? null : 'camera.reason.overlayLive';
}

/**
 * Pide una acción de la cámara desde un comando o un botón. Con la cámara en el cabezal el
 * recorrido mueve la máquina: se abre el panel de Cámara y se pide confirmación antes.
 */
export function requestAction(kind: HeadAction): void {
  const s = app();
  s.setDockTab('camera');
  if (s.cameraSetup.head?.enabled) useHeadUi.getState().ask(kind);
  else void runAction(kind);
}

/** Ejecuta la acción (ya confirmada si hacía falta). */
export async function runAction(kind: HeadAction): Promise<void> {
  useHeadUi.getState().ask(null);
  if (kind === 'capture') await captureBedImage();
  else if (kind === 'vectorize') await vectorizeFromCamera();
  else await refreshOverlayNow();
}
