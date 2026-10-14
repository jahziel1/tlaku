import { describe, expect, it } from 'vitest';
import type { CornerMark } from '@/bindings/CornerMark';
import { polylineBez } from './bezpath';
import { cornerStillValid, pruneCorners } from './cornerMarks';

// Cuadrado de 10 mm con el vértice (10, 10) achaflanado 2 mm.
const chamfered = polylineBez([[0, 0], [10, 0], [10, 8], [8, 10], [0, 10]], true);
const mark: CornerMark = { vertex: [10, 10], from: [10, 8], to: [8, 10] };

describe('esquinas guardadas (D60)', () => {
  it('la marca vale mientras los nodos sigan donde la herramienta los dejó', () => {
    expect(cornerStillValid([chamfered], mark)).toBe(true);
    expect(pruneCorners([mark], [chamfered])).toEqual([mark]);
  });

  it('mover uno de los nodos del chaflán la descarta', () => {
    const moved = polylineBez([[0, 0], [10, 0], [10, 7], [8, 10], [0, 10]], true);
    expect(cornerStillValid([moved], mark)).toBe(false);
    expect(pruneCorners([mark], [moved])).toBeUndefined();
  });

  it('mover el nodo vecino (la recta ya no pasa por la punta) la descarta', () => {
    const moved = polylineBez([[0, 0], [12, 0], [10, 8], [8, 10], [0, 10]], true);
    expect(cornerStillValid([moved], mark)).toBe(false);
  });

  it('agregar un nodo en medio del chaflán la descarta', () => {
    const split = polylineBez([[0, 0], [10, 0], [10, 8], [9, 9], [8, 10], [0, 10]], true);
    expect(cornerStillValid([split], mark)).toBe(false);
  });

  it('borrar el nodo la descarta; las demás marcas se conservan', () => {
    const other: CornerMark = { vertex: [0, 0], from: [0, 2], to: [2, 0] };
    const two = polylineBez([[2, 0], [10, 0], [10, 8], [8, 10], [0, 10], [0, 2]], true);
    expect(pruneCorners([mark, other], [two])).toEqual([mark, other]);
    const lost = polylineBez([[2, 0], [10, 0], [10, 8], [8, 10], [0, 10]], true);
    expect(pruneCorners([mark, other], [lost])).toEqual([mark]);
  });

  it('el sentido contrario del trazo también vale', () => {
    const reversed = polylineBez([[0, 10], [8, 10], [10, 8], [10, 0], [0, 0]], true);
    expect(cornerStillValid([reversed], mark)).toBe(true);
  });

  it('sin marcas no hay nada que conservar', () => {
    expect(pruneCorners(undefined, [chamfered])).toBeUndefined();
    expect(pruneCorners([], [chamfered])).toBeUndefined();
  });
});
