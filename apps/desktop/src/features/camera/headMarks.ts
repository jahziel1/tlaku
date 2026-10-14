/**
 * Alineación automática de la cámara del cabezal con marcas grabadas (M36). Se graba un cuadro de
 * 9 marcas (cruces) en un lugar conocido de la cama y se acerca el cabezal hasta verlas; el motor
 * (`tlaku-camera`, `marks/`) las detecta en la foto y cada una da una observación: con el láser en
 * `L` la marca de posición conocida `M` se ve en el píxel `p`, y `M = L + offset + A (p - ref)`.
 * Una sola foto con 4 o más marcas basta para el desfase y la escala; varias posiciones afinan.
 *
 * El cuadro de marcas es el mismo del motor (`camera_marks`) con su tamaño propio, movido a su
 * origen en la cama. El círculo de la primera marca fija la orientación: debe verse en la foto.
 */
import { getApi } from '@/api';
import type { Document, HeadCalibration, MarksReport } from '@/api/types';
import { ensureSaved } from '@/features/history/files';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { selectActiveDevice, useStore } from '@/store';
import { selectHeadDoc } from '@/store/machine';
import { applyHeadCalibration } from './actions';
import type { Vec2 } from './geometry';
import { fitHeadCalibration, type HeadFit, type HeadFitError, type HeadObservation } from './head';
import {
  HEAD_MARKS_MAX_SIZE,
  HEAD_MARKS_MIN_SIZE,
  HEAD_MARKS_MIN_USED,
  useHeadMarks,
} from './headMarksState';
import { rescaleReport } from './marksView';
import { correctedSpace, grabCorrectedPng } from './session';

const app = () => useStore.getState();

export const clampMarksSize = (v: number): number => Math.min(HEAD_MARKS_MAX_SIZE, Math.max(HEAD_MARKS_MIN_SIZE, v));

/** Esquina del cuadro: la elegida, o la que lo deja centrado en la cama (sin salirse de ella). */
export function marksOrigin(size: number, origin: Vec2 | null, bed: readonly [number, number]): Vec2 {
  if (origin) return origin;
  return [Math.max(0, Math.round((bed[0] - size) / 2)), Math.max(0, Math.round((bed[1] - size) / 2))];
}

/** Mueve todo el documento `(dx, dy)` mm (las marcas nacen en el origen). */
export function shiftDocument(doc: Document, dx: number, dy: number): Document {
  return {
    ...doc,
    objects: doc.objects.map((o) => {
      const tr = o.transform;
      return { ...o, transform: [tr[0], tr[1], tr[2], tr[3], tr[4] + dx, tr[5] + dy] };
    }),
  };
}

/** Observaciones de una detección: cada marca usada, con su posición absoluta en la cama. */
export function observationsFromReport(report: MarksReport, laser: Vec2, origin: Vec2): HeadObservation[] {
  const out: HeadObservation[] = [];
  for (const m of report.marks) {
    if (!m.used || !m.image) continue;
    out.push({ laser, px: [m.image[0], m.image[1]], mark: [m.bed[0] + origin[0], m.bed[1] + origin[1]] });
  }
  return out;
}

/** Abre el cuadro de marcas como documento nuevo (pregunta antes si hay cambios sin guardar). */
export async function openHeadMarksAsDocument(): Promise<boolean> {
  const ui = useHeadMarks.getState();
  const bed = selectActiveDevice(app()).bed;
  const size = clampMarksSize(ui.size);
  const origin = marksOrigin(size, ui.origin, bed);
  if (origin[0] + size > bed[0] + 1e-6 || origin[1] + size > bed[1] + 1e-6) {
    app().notify('error', t('camera.head.marks.outside', { size, w: bed[0], h: bed[1] }));
    return false;
  }
  let doc: Document;
  try {
    doc = await getApi().cameraMarks({ bed: [size, size], count: 9 });
  } catch (e) {
    app().notify('error', t('camera.marks.err.generate', { error: errorMessage(e) }));
    return false;
  }
  if (!(await ensureSaved('new'))) return false;
  ui.setOrigin(origin);
  const s = app();
  s.replaceDocument({ ...shiftDocument(doc, origin[0], origin[1]), device_id: s.doc.device_id }, null);
  s.fitContent();
  void s.computePreview();
  s.notify('success', t('camera.head.marks.opened', { size, x: origin[0], y: origin[1] }));
  return true;
}

export type HeadMarksProblem = 'noPosition' | 'noSpace' | 'noImage' | 'tooFew';

export class HeadMarksError extends Error {
  constructor(
    readonly kind: HeadMarksProblem,
    message: string,
  ) {
    super(message);
  }
}

/**
 * Toma la imagen que se ve, detecta las marcas y suma sus observaciones. Devuelve el reporte.
 * Lanza `HeadMarksError` (con texto para el usuario) si no se puede o si hay muy pocas marcas.
 */
export async function detectHeadMarks(): Promise<MarksReport> {
  const laser = selectHeadDoc(app());
  if (!laser) throw new HeadMarksError('noPosition', t('camera.mapping.noHeadPosition'));
  const space = correctedSpace();
  if (!space) throw new HeadMarksError('noSpace', t('camera.mapping.noSpace'));
  const frame = await grabCorrectedPng();
  if (!frame) throw new HeadMarksError('noImage', t('camera.marks.noImage'));
  const ui = useHeadMarks.getState();
  const size = clampMarksSize(ui.size);
  const origin = marksOrigin(size, ui.origin, selectActiveDevice(app()).bed);
  const raw = await getApi().detectCameraMarks({ image_png_base64: frame.png, bed: [size, size], count: 9, prior_to_bed: null });
  const report = rescaleReport(raw, space[0] / frame.width);
  const obs = observationsFromReport(report, laser, origin);
  if (obs.length < HEAD_MARKS_MIN_USED) {
    throw new HeadMarksError('tooFew', t('camera.head.marks.tooFew', { found: report.found, used: obs.length, min: HEAD_MARKS_MIN_USED }));
  }
  ui.add(obs, { laser, found: report.found, used: obs.length });
  return report;
}

/** Ajuste con todas las observaciones acumuladas. */
export function currentMarksFit(): HeadFit | { error: HeadFitError | 'noSpace' } {
  const space = correctedSpace();
  if (!space) return { error: 'noSpace' };
  return fitHeadCalibration(useHeadMarks.getState().obs, null, space);
}

/** Detecta y deja el aviso en el store si falla. Devuelve si sumó observaciones. */
export async function runDetectHeadMarks(): Promise<boolean> {
  const ui = useHeadMarks.getState();
  ui.setBusy(true);
  try {
    await detectHeadMarks();
    return true;
  } catch (e) {
    ui.setError(errorMessage(e));
    return false;
  } finally {
    ui.setBusy(false);
  }
}

/** Guarda la calibración sacada de las marcas, enciende el modo de cabezal y limpia lo acumulado. */
export function saveHeadMarksCalibration(cal: HeadCalibration): void {
  applyHeadCalibration(cal);
  useHeadMarks.getState().reset();
  app().notify('success', t('camera.head.cal.saved', { x: cal.offset_mm[0].toFixed(2), y: cal.offset_mm[1].toFixed(2) }));
}
