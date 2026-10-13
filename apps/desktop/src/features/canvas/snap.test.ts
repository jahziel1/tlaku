import { describe, expect, it } from 'vitest';
import { collectSnapTargets, snapMove, snapPoint, snapToleranceMM, SNAP_PX, type SnapOptions } from './snap';
import { rectObj } from './testUtil';
import type { Bounds } from '@/lib/geometry';

const bed = { width: 400, height: 300 };
const other = rectObj('other', 100, 100, 200, 160); // x: 100, 150, 200 / y: 100, 130, 160
const set = collectSnapTargets([other], new Set(), bed);
const opts = (over: Partial<SnapOptions> = {}): SnapOptions => ({ tol: 2, grid: null, objects: true, ...over });
const box = (x0: number, y0: number, x1: number, y1: number): Bounds => [[x0, y0], [x1, y1]];

describe('tolerancia en pantalla', () => {
  it('son 8 px de pantalla: a más zoom, menos mm', () => {
    expect(snapToleranceMM(1)).toBe(SNAP_PX);
    expect(snapToleranceMM(4)).toBe(SNAP_PX / 4);
    expect(snapToleranceMM(0.5)).toBe(SNAP_PX * 2);
  });
});

describe('ajuste a objetos y cama', () => {
  it('pega el borde izquierdo al borde derecho de otro objeto si está dentro de la tolerancia', () => {
    const r = snapMove(box(201, 0, 231, 20), set, opts());
    expect(r.dx).toBeCloseTo(-1, 9);
    expect(r.snapped).toBe(true);
    expect(r.guides.some((g) => g.axis === 'x' && g.v === 200)).toBe(true);
  });

  it('no ajusta fuera de la tolerancia (predecible)', () => {
    const r = snapMove(box(204, 50, 234, 70), set, opts());
    expect(r.dx).toBe(0);
    expect(r.dy).toBe(0);
    expect(r.snapped).toBe(false);
    expect(r.guides).toEqual([]);
  });

  it('también ajusta centros (centro con centro)', () => {
    // El centro X de la caja (130+..): caja 130..170 -> centro 150, igual al centro del otro (150).
    const r = snapMove(box(131, 300, 171, 320), set, opts());
    expect(r.dx).toBeCloseTo(-1, 9);
  });

  it('cada eje se resuelve por separado', () => {
    const r = snapMove(box(98.5, 50, 128.5, 101.5), set, opts());
    expect(r.dx).toBeCloseTo(1.5, 9); // borde izquierdo a x=100
    expect(r.dy).toBeCloseTo(-1.5, 9); // borde superior a y=100
  });

  it('se pega a los bordes y al centro de la cama', () => {
    expect(snapMove(box(-1, 50, 29, 80), set, opts()).dx).toBeCloseTo(1, 9); // borde x=0
    expect(snapMove(box(371.5, 50, 401.5, 80), set, opts()).dx).toBeCloseTo(-1.5, 9); // borde x=400
    expect(snapMove(box(185, 20, 215.5, 40), set, opts()).dx).toBeCloseTo(-0.25, 9); // centro 200.25 -> 200 (centro de la cama)
  });

  it('gana el candidato más cercano', () => {
    const r = snapMove(box(97, 0, 127, 20), set, opts({ tol: 5 }));
    expect(r.dx).toBeCloseTo(3, 9); // 100 (a 3) y no el centro (150-... lejos)
  });

  it('un objeto saltado (el que se arrastra) no es candidato', () => {
    const s = collectSnapTargets([other], new Set(['other']), bed);
    expect(s.xs.filter((t) => t.kind === 'object')).toHaveLength(0);
  });

  it('con "objetos" apagado ignora objetos y cama', () => {
    const r = snapMove(box(201, 0, 231, 20), set, opts({ objects: false }));
    expect(r.snapped).toBe(false);
  });

  it('los objetos ocultos no son candidatos', () => {
    const s = collectSnapTargets([rectObj('h', 100, 100, 200, 160, { hidden: true })], new Set(), bed);
    expect(s.xs.filter((t) => t.kind === 'object')).toHaveLength(0);
  });
});

describe('ajuste a la cuadrícula', () => {
  it('pega los bordes al paso visible', () => {
    const r = snapMove(box(10.8, 20.9, 25.8, 30.9), { xs: [], ys: [] }, opts({ grid: 5, objects: false }));
    expect(r.dx).toBeCloseTo(-0.8, 9);
    expect(r.dy).toBeCloseTo(-0.9, 9);
  });

  it('el centro no se pega a la cuadrícula', () => {
    // Rejilla de 10, tolerancia 2: los bordes (3.5 y 17.5) quedan lejos, el centro (10.5) está a 0.5.
    const r = snapMove(box(3.5, 0.5, 17.5, 0.5), { xs: [], ys: [] }, opts({ grid: 10, objects: false }));
    expect(r.dx).toBe(0); // el eje X no se movió (el Y sí se pegó a la rejilla)
  });

  it('un objeto gana a la cuadrícula cuando empatan', () => {
    // Borde izquierdo en 100.4: el objeto está en 100 (0.4) y la rejilla de 5 también en 100 (0.4): empate.
    const r = snapMove(box(100.4, 0, 120.4, 10), set, opts({ grid: 5 }));
    expect(r.dx).toBeCloseTo(-0.4, 9);
    expect(r.guides.find((g) => g.axis === 'x')?.kind).toBe('object');
  });

  it('la cuadrícula sola deja una guía de tipo cuadrícula', () => {
    const r = snapMove(box(10.5, 0, 20.5, 10), { xs: [], ys: [] }, opts({ grid: 5, objects: false }));
    expect(r.guides.some((g) => g.kind === 'grid')).toBe(true);
  });
});

describe('ajuste de un punto (dibujo)', () => {
  it('pega el cursor a un borde y a la rejilla', () => {
    const r = snapPoint([100.7, 99.2], set, opts({ grid: 10 }));
    expect(r.dx).toBeCloseTo(-0.7, 9);
    expect(r.dy).toBeCloseTo(0.8, 9);
  });

  it('un eje desactivado no se ajusta (movimiento restringido con Mayús)', () => {
    const r = snapPoint([100.7, 99.2], set, opts(), { x: true, y: false });
    expect(r.dy).toBe(0);
  });
});
