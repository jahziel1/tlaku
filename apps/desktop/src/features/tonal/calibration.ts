/**
 * Calibración del tono con la prueba de materiales: de la cuadrícula del formulario (`TestForm`) sale
 * la dosis de cada celda; el usuario indica a qué punto de la curva se parece cada celda quemada y
 * `fitCurve` ajusta la curva del material. Lógica pura.
 */
import { axisValues, type FixedValues, type TestForm } from '@/features/materials/testPlan';
import { fitCurve, toneDarkness, toneDose, type ToneCurve, type ToneSample } from './curve';

export interface GridCell {
  row: number;
  col: number;
  speed: number;
  power: number;
  passes: number;
  interval: number;
  /** Dosis que recibe la celda con el ancho de punto dado. */
  dose: number;
}

/** Valores de una celda: los fijos del formulario con los que varía cada eje. */
export function cellValues(form: TestForm, rowValue: number, colValue: number): FixedValues {
  const v: FixedValues = { ...form.fixed };
  v[form.rows.param] = rowValue;
  v[form.cols.param] = colValue;
  return v;
}

/**
 * Dosis de una celda. En relleno el ancho es el intervalo (la dosis media del área); en línea, el
 * ancho del punto del láser.
 */
export function cellDose(form: Pick<TestForm, 'mode'>, v: FixedValues, spotMm: number): number {
  return toneDose(v.power, v.speed, v.passes, form.mode === 'fill' ? v.interval : spotMm);
}

/** Todas las celdas de la cuadrícula, por filas. */
export function gridCells(form: TestForm, spotMm: number): GridCell[][] {
  const rows = axisValues(form.rows);
  const cols = axisValues(form.cols);
  return rows.map((rv, row) =>
    cols.map((cv, col) => {
      const v = cellValues(form, rv, cv);
      return { row, col, ...v, dose: cellDose(form, v, spotMm) };
    }),
  );
}

/** Oscuridad que la curva predice para cada celda. */
export function predictedDarkness(cells: GridCell[][], curve: ToneCurve): number[][] {
  return cells.map((r) => r.map((c) => toneDarkness(curve, c.dose)));
}

/** Clave de una celda en el mapa de observaciones. */
export const cellKey = (row: number, col: number) => `${row},${col}`;

/** Observaciones del usuario (`clave de celda` -> oscuridad 0..1) convertidas en muestras de la curva. */
export function samplesOf(cells: GridCell[][], marks: Readonly<Record<string, number>>): ToneSample[] {
  const out: ToneSample[] = [];
  for (const r of cells) {
    for (const c of r) {
      const d = marks[cellKey(c.row, c.col)];
      if (d !== undefined && Number.isFinite(d) && c.dose > 0) out.push({ dose: c.dose, darkness: Math.min(1, Math.max(0, d)) });
    }
  }
  return out;
}

/** Curva ajustada a las observaciones (la misma si no hay ninguna). */
export function fitFromMarks(curve: ToneCurve, cells: GridCell[][], marks: Readonly<Record<string, number>>): ToneCurve {
  return fitCurve(curve, samplesOf(cells, marks));
}

/** Observaciones guardadas (muestras) puestas sobre las celdas de la cuadrícula actual, por dosis más cercana. */
export function marksFromSamples(cells: GridCell[][], samples: readonly ToneSample[]): Record<string, number> {
  const out: Record<string, number> = {};
  const flat = cells.flat();
  for (const s of samples) {
    let best: GridCell | null = null;
    let bestErr = Infinity;
    for (const c of flat) {
      const err = Math.abs(Math.log((c.dose || 1e-9) / (s.dose || 1e-9)));
      if (err < bestErr) {
        best = c;
        bestErr = err;
      }
    }
    if (best && bestErr < 1e-3) out[cellKey(best.row, best.col)] = s.darkness;
  }
  return out;
}
