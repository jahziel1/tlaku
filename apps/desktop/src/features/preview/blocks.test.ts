import { describe, expect, it } from 'vitest';
import { BlockCache, buildPaths, lodSettings, MAX_LOD, slotOf, type PathLike } from './blocks';
import { buildModel, MoveClass, type PreviewData } from './model';

class FakePath implements PathLike {
  ops: ['M' | 'L', number, number][] = [];
  moveTo(x: number, y: number) {
    this.ops.push(['M', x, y]);
  }
  lineTo(x: number, y: number) {
    this.ops.push(['L', x, y]);
  }
}
const factory = () => new FakePath();

/** `n` cortes consecutivos de 1 mm sobre el eje X con potencia 100 (todos del mismo tipo y nivel). */
function line(n: number): PreviewData {
  const moves: number[] = [];
  for (let i = 1; i <= n; i++) moves.push(1, i, 0, 100);
  return { layers: [], moves, move_times: Array.from({ length: n }, (_, i) => i + 1), total_s: n, bounds: [[0, 0], [n, 0]], order_note: null, warnings: [] };
}

describe('buildPaths', () => {
  it('agrupa por clase y nivel y encadena segmentos contiguos sin moveTo repetidos', () => {
    const m = buildModel(line(10));
    const b = buildPaths(m, 0, 10, factory);
    const p = b.paths[slotOf(MoveClass.Cut, 3)] as FakePath;
    expect(p.ops.filter((o) => o[0] === 'M')).toHaveLength(1);
    expect(p.ops.filter((o) => o[0] === 'L')).toHaveLength(10);
    expect(b.paths.filter(Boolean)).toHaveLength(1);
    expect(b.bbox).toEqual([0, 0, 10, 0]);
  });

  it('abre un moveTo nuevo cuando el siguiente segmento no empieza donde terminó el anterior', () => {
    // corte, viaje a otro lado y corte: el segundo corte no empieza donde terminó el primero
    const d: PreviewData = { ...line(0), moves: [1, 1, 0, 100, 0, 5, 5, 0, 1, 6, 5, 100], move_times: [1, 2, 3], total_s: 3 };
    const p = buildPaths(buildModel(d), 0, 3, factory).paths[slotOf(MoveClass.Cut, 3)] as FakePath;
    expect(p.ops.filter((o) => o[0] === 'M')).toHaveLength(2);
  });

  it('con `tail` dibuja el último movimiento solo hasta ese punto', () => {
    const m = buildModel(line(4));
    const b = buildPaths(m, 0, 2, factory, { x: 2.5, y: 0 });
    const p = b.paths[slotOf(MoveClass.Cut, 3)] as FakePath;
    expect(p.ops.at(-1)).toEqual(['L', 2.5, 0]);
    expect(p.ops.filter((o) => o[0] === 'L')).toHaveLength(3);
  });
});

describe('BlockCache', () => {
  it('construye cada bloque una sola vez y solo cuando se pide', () => {
    const m = buildModel(line(25));
    const cache = new BlockCache(m, factory, 10);
    expect(cache.blockCount).toBe(3);
    expect(cache.built).toBe(0);
    const a = cache.get(1);
    expect(cache.built).toBe(1);
    expect(cache.get(1)).toBe(a); // misma instancia: no se reconstruye
    expect(cache.built).toBe(1);
    expect(cache.has(0)).toBe(false);
    cache.get(0);
    cache.get(0);
    expect(cache.built).toBe(2);
  });

  it('el último bloque es el sobrante y su bbox no incluye puntos ajenos', () => {
    const m = buildModel(line(25));
    const cache = new BlockCache(m, factory, 10);
    const last = cache.get(2);
    expect(last.bbox).toEqual([20, 0, 25, 0]);
    expect((last.paths[slotOf(MoveClass.Cut, 3)] as FakePath).ops.filter((o) => o[0] === 'L')).toHaveLength(5);
  });

  it('warm respeta el presupuesto y termina de a poco', () => {
    const m = buildModel(line(100));
    const cache = new BlockCache(m, factory, 10);
    let clock = 0;
    const now = () => clock++; // cada llamada avanza 1 ms
    let done = cache.warm(3, 0, now);
    expect(done).toBe(false);
    expect(cache.built).toBeGreaterThan(0);
    expect(cache.built).toBeLessThan(10);
    let guard = 0;
    while (!done && guard++ < 20) done = cache.warm(3, 0, now);
    expect(done).toBe(true);
    expect(cache.built).toBe(10);
    expect(cache.warm(3, 0, now)).toBe(true);
  });

  it('un trabajo vacío no tiene bloques', () => {
    const cache = new BlockCache(buildModel({ ...line(0) }), factory);
    expect(cache.blockCount).toBe(0);
    expect(cache.warm(5)).toBe(true);
  });
});

