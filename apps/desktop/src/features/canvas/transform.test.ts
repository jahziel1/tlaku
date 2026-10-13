import { describe, expect, it } from 'vitest';
import type { Affine, Point } from '@/api/types';
import { applyAffine, type Bounds } from '@/lib/geometry';
import {
  anchorPoint, boundsOf, frameValues, handlePoint, numericEdit, rotateAbout, rotateByPointer, rotationOf, scaleAbout,
  scaleByHandle, selectionRotation, canRotate,
} from './transform';
import { lineObj, rectObj } from './testUtil';

const close = (a: Point, b: Point) => {
  expect(a[0]).toBeCloseTo(b[0], 9);
  expect(a[1]).toBeCloseTo(b[1], 9);
};
const B: Bounds = [[10, 20], [110, 70]]; // 100 x 50

describe('puntos de referencia', () => {
  it('9 posiciones fila por fila desde arriba a la izquierda, con Y hacia arriba', () => {
    close(anchorPoint(B, 0), [10, 70]);
    close(anchorPoint(B, 2), [110, 70]);
    close(anchorPoint(B, 4), [60, 45]);
    close(anchorPoint(B, 6), [10, 20]);
    close(anchorPoint(B, 8), [110, 20]);
  });
});

describe('matrices con ancla', () => {
  it('la escala deja fijo el ancla', () => {
    const m = scaleAbout([10, 20], 2, 3);
    close(applyAffine(m, [10, 20]), [10, 20]);
    close(applyAffine(m, [30, 30]), [50, 50]);
  });

  it('el giro deja fijo el centro y gira 90° antihorario', () => {
    const m = rotateAbout([5, 5], Math.PI / 2);
    close(applyAffine(m, [5, 5]), [5, 5]);
    close(applyAffine(m, [10, 5]), [5, 10]);
  });

  it('esquina: proporcional por defecto, desde la esquina opuesta', () => {
    // Se arrastra la esquina superior derecha (110, 70) hasta (210, 70): x duplica, y no cambia.
    const m = scaleByHandle(B, 'ne', [210, 70], { proportional: true, fromCenter: false });
    // Domina el eje que más se movió: ambos ejes escalan por 2.
    close(applyAffine(m, [10, 20]), [10, 20]); // ancla = esquina opuesta
    close(applyAffine(m, [110, 70]), [210, 120]);
  });

  it('esquina con Mayús (libre): cada eje por separado', () => {
    const m = scaleByHandle(B, 'ne', [210, 70], { proportional: false, fromCenter: false });
    close(applyAffine(m, [110, 70]), [210, 70]);
    close(applyAffine(m, [10, 20]), [10, 20]);
  });

  it('con Alt escala desde el centro', () => {
    const m = scaleByHandle(B, 'ne', [160, 95], { proportional: false, fromCenter: true });
    close(applyAffine(m, [60, 45]), [60, 45]); // el centro no se mueve
    close(applyAffine(m, [110, 70]), [160, 95]);
    close(applyAffine(m, [10, 20]), [-40, -5]);
  });

  it('un lado solo escala un eje', () => {
    const m = scaleByHandle(B, 'e', [210, 999], { proportional: true, fromCenter: false });
    close(applyAffine(m, [110, 70]), [210, 70]);
    close(applyAffine(m, [10, 20]), [10, 20]);
    expect(m[3]).toBe(1);
  });

  it('pasar al otro lado del ancla refleja y nunca llega a escala 0', () => {
    const flip = scaleByHandle(B, 'e', [-90, 0], { proportional: true, fromCenter: false });
    expect(flip[0]).toBeCloseTo(-1, 9);
    const zero = scaleByHandle(B, 'e', [10, 0], { proportional: true, fromCenter: false });
    expect(Math.abs(zero[0])).toBeGreaterThan(0);
  });

  it('una caja sin ancho (línea vertical) no produce NaN', () => {
    const line: Bounds = [[5, 0], [5, 100]];
    const m = scaleByHandle(line, 'ne', [50, 200], { proportional: true, fromCenter: false });
    for (const v of m) expect(Number.isFinite(v)).toBe(true);
    expect(m[3]).toBeCloseTo(2, 9);
  });

  it('la manija está donde dice la caja', () => {
    close(handlePoint(B, 'nw'), [10, 70]);
    close(handlePoint(B, 's'), [60, 20]);
  });
});

describe('rotación por arrastre', () => {
  it('gira el ángulo recorrido por el puntero', () => {
    const r = rotateByPointer([0, 0], [10, 0], [0, 10]);
    expect(r.degrees).toBeCloseTo(90, 9);
    close(applyAffine(r.matrix, [10, 0]), [0, 10]);
  });

  it('Mayús lleva el resultado a pasos de 15°', () => {
    const r = rotateByPointer([0, 0], [10, 0], [10, 2.7], { stepDeg: 15 }); // ~15.1°
    expect(r.degrees).toBeCloseTo(15, 9);
    const base = rotateByPointer([0, 0], [10, 0], [10, 1], { stepDeg: 15, baseDeg: 7 }); // 7° + 5.7° -> 15°
    expect(base.degrees).toBeCloseTo(8, 9);
  });

  it('no salta al cruzar los 180°', () => {
    const r = rotateByPointer([0, 0], [-10, 1], [-10, -1]);
    expect(Math.abs(r.degrees)).toBeLessThan(20);
  });
});

