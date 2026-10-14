import { describe, expect, it } from 'vitest';
import { bedPoint, crossCenters, moveBetween, registrationMarks, tileCell } from './guide';
import type { MarkSegment, TileSpec } from './types';

/** Cuadrícula 2 x 2 de tramos de 55 x 35 con 10 mm de solape sobre un diseño de 100 x 60. */
const spec = (x0: number, y0: number, x1: number, y1: number): TileSpec => ({ rect: [[x0, y0], [x1, y1]], offset: [-x0 + 5, -y0 + 5] });
// Filas de arriba hacia abajo: la de arriba tiene Y mayor.
const specs: TileSpec[] = [spec(0, 25, 55, 60), spec(45, 25, 100, 60), spec(0, 0, 55, 35), spec(45, 0, 100, 35)];
const cross = (x: number, y: number): MarkSegment[] => [[[x - 3, y], [x + 3, y]], [[x, y - 3], [x, y + 3]]];

describe('guía de movimiento del material (M47)', () => {
  it('ubica cada tramo en la cuadrícula', () => {
    expect(tileCell(specs, 0)).toEqual({ col: 1, row: 1, cols: 2, rows: 2 });
    expect(tileCell(specs, 1)).toEqual({ col: 2, row: 1, cols: 2, rows: 2 });
    expect(tileCell(specs, 2)).toEqual({ col: 1, row: 2, cols: 2, rows: 2 });
    expect(tileCell(specs, 3)).toMatchObject({ col: 2, row: 2 });
  });

  it('dice qué se mueve entre dos tramos', () => {
    expect(moveBetween(specs, 0, 1)).toBe('column');
    expect(moveBetween(specs, 1, 3)).toBe('row');
    expect(moveBetween(specs, 1, 2)).toBe('both');
    expect(moveBetween(specs, 0, 0)).toBe('none');
  });

  it('las marcas de registro son las cruces ya grabadas que caen en el tramo', () => {
    const marks = [...cross(50, 55), ...cross(50, 30), ...cross(10, 55)];
    expect(crossCenters(marks)).toEqual([[50, 55], [50, 30], [10, 55]]);
    // Con el tramo 0 hecho, para el 1 sirven las cruces del solape (x = 50) que caen en él.
    const reg = registrationMarks(specs, ['done', 'pending', 'pending', 'pending'], marks, 1);
    expect(reg).toEqual([[50, 30], [50, 55]]);
    // Sin tramos hechos no hay cruces grabadas todavía; sin marcas tampoco.
    expect(registrationMarks(specs, ['pending', 'pending', 'pending', 'pending'], marks, 1)).toEqual([]);
    expect(registrationMarks(specs, ['done', 'pending', 'pending', 'pending'], [], 1)).toEqual([]);
  });

  it('lleva la cruz a su sitio en la cama con el desplazamiento del tramo', () => {
    expect(bedPoint(specs[1]!, [50, 55])).toEqual([10, 35]);
  });
});