describe('niveles de detalle', () => {
  it('lodSettings: los niveles altos repiten la tolerancia y funden la potencia', () => {
    expect(lodSettings(0)).toEqual({ minStep: 0, collapsePower: false });
    expect(lodSettings(3).collapsePower).toBe(false);
    expect(lodSettings(MAX_LOD + 1)).toEqual({ minStep: 0, collapsePower: true });
    expect(lodSettings(MAX_LOD + 1 + 3).minStep).toBe(lodSettings(3).minStep);
  });

  it('omite vértices a menos de la tolerancia pero conserva el extremo de la línea', () => {
    // 100 pasos de 0.1 mm: con tolerancia de 1 mm quedan ~10 segmentos, y el trazo llega a 10 mm.
    const moves: number[] = [];
    for (let i = 1; i <= 100; i++) moves.push(1, i * 0.1, 0, 100);
    const m = buildModel({ ...line(0), moves, move_times: Array.from({ length: 100 }, (_, i) => i + 1), total_s: 100 });
    const exact = buildPaths(m, 0, 100, factory);
    const coarse = buildPaths(m, 0, 100, factory, undefined, { minStep: 1, collapsePower: false });
    expect(exact.segs).toBe(100);
    expect(coarse.segs).toBeLessThanOrEqual(11);
    expect(coarse.segs).toBeGreaterThanOrEqual(9);
    const ops = (coarse.paths[slotOf(MoveClass.Cut, 3)] as FakePath).ops;
    const last = ops.at(-1)!;
    expect(last[1]).toBeCloseTo(10, 5);
    expect(coarse.bbox).toEqual(exact.bbox); // el recuadro sigue siendo el real
  });

  it('con la potencia fundida, tramos de distinta potencia van al mismo trazado', () => {
    const moves: number[] = [];
    for (let i = 1; i <= 40; i++) moves.push(1, i, 0, i % 2 ? 10 : 90);
    const m = buildModel({ ...line(0), moves, move_times: Array.from({ length: 40 }, (_, i) => i + 1), total_s: 40, layers: [{ color: '#fff', mode: 'fill', seconds: 40, summary: '', travel_saved_pct: null, explanations: [] }] });
    const keep = buildPaths(m, 0, 40, factory);
    const merged = buildPaths(m, 0, 40, factory, undefined, { minStep: 0, collapsePower: true });
    expect(keep.paths.filter(Boolean).length).toBeGreaterThan(1);
    expect(merged.paths.filter(Boolean)).toHaveLength(1);
    expect((merged.paths[0] ?? merged.paths.find(Boolean)) as FakePath).toBeDefined();
  });

  it('cada nivel de la caché es independiente', () => {
    const m = buildModel(line(25));
    const cache = new BlockCache(m, factory, 10);
    const a = cache.get(0, 0);
    const b = cache.get(0, 5);
    expect(a).not.toBe(b);
    expect(cache.get(0, 5)).toBe(b);
    expect(cache.has(0, 5)).toBe(true);
    expect(cache.has(1, 5)).toBe(false);
  });
});
