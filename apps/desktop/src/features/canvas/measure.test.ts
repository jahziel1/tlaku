import { describe, expect, it } from 'vitest';
import { ellipseBez, polylineBez } from '@/lib/bezpath';
import { measureBetween, measureShape } from './measure';
import { newVectorObject } from './shapes';
import { lineObj, rectObj } from './testUtil';

describe('medir entre dos puntos', () => {
  it('distancia, ángulo y deltas en mm', () => {
    const m = measureBetween([10, 10], [13, 14]);
    expect(m.distance).toBe(5);
    expect(m.dx).toBe(3);
    expect(m.dy).toBe(4);
    expect(m.angle).toBeCloseTo(53.1301, 3);
  });

  it('ángulos en los cuatro cuadrantes (antihorario, Y hacia arriba)', () => {
    expect(measureBetween([0, 0], [0, 5]).angle).toBe(90);
    expect(measureBetween([0, 0], [-5, 0]).angle).toBe(180);
    expect(measureBetween([0, 0], [0, -5]).angle).toBe(-90);
    expect(measureBetween([0, 0], [0, 0])).toMatchObject({ distance: 0, angle: 0 });
  });
});

describe('medir una forma', () => {
  it('rectángulo: nodos, perímetro, área y cerrado', () => {
    const info = measureShape(rectObj('r', 0, 0, 10, 4))!;
    expect(info.nodes).toBe(4);
    expect(info.lines).toBe(4);
    expect(info.curves).toBe(0);
    expect(info.perimeter).toBeCloseTo(28, 9);
    expect(info.area).toBeCloseTo(40, 9);
    expect(info.closed).toBe(true);
    expect(info.open).toBe(false);
    expect(info.size).toEqual([10, 4]);
    expect(info.circle).toBeNull();
  });

  it('línea abierta: sin área', () => {
    const info = measureShape(lineObj('l', [0, 0], [3, 4]))!;
    expect(info.nodes).toBe(2);
    expect(info.perimeter).toBeCloseTo(5, 9);
    expect(info.area).toBe(0);
    expect(info.open).toBe(true);
    expect(info.closed).toBe(false);
  });

  it('un círculo informa diámetro y centro', () => {
    const info = measureShape(newVectorObject('c', '#ff0000', [ellipseBez(10, 20, 5, 5)]))!;
    expect(info.circle?.diameter).toBeCloseTo(10, 9);
    expect(info.circle?.center).toEqual([10, 20]);
    expect(info.curves).toBe(4);
    expect(info.area).toBeCloseTo(Math.PI * 25, 0); // el contorno se aplana con 0,01 mm de error
    expect(info.perimeter).toBeCloseTo(Math.PI * 10, 0);
  });

  it('un agujero resta del área (par-impar)', () => {
    const plate = newVectorObject('p', '#ff0000', [
      polylineBez([[0, 0], [20, 0], [20, 20], [0, 20]], true),
      polylineBez([[5, 5], [15, 5], [15, 15], [5, 15]], true),
    ]);
    expect(measureShape(plate)!.area).toBeCloseTo(400 - 100, 9);
    expect(measureShape(plate)!.nodes).toBe(8);
  });
});
