/**
 * Alineación con marcas grabadas: generar el documento de marcas, capturar la foto de la cama y
 * detectarlas con el motor. El ajuste y la precisión los calcula `tlaku-camera`; aquí solo se
 * conecta con la cámara, el documento y la configuración guardada.
 */
import { getApi } from '@/api';
import type { CameraAlignment, MarksReport } from '@/api/types';
import { ensureSaved } from '@/features/history/files';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { selectActiveDevice, useStore } from '@/store';
import { applyAlignment } from './actions';
import { alignmentFromMarks, removeSample } from './height';
import type { Mat3 } from './geometry';
import type { MarkCount } from './marksState';
import { rescaleReport } from './marksView';
import { correctedSpace, grabCorrectedPng } from './session';

const app = () => useStore.getState();

/** Tamaño de la cama de la máquina activa (mm). */
export function activeBed(): [number, number] {
  return selectActiveDevice(app()).bed;
}

/**
 * Abre las marcas como documento nuevo (pregunta antes si hay cambios sin guardar). El usuario las
 * graba con los controles normales de trabajo. Devuelve si se abrió.
 */
export async function openMarksAsNewDocument(count: MarkCount): Promise<boolean> {
  const bed = activeBed();
  let doc;
  try {
    doc = await getApi().cameraMarks({ bed, count });
  } catch (e) {
    app().notify('error', t('camera.marks.err.generate', { error: errorMessage(e) }));
    return false;
  }
  if (!(await ensureSaved('new'))) return false;
  const s = app();
  s.replaceDocument({ ...doc, device_id: s.doc.device_id }, null);
  s.fitContent();
  void s.computePreview();
  s.notify('success', t('camera.marks.opened', { n: count, w: bed[0], h: bed[1] }));
  return true;
}

/**
 * Foto de la imagen corregida que se ve y su detección, ya en el espacio de la alineación (píxeles
 * de la imagen corregida a la resolución de la calibración: la foto puede estar reducida a 1280 px
 * de ancho). Lanza un error con texto para el usuario.
 */
export async function captureAndDetect(count: MarkCount): Promise<{ report: MarksReport; png: string; size: [number, number] }> {
  const frame = await grabCorrectedPng();
  if (!frame) throw new Error(t('camera.marks.noImage'));
  const space = correctedSpace();
  const k = space ? space[0] / frame.width : 1;
  // La alineación guardada vale en el espacio de la alineación: se pasa a los píxeles de la foto enviada.
  const saved = app().cameraSetup.alignment?.to_bed ?? null;
  const prior = saved ? (saved.map((r) => [r[0] * k, r[1] * k, r[2]]) as Mat3) : null;
  const report = await getApi().detectCameraMarks({ image_png_base64: frame.png, bed: activeBed(), count, prior_to_bed: prior });
  return { report: rescaleReport(report, k), png: frame.png, size: space ?? [frame.width, frame.height] };
}

/**
 * Acepta una detección: se suma a las capturas guardadas (a esa altura) y se persiste; el motor
 * recalcula la homografía y el modelo de altura. Devuelve el motivo si no se pudo aceptar.
 */
export function acceptMarks(report: MarksReport, size: [number, number], height: number): 'tooClose' | 'tooFew' | 'noSpace' | null {
  const space = correctedSpace();
  if (!space || space[0] !== size[0] || space[1] !== size[1]) return 'noSpace';
  const r = alignmentFromMarks(report, { imageSize: size, height, existing: app().cameraSetup.alignment });
  if (r.error) return r.error;
  applyAlignment(r.alignment);
  return null;
}

/** Quita la captura de una altura de la alineación con marcas. */
export function dropHeight(a: CameraAlignment, height: number): void {
  applyAlignment(removeSample(a, height));
}
