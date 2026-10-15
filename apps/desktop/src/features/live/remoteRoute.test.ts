import { describe, expect, it } from 'vitest';
import { buildLivePayload, estimatePitch, extractRuns, MAX_BYTES, MAX_MOVES, simplifyRun } from './remoteRoute';
import { buildLiveRoute } from './route';
import { previewOf, rasterFill, squareCut, type Seg } from './testRoutes';

const route = (p = squareCut()) => buildLiveRoute(p)!;
const bytes = (v: unknown) => JSON.stringify(v).length;

describe('trazos para el celular', () => {
  it('un cuadrado cortado son cuatro lados en un solo trazo con el tiempo en cada vértice', () => {
    const r = route();
    const runs = extractRuns(r);
    expect(runs).toHaveLength(1);
    expect(runs[0]!.k).toBe(0);
    expect(runs[0]!.p).toHaveLength(5 * 3);
    // El tiempo en cada vértice es el de la ruta: sube y termina en el total.
    const ts = runs[0]!.p.filter((_, i) => i % 3 === 2);
    expect(ts).toEqual([...ts].sort((a, b) => a - b));
    expect(ts.at(-1)).toBeCloseTo(r.total, 9);
  });

  it('el desplazamiento y el overscan no van: cada pasada del relleno es un trazo', () => {
    const r = route(rasterFill(40, 30, 0.1));
    const runs = extractRuns(r);
    expect(runs).toHaveLength(40);
    expect(runs.every((x) => x.k === 1)).toBe(true);
    for (const x of runs) expect(x.p.length).toBe(6);
  });

  it('cortes y grabados de capas distintas se separan', () => {
    const segs: Seg[] = [
      [0, 10, 10, 0],
      [1, 50, 10, 80],
      [1, 50, 50, 80],
      [0, 0, 0, 0],
      [1, 20, 0, 40],
    ];
    const p = previewOf(segs, [
      { color: '#ff0000', mode: 'line', from: 0 },
      { color: '#0000ff', mode: 'fill', from: 3 },
    ]);
    const runs = extractRuns(buildLiveRoute(p)!);
    expect(runs.map((x) => [x.k, x.layer])).toEqual([[0, 0], [1, 1]]);
  });
});

describe('simplificación con tolerancia en mm', () => {
  it('quita los puntos de una recta y conserva los extremos y los tiempos de lo que queda', () => {
    const p: number[] = [];
    for (let i = 0; i <= 100; i++) p.push(i, 0.01 * Math.sin(i), i * 0.1);
    const out = simplifyRun(p, 0.1);
    expect(out).toHaveLength(6);
    expect(out.slice(0, 3)).toEqual([0, 0.01 * Math.sin(0), 0]);
    expect(out.slice(-3)).toEqual([100, 0.01 * Math.sin(100), 10]);
  });

  it('conserva las esquinas y una curva a la tolerancia pedida', () => {
    const p: number[] = [];
    for (let i = 0; i <= 360; i += 2) p.push(50 * Math.cos((i * Math.PI) / 180), 50 * Math.sin((i * Math.PI) / 180), i * 0.01);
    const fine = simplifyRun(p, 0.05);
    const coarse = simplifyRun(p, 1);
    expect(fine.length).toBeGreaterThan(coarse.length);
    // Ningún punto original queda a más de la tolerancia del trazo simplificado.
    const dist = (x: number, y: number, a: number[], b: number[]) => {
      const dx = b[0]! - a[0]!, dy = b[1]! - a[1]!, l2 = dx * dx + dy * dy;
      const t = l2 ? Math.min(1, Math.max(0, ((x - a[0]!) * dx + (y - a[1]!) * dy) / l2)) : 0;
      return Math.hypot(x - (a[0]! + t * dx), y - (a[1]! + t * dy));
    };
    for (let i = 0; i < p.length; i += 3) {
      let best = Infinity;
      for (let j = 0; j + 5 < coarse.length; j += 3) best = Math.min(best, dist(p[i]!, p[i + 1]!, coarse.slice(j, j + 2), coarse.slice(j + 3, j + 5)));
      expect(best).toBeLessThanOrEqual(1 + 1e-9);
    }
    const square = simplifyRun([0, 0, 0, 10, 0, 1, 10, 10, 2, 0, 10, 3, 0, 0, 4], 0.1);
    expect(square).toHaveLength(15);
  });
});

