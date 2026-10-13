/** Lectura de un `MarksReport` para mostrarlo: calificación, tono de cada marca y marcas faltantes. */
import type { MarkResult, MarksReport } from '@/api/types';
import { applyH, invertH, type Vec2 } from './geometry';

export type MarkGrade = 'excellent' | 'good' | 'fair' | 'poor';

/** Calificación de un error en mm: menos de 0,3 excelente, 0,6 buena, 1 aceptable; más, mala. */
export function rateMm(mm: number): MarkGrade {
  if (mm < 0.3) return 'excellent';
  if (mm < 0.6) return 'good';
  if (mm < 1) return 'fair';
  return 'poor';
}

/**
 * Error que se usa para calificar la alineación: el peor error de validación cruzada si se pudo
 * medir (es el que no se ajusta al azar) y, si no, el mayor residuo del ajuste.
 */
export function reportError(r: MarksReport): number {
  return r.loo_max_mm ?? r.max_residual_mm;
}

export const reportGrade = (r: MarksReport): MarkGrade => rateMm(reportError(r));

export type MarkTone = 'ok' | 'warn' | 'bad' | 'missing';

/** Cómo se dibuja una marca: no encontrada, descartada, con error alto o buena. */
export function markTone(m: MarkResult): MarkTone {
  if (!m.image) return 'missing';
  if (!m.used) return 'bad';
  const e = Math.max(m.residual_mm ?? 0, m.loo_mm ?? 0);
  return e > 0.6 ? 'warn' : 'ok';
}

/** Dónde deberían estar en la imagen las marcas no encontradas, según el ajuste (px). */
export function missingMarkPositions(r: MarksReport): { index: number; at: Vec2 }[] {
  const toImage = invertH(r.to_bed);
  if (!toImage) return [];
  return r.marks.filter((m) => !m.image).map((m) => ({ index: m.index, at: applyH(toImage, m.bed) }));
}

/**
 * Pasa un reporte medido sobre una imagen de otro tamaño al espacio de la alineación (píxeles de la
 * imagen corregida a la resolución de la calibración). `k` es la razón de tamaños (espacio / foto).
 * Los errores en mm no cambian; las posiciones y la homografía píxel -> mm sí.
 */
export function rescaleReport(r: MarksReport, k: number): MarksReport {
  if (k === 1) return r;
  const up = (p: [number, number]): [number, number] => [p[0] * k, p[1] * k];
  const m = r.to_bed.map((row) => [row[0] / k, row[1] / k, row[2]]) as MarksReport['to_bed'];
  return {
    ...r,
    marks: r.marks.map((x) => ({ ...x, image: x.image ? up(x.image) : null })),
    candidates: r.candidates.map(up),
    to_bed: m,
    scale_px_per_mm: r.scale_px_per_mm * k,
  };
}
