/**
 * Corrección por altura del material en la interfaz.
 *
 * El motor guarda en la alineación un modelo `H(h) = [H1 H2 (H3 + (h - ref)·G)]` de la cama a la
 * imagen (ver tlaku-camera/src/height.rs). Aquí solo se evalúa: la homografía píxel -> mm a la
 * altura del material actual, con la que se dibuja el fondo del lienzo, y los textos de estado.
 */
import type { AlignmentSample, CameraAlignment, HeightModel, MarksReport } from '@/api/types';
import { applyH, invertH, type Mat3, type Quad, type Vec2 } from './geometry';

/** Diferencia mínima (mm) entre alturas distintas guardadas; igual que en el motor. */
export const MIN_HEIGHT_GAP_MM = 2;
/** Alturas dentro de este margen (mm) cuentan como la misma: la captura nueva reemplaza a la vieja. */
export const SAME_HEIGHT_MM = 0.25;

/** Matriz cama (mm) -> imagen (px) del plano a altura `h`. */
export function bedToImageAt(model: HeightModel, h: number): Mat3 {
  const dh = h - model.ref_height_mm;
  const m = model.bed_to_image.map((row) => [...row]) as Mat3;
  for (let r = 0; r < 3; r++) m[r]![2] = m[r]![2]! + dh * model.gradient[r]!;
  return m;
}

/** Homografía píxel -> mm del plano a altura `h`. Sin modelo de altura, la guardada (`to_bed`). */
export function toBedAt(a: CameraAlignment, h: number): Mat3 {
  if (!a.height) return a.to_bed;
  const inv = invertH(bedToImageAt(a.height, h));
  if (!inv) return a.to_bed;
  const s = inv[2][2];
  if (!Number.isFinite(s) || Math.abs(s) < 1e-300) return a.to_bed;
  return inv.map((r) => r.map((v) => v / s)) as Mat3;
}

export type HeightStatus =
  /** No hay forma de corregir: la alineación vale solo para la altura de las marcas. */
  | { kind: 'none' }
  /** Estimado con la matriz de la lente a partir de una sola alineación. */
  | { kind: 'modelled' }
  /** Medido con marcas a dos o más alturas; `outside` son los mm fuera del rango medido. */
  | { kind: 'measured'; min: number; max: number; outside: number };

export function heightStatus(a: CameraAlignment | null, h: number): HeightStatus {
  const m = a?.height;
  if (!a || !m) return { kind: 'none' };
  if (m.source === 'modelled') return { kind: 'modelled' };
  const outside = h < m.min_height_mm ? m.min_height_mm - h : h > m.max_height_mm ? h - m.max_height_mm : 0;
  return { kind: 'measured', min: m.min_height_mm, max: m.max_height_mm, outside };
}

/**
 * Cuánto se desplaza (mm) el contenido de la imagen al corregir por la altura `h` respecto de
 * la alineación a su altura de referencia: máximo sobre las 4 esquinas y el centro de la imagen.
 */
export function maxShiftMm(a: CameraAlignment, h: number): number {
  const base = a.to_bed;
  const now = toBedAt(a, h);
  const [w, ht] = a.image_size;
  const pts: Vec2[] = [[0, 0], [w, 0], [w, ht], [0, ht], [w / 2, ht / 2]];
  return Math.max(
    ...pts.map((p) => {
      const [x0, y0] = applyH(base, p);
      const [x1, y1] = applyH(now, p);
      return Math.hypot(x1 - x0, y1 - y0);
    }),
  );
}

/**
 * Agrega una captura a las guardadas. A la misma altura (± `SAME_HEIGHT_MM`) reemplaza la anterior;
 * una altura distinta pero a menos de `MIN_HEIGHT_GAP_MM` de otra se rechaza (no define el modelo).
 */
export function mergeSample(
  existing: readonly AlignmentSample[],
  sample: AlignmentSample,
): { samples: AlignmentSample[]; error: 'tooClose' | null } {
  const rest = existing.filter((s) => Math.abs(s.height_mm - sample.height_mm) >= SAME_HEIGHT_MM);
  if (rest.some((s) => Math.abs(s.height_mm - sample.height_mm) < MIN_HEIGHT_GAP_MM)) {
    return { samples: [...existing], error: 'tooClose' };
  }
  return { samples: [...rest, sample].sort((p, q) => p.height_mm - q.height_mm), error: null };
}

/**
 * Alineación a guardar tras aceptar una detección de marcas a la altura `height`. Se suma a las
 * capturas anteriores si son de la misma imagen. El motor recalcula `to_bed` y el modelo de altura
 * al guardar; aquí se deja una alineación coherente (con la del reporte si es la única captura).
 */
export function alignmentFromMarks(
  report: MarksReport,
  opts: { imageSize: [number, number]; height: number; existing: CameraAlignment | null },
): { alignment: CameraAlignment; error: null } | { alignment: null; error: 'tooClose' | 'tooFew' } {
  const used = report.marks.filter((m) => m.used && m.image);
  if (used.length < 4) return { alignment: null, error: 'tooFew' };
  const sample: AlignmentSample = {
    height_mm: opts.height,
    image_px: used.map((m) => [m.image![0], m.image![1]]),
    bed_mm: used.map((m) => [m.bed[0], m.bed[1]]),
    loo_rms_mm: report.loo_rms_mm,
    loo_max_mm: report.loo_max_mm,
  };
  const prev = opts.existing;
  const sameImage = prev && prev.method === 'marks' && prev.image_size[0] === opts.imageSize[0] && prev.image_size[1] === opts.imageSize[1];
  const merged = mergeSample(sameImage ? (prev.samples ?? []) : [], sample);
  if (merged.error) return { alignment: null, error: merged.error };
  // Las 4 esquinas del rectángulo de las marcas (inferior izq., inferior der., superior der., superior izq.).
  const xs = sample.bed_mm.map((p) => p[0]);
  const ys = sample.bed_mm.map((p) => p[1]);
  const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const bedCorners: Quad = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
  const toImage = invertH(report.to_bed);
  const imageCorners = (toImage ? bedCorners.map((p) => applyH(toImage, p)) : bedCorners) as Quad;
  const only = merged.samples.length === 1;
  return {
    error: null,
    alignment: {
      image_size: opts.imageSize,
      image_px: imageCorners,
      bed_mm: bedCorners,
      to_bed: only || !sameImage || !prev ? report.to_bed : prev.to_bed,
      rms_mm: report.rms_mm,
      method: 'marks',
      reference_height_mm: merged.samples[0]!.height_mm,
      samples: merged.samples,
      // El motor lo recalcula al guardar.
      height: only ? null : (prev?.height ?? null),
    },
  };
}

/** Quita la captura de una altura; sin capturas, la alineación desaparece. */
export function removeSample(a: CameraAlignment, height: number): CameraAlignment | null {
  const samples = (a.samples ?? []).filter((s) => s.height_mm !== height);
  if (samples.length === 0) return null;
  return { ...a, samples, height: samples.length === 1 ? null : (a.height ?? null), reference_height_mm: samples[0]!.height_mm };
}
