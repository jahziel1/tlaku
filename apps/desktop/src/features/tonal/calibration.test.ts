import { describe, expect, it } from 'vitest';
import { defaultForm } from '@/features/materials/testPlan';
import { cellDose, cellKey, cellValues, fitFromMarks, gridCells, marksFromSamples, predictedDarkness, samplesOf } from './calibration';
import { presetCurve, toneDarkness } from './curve';

const form = defaultForm(null);

describe('cuadrícula de la prueba de materiales', () => {
  it('la prueba de fábrica es de 5 x 5 con potencia por filas y velocidad por columnas', () => {
    const cells = gridCells(form, 0.1);
    expect(cells).toHaveLength(5);
    expect(cells.every((r) => r.length === 5)).toBe(true);
    expect(cells[0]![0]).toMatchObject({ power: 10, speed: 500 });
    expect(cells[4]![4]).toMatchObject({ power: 100, speed: 3000 });
  });

  it('más potencia (hacia abajo) o menos velocidad (hacia la izquierda) dan más dosis', () => {
    const cells = gridCells(form, 0.1);
    for (let r = 0; r < 4; r++) expect(cells[r + 1]![2]!.dose).toBeGreaterThan(cells[r]![2]!.dose);
    for (let c = 0; c < 4; c++) expect(cells[2]![c]!.dose).toBeGreaterThan(cells[2]![c + 1]!.dose);
  });

  it('en relleno el ancho es el intervalo y en línea el punto del láser', () => {
    const v = cellValues(form, 50, 1000);
    expect(cellDose({ mode: 'line' }, { ...v, interval: 0.2 }, 0.1)).toBeCloseTo(cellDose({ mode: 'fill' }, { ...v, interval: 0.1 }, 0.5), 9);
    expect(cellDose({ mode: 'fill' }, { ...v, interval: 0.2 }, 0.1)).toBeLessThan(cellDose({ mode: 'line' }, v, 0.1));
  });

  it('los ejes pueden ser pasadas e intervalo', () => {
    const f = { ...form, mode: 'fill' as const, rows: { param: 'passes' as const, count: 3, from: 1, to: 3 }, cols: { param: 'interval' as const, count: 2, from: 0.1, to: 0.2 } };
    const cells = gridCells(f, 0.1);
    expect(cells[0]![0]!.passes).toBe(1);
    expect(cells[2]![0]!.passes).toBe(3);
    expect(cells[2]![0]!.dose).toBeCloseTo(cells[0]![0]!.dose * 3, 9);
    expect(cells[0]![1]!.dose).toBeCloseTo(cells[0]![0]!.dose / 2, 9);
  });
});

describe('observaciones', () => {
  const cells = gridCells(form, 0.1);
  const curve = presetCurve('plywood');

  it('la curva predice un degradado de la esquina clara a la oscura', () => {
    const p = predictedDarkness(cells, curve);
    expect(p[0]![4]).toBeLessThan(p[4]![0]!);
    expect(p[4]![0]).toBeGreaterThan(p[0]![0]!);
  });

  it('si las celdas se ven como la curva predice, el ajuste no cambia nada', () => {
    const marks: Record<string, number> = {};
    for (const c of [cells[1]![1]!, cells[2]![3]!, cells[4]![2]!]) marks[cellKey(c.row, c.col)] = toneDarkness(curve, c.dose);
    const fit = fitFromMarks(curve, cells, marks);
    expect(fit.dose_char).toBeCloseTo(curve.dose_char, 2);
    expect(fit.gamma).toBeCloseTo(curve.gamma, 2);
  });

  it('si el material real se ve más claro, sube la dosis característica', () => {
    const marks = { [cellKey(2, 2)]: toneDarkness(curve, cells[2]![2]!.dose) / 2 };
    expect(fitFromMarks(curve, cells, marks).dose_char).toBeGreaterThan(curve.dose_char);
  });

  it('samplesOf ignora lo inválido y marksFromSamples vuelve a ubicar las muestras', () => {
    const marks = { [cellKey(1, 1)]: 0.4, [cellKey(3, 2)]: 0.8, [cellKey(0, 0)]: NaN };
    const samples = samplesOf(cells, marks);
    expect(samples).toHaveLength(2);
    expect(marksFromSamples(cells, samples)).toEqual({ [cellKey(1, 1)]: 0.4, [cellKey(3, 2)]: 0.8 });
    // Una muestra de otra cuadrícula (dosis que no coincide) no se ubica.
    expect(marksFromSamples(cells, [{ dose: 12345, darkness: 0.5 }])).toEqual({});
  });
});