describe('edición numérica', () => {
  const ctx = (over: Partial<Parameters<typeof numericEdit>[2]> = {}) => ({ bounds: B, anchor: 6, lockRatio: false, rotation: 0, ...over });

  it('X e Y mueven el punto de referencia al valor', () => {
    const mx = numericEdit('x', 50, ctx())!;
    close(applyAffine(mx, [10, 20]), [50, 20]);
    // Con el ancla al centro, X es el centro.
    const mc = numericEdit('x', 100, ctx({ anchor: 4 }))!;
    close(applyAffine(mc, [60, 45]), [100, 45]);
    const my = numericEdit('y', 0, ctx({ anchor: 0 }))!; // borde superior a Y=0
    close(applyAffine(my, [10, 70]), [10, 0]);
  });

  it('el ancho escala desde el ancla; con candado también el alto', () => {
    const free = numericEdit('w', 200, ctx())!;
    close(applyAffine(free, [110, 20]), [210, 20]);
    expect(free[3]).toBe(1);
    const locked = numericEdit('w', 200, ctx({ lockRatio: true }))!;
    expect(locked[3]).toBeCloseTo(2, 9);
    close(applyAffine(locked, [10, 20]), [10, 20]); // el ancla (abajo a la izquierda) no se mueve
  });

  it('el alto con ancla al centro crece hacia los dos lados', () => {
    const m = numericEdit('h', 100, ctx({ anchor: 4 }))!;
    close(applyAffine(m, [60, 20]), [60, -5]);
    close(applyAffine(m, [60, 70]), [60, 95]);
  });

  it('la rotación es relativa a la actual y gira alrededor del ancla', () => {
    const m = numericEdit('rot', 90, ctx({ anchor: 4, rotation: 0 }))!;
    close(applyAffine(m, [60, 45]), [60, 45]);
    const again = numericEdit('rot', 90, ctx({ anchor: 4, rotation: 90 }));
    expect(again).toBeNull(); // ya está en 90°
    const back = numericEdit('rot', 0, ctx({ anchor: 4, rotation: 90 }))!;
    expect(Math.atan2(back[1], back[0])).toBeCloseTo(-Math.PI / 2, 9);
  });

  it('sin cambio o con valores imposibles devuelve null', () => {
    expect(numericEdit('x', 10, ctx())).toBeNull();
    expect(numericEdit('w', 0, ctx())).toBeNull();
    expect(numericEdit('w', -5, ctx())).toBeNull();
    expect(numericEdit('h', 10, ctx({ bounds: [[0, 0], [10, 0]] }))).toBeNull(); // alto 0: nada que escalar
    expect(numericEdit('x', Number.NaN, ctx())).toBeNull();
  });

  it('frameValues lee la posición del ancla y el tamaño', () => {
    expect(frameValues(B, 6)).toEqual({ x: 10, y: 20, w: 100, h: 50 });
    expect(frameValues(B, 4)).toEqual({ x: 60, y: 45, w: 100, h: 50 });
  });
});

describe('cajas y rotación de objetos', () => {
  it('boundsOf bajo una matriz de giro recorre los puntos (no solo la caja)', () => {
    const o = rectObj('r', 0, 0, 10, 10);
    const m = rotateAbout([5, 5], Math.PI / 4);
    const b = boundsOf([o], m)!;
    expect(b[1][0] - b[0][0]).toBeCloseTo(10 * Math.SQRT2, 9);
    // Con una matriz que conserva ejes basta transformar la caja.
    const s = scaleAbout([0, 0], 2, 3) as Affine;
    expect(boundsOf([o], s)).toEqual([[0, 0], [20, 30]]);
    expect(boundsOf([])).toBeNull();
  });

  it('rotación de un objeto con transform de giro; 0 para varios', () => {
    const o = { ...rectObj('r', 0, 0, 10, 10), transform: rotateAbout([0, 0], Math.PI / 6) };
    expect(rotationOf(o)).toBeCloseTo(30, 9);
    expect(selectionRotation([o])).toBeCloseTo(30, 9);
    expect(selectionRotation([o, rectObj('b', 0, 0, 1, 1)])).toBe(0);
  });

  it('las imágenes no se pueden rotar', () => {
    const img = { ...rectObj('i', 0, 0, 1, 1), kind: { type: 'image' as const, source_path: 'a', width_px: 1, height_px: 1, size_mm: [1, 1] as [number, number] } };
    expect(canRotate([img])).toBe(false);
    expect(canRotate([lineObj('l', [0, 0], [1, 1])])).toBe(true);
    expect(canRotate([])).toBe(false);
  });
});