describe('ruta del celular con tope de tamaño', () => {
  it('un corte normal va completo, muy por debajo del tope y sin marcar como simplificado', () => {
    const payload = buildLivePayload(route(), [400, 300])!;
    expect(payload.decimated).toBe(false);
    expect(payload.runs).toHaveLength(1);
    expect(payload.layers).toEqual(['#ff0000']);
    expect(payload.bed).toEqual([400, 300]);
    expect(bytes(payload)).toBeLessThan(2000);
  });

  it('un grabado de imagen grande (miles de pasadas) cabe en ~300 KB con líneas agrupadas', () => {
    const r = route(rasterFill(40_000, 120, 0.05));
    expect(r.count).toBeGreaterThan(100_000);
    const payload = buildLivePayload(r, [400, 300])!;
    expect(bytes(payload)).toBeLessThanOrEqual(MAX_BYTES);
    expect(payload.decimated).toBe(true);
    expect(payload.runs.length).toBeLessThan(40_000);
    expect(payload.runs.length).toBeGreaterThan(1000);
    // Las líneas que quedan son más anchas para cubrir lo mismo.
    const pitch = estimatePitch(extractRuns(r));
    expect(pitch).toBeGreaterThan(0.04);
    expect(pitch).toBeLessThan(0.06);
    expect(payload.runs[0]!.w).toBeGreaterThan(pitch);
    // Los tiempos de los vértices que quedan siguen creciendo entre trazos.
    let last = -1;
    for (const run of payload.runs) {
      expect(run.p[2]!).toBeGreaterThanOrEqual(last);
      last = run.p.at(-1)!;
    }
    expect(last).toBeLessThanOrEqual(payload.total + 0.01);
  });

  it('los cortes no se agrupan aunque haya mucho grabado', () => {
    const segs: Seg[] = [[0, 0, 0, 0]];
    for (let i = 0; i < 20; i++) segs.push([1, 10 + i, 0, 90], [1, 10 + i, 10, 90], [0, 10 + i + 1, 0, 0]);
    // 30 000 pasadas de grabado detrás.
    const fill = rasterFill(30_000, 100, 0.01).moves;
    for (let i = 0; i < fill.length; i += 4) segs.push([fill[i] as 0 | 1, fill[i + 1]!, fill[i + 2]!, fill[i + 3]!]);
    const p = previewOf(segs, [
      { color: '#ff0000', mode: 'line', from: 0 },
      { color: '#0000ff', mode: 'fill', from: 61 },
    ]);
    const payload = buildLivePayload(buildLiveRoute(p)!, [400, 300])!;
    expect(bytes(payload)).toBeLessThanOrEqual(MAX_BYTES);
    expect(payload.runs.filter((r) => r.k === 0)).toHaveLength(20);
  });

  it('el tope se respeta con un tope más chico y se verifica serializando', () => {
    const r = route(rasterFill(5000, 80, 0.1));
    for (const cap of [50_000, 120_000]) {
      const payload = buildLivePayload(r, [400, 300], cap)!;
      expect(bytes(payload)).toBeLessThanOrEqual(cap);
    }
  });

  it('sin láser, o con una ruta desmesurada, no hay ruta para el celular', () => {
    const travelOnly = previewOf([[0, 10, 10, 0], [0, 20, 20, 0]]);
    expect(buildLivePayload(buildLiveRoute(travelOnly)!, [100, 100])).toBeNull();
    const huge = { ...route(), count: MAX_MOVES + 1 };
    expect(buildLivePayload(huge, [100, 100])).toBeNull();
  });

  it('los colores de capa inválidos se reemplazan por gris', () => {
    const p = squareCut();
    p.layers[0]!.color = 'rojo';
    expect(buildLivePayload(buildLiveRoute(p)!, [400, 300])!.layers).toEqual(['#808080']);
  });
});
