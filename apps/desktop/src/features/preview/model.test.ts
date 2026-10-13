import { describe, expect, it } from 'vitest';
import {
  advanceScrub,
  buildModel,
  classify,
  completedMoves,
  fallbackTimes,
  getModel,
  headAt,
  layerStartIndices,
  lowerBound,
  MoveClass,
  powerLevel,
  sanitizeMoveTimes,
  type PreviewData,
} from './model';

/** Trabajo de prueba: viaje de (0,0) a (3,4) [5 mm], corte hasta (3,14) [10 mm] y overscan de 2 mm. */
function data(over: Partial<PreviewData> = {}): PreviewData {
  return {
    layers: [],
    moves: [0, 3, 4, 0, 1, 3, 14, 80, 1, 5, 14, 0],
    move_times: [1, 3, 3.5],
    total_s: 3.5,
    bounds: [[0, 0], [5, 14]],
    order_note: null,
    warnings: [],
    ...over,
  };
}

describe('búsqueda binaria en move_times', () => {
  const times = [1, 3, 3, 3.5, 10];
  it('completedMoves cuenta los movimientos con tiempo <= t', () => {
    expect(completedMoves(times, 0)).toBe(0);
    expect(completedMoves(times, 0.99)).toBe(0);
    expect(completedMoves(times, 1)).toBe(1);
    expect(completedMoves(times, 3)).toBe(3); // los de duración cero cuentan juntos
    expect(completedMoves(times, 9.9)).toBe(4);
    expect(completedMoves(times, 10)).toBe(5);
    expect(completedMoves(times, 1e9)).toBe(5);
    expect(completedMoves([], 5)).toBe(0);
  });

  it('lowerBound devuelve el primer índice con tiempo >= t', () => {
    expect(lowerBound(times, 0)).toBe(0);
    expect(lowerBound(times, 3)).toBe(1);
    expect(lowerBound(times, 3.2)).toBe(3);
    expect(lowerBound(times, 11)).toBe(5);
  });

  it('coincide con una búsqueda lineal en un arreglo grande', () => {
    const big = Float64Array.from({ length: 5000 }, (_, i) => i * 0.37 + Math.sin(i) * 0.1 + (i > 0 ? 0 : 0));
    for (let i = 1; i < big.length; i++) big[i] = Math.max(big[i]!, big[i - 1]!);
    for (const t of [0, 1.2, 100.5, 1000, 1849, 2e4]) {
      let n = 0;
      while (n < big.length && big[n]! <= t) n++;
      expect(completedMoves(big, t)).toBe(n);
    }
  });
});

describe('interpolación de la posición en el tiempo t', () => {
  const m = buildModel(data());

  it('al inicio está en el origen y al final en el último punto', () => {
    expect(headAt(m, 0)).toMatchObject({ done: 0, x: 0, y: 0, finished: false });
    expect(headAt(m, 3.5)).toMatchObject({ done: 3, x: 5, y: 14, finished: true, cls: null });
    expect(headAt(m, 99).finished).toBe(true);
  });

  it('interpola linealmente dentro del movimiento en curso', () => {
    // Primer movimiento: 0 -> 1 s, de (0,0) a (3,4).
    const h = headAt(m, 0.5);
    expect(h.done).toBe(0);
    expect(h.frac).toBeCloseTo(0.5);
    expect(h.x).toBeCloseTo(1.5);
    expect(h.y).toBeCloseTo(2);
    // Segundo: 1 -> 3 s, de (3,4) a (3,14); a t = 2 va por la mitad.
    const h2 = headAt(m, 2);
    expect(h2.done).toBe(1);
    expect(h2.x).toBeCloseTo(3);
    expect(h2.y).toBeCloseTo(9);
    expect(h2.cls).toBe(MoveClass.Cut);
  });

  it('en el instante exacto en que termina un movimiento empieza el siguiente', () => {
    const h = headAt(m, 1);
    expect(h.done).toBe(1);
    expect(h.frac).toBe(0);
    expect([h.x, h.y]).toEqual([3, 4]);
  });

  it('un movimiento de duración cero no divide entre cero', () => {
    const z = buildModel(data({ moves: [0, 3, 4, 0, 1, 3, 4, 50, 1, 3, 14, 50], move_times: [1, 1, 3], total_s: 3 }));
    const h = headAt(z, 1);
    expect(Number.isFinite(h.x) && Number.isFinite(h.y)).toBe(true);
    expect(h.done).toBe(2);
  });

  it('un trabajo vacío no falla', () => {
    const e = buildModel(data({ moves: [], move_times: [], total_s: 0 }));
    expect(e.count).toBe(0);
    expect(headAt(e, 0).finished).toBe(true);
    expect(e.total).toBe(0);
  });
});

describe('tiempos: move_times o aproximación por distancia', () => {
  it('con move_times válidos los usa tal cual y no es aproximado', () => {
    const m = buildModel(data());
    expect(m.approximate).toBe(false);
    expect(Array.from(m.times)).toEqual([1, 3, 3.5]);
    expect(m.total).toBe(3.5);
  });

  it('rechaza move_times de otro largo, con NaN o que retroceden', () => {
    expect(sanitizeMoveTimes([1, 2], 3)).toBeNull();
    expect(sanitizeMoveTimes([1, NaN, 3], 3)).toBeNull();
    expect(sanitizeMoveTimes([1, 3, 2], 3)).toBeNull();
    expect(sanitizeMoveTimes(null, 3)).toBeNull();
    expect(sanitizeMoveTimes([0, 0, 1], 3)).not.toBeNull();
  });

  it('sin move_times aproxima por distancia y velocidad y lo marca', () => {
    const m = buildModel(data({ move_times: undefined, total_s: 7 }), 6000);
    expect(m.approximate).toBe(true);
    expect(m.times[2]).toBeCloseTo(7, 6); // el total del motor manda
    // El viaje de 5 mm a 100 mm/s tarda 0.05 s; el resto reparte los 6.95 s por distancia.
    expect(m.times[0]).toBeCloseTo(0.05 * (7 / 7), 4);
    expect(m.times[1]! - m.times[0]!).toBeGreaterThan(m.times[2]! - m.times[1]!); // 10 mm > 2 mm
  });

  it('fallbackTimes es monótona y con el cabezal parado no suma tiempo', () => {
    const pos = Float32Array.from([0, 0, 10, 0, 10, 0, 10, 10]);
    const t = fallbackTimes(pos, (i) => i === 0, 3, 5, 6000);
    expect(t[0]).toBeGreaterThan(0);
    expect(t[1]).toBeCloseTo(t[0]!); // movimiento de longitud cero
    expect(t[2]).toBeCloseTo(5);
  });

  it('sin total del motor usa solo las velocidades (sin reescalar)', () => {
    const pos = Float32Array.from([0, 0, 60, 0]);
    const t = fallbackTimes(pos, () => true, 1, 0, 6000); // 60 mm a 100 mm/s
    expect(t[0]).toBeCloseTo(0.6);
  });
});

describe('distancias por tipo', () => {
  it('suma milímetros y segundos por clase (rápido, corte, overscan)', () => {
    const m = buildModel(data());
    expect(m.distances.travel.mm).toBeCloseTo(5);
    expect(m.distances.cut.mm).toBeCloseTo(10);
    expect(m.distances.overscan.mm).toBeCloseTo(2);
    expect(m.distances.engrave.mm).toBe(0);
    expect(m.distances.travel.seconds).toBeCloseTo(1);
    expect(m.distances.cut.seconds).toBeCloseTo(2);
    expect(m.distances.overscan.seconds).toBeCloseTo(0.5);
  });

  it('sin modo de capa, la potencia separa corte de grabado', () => {
    expect(classify(1, 60)).toBe(MoveClass.Cut);
    expect(classify(1, 59.9)).toBe(MoveClass.Engrave);
    expect(classify(1, 0)).toBe(MoveClass.Overscan);
    expect(classify(0, 100)).toBe(MoveClass.Travel);
  });

  it('con tiempos exactos el modo de cada capa decide corte o grabado y reparte por capa', () => {
    const layer = (mode: string, seconds: number) => ({ color: '#fff', mode, seconds, summary: '', travel_saved_pct: null, explanations: [] });
    const d = data({
      // capa 1 (line, 2 s): viaje + corte al 30 %; capa 2 (fill, 2 s): viaje + trazo al 80 %
      moves: [0, 10, 0, 0, 1, 10, 10, 30, 0, 0, 10, 0, 1, 20, 10, 80],
      move_times: [0.5, 2, 2.5, 4],
      total_s: 4,
      layers: [layer('line', 2), layer('fill', 2)],
    });
    const m = buildModel(d);
    expect(Array.from(m.layerStarts!)).toEqual([0, 2, 4]);
    expect(m.cls[1]).toBe(MoveClass.Cut); // 30 % pero la capa es de línea
    expect(m.cls[3]).toBe(MoveClass.Engrave); // 80 % pero la capa es de relleno
    expect(m.layerDistances![0]!.cut.mm).toBeCloseTo(10);
    expect(m.layerDistances![1]!.engrave.mm).toBeCloseTo(20);
    expect(m.distances.cut.mm + m.distances.engrave.mm).toBeCloseTo(30);
  });

  it('los límites de capa se reparten por los segundos de cada bloque', () => {
    const times = Float64Array.from([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    expect(Array.from(layerStartIndices(times, [2, 3, 5], 10))).toEqual([0, 2, 5, 10]);
    expect(Array.from(layerStartIndices(times, [0, 0], 10))).toEqual([0, 0, 10]);
  });
});

describe('potencia y reproducción', () => {
  it('el nivel de opacidad sube con la potencia', () => {
    expect(powerLevel(1)).toBe(0);
    expect(powerLevel(25)).toBe(0);
    expect(powerLevel(26)).toBe(1);
    expect(powerLevel(60)).toBe(2);
    expect(powerLevel(100)).toBe(3);
    expect(powerLevel(0)).toBe(3); // sin potencia (viaje, overscan): trazo pleno
  });

  it('advanceScrub avanza según la velocidad y termina en 1', () => {
    expect(advanceScrub(0, 1, 1, 10).scrub).toBeCloseTo(0.1);
    expect(advanceScrub(0, 1, 5, 10).scrub).toBeCloseTo(0.5);
    expect(advanceScrub(0.95, 1, 1, 10)).toEqual({ scrub: 1, ended: true });
    expect(advanceScrub(0, 1, 1, 0)).toEqual({ scrub: 1, ended: true });
  });

  it('getModel guarda el modelo por arreglo de movimientos', () => {
    const d = data();
    const a = getModel(d);
    expect(getModel({ ...d })).toBe(a); // una copia superficial comparte `moves`
    expect(getModel(data())).not.toBe(a);
  });
});

describe('rendimiento del modelo', () => {
  it('construye un millón de movimientos en tiempo razonable', () => {
    const n = 1_000_000;
    const moves = new Array<number>(n * 4);
    const times = new Array<number>(n);
    for (let i = 0; i < n; i++) {
      moves[4 * i] = i % 50 === 0 ? 0 : 1;
      moves[4 * i + 1] = (i * 7) % 400;
      moves[4 * i + 2] = (i * 13) % 400;
      moves[4 * i + 3] = i % 100;
      times[i] = i * 0.001;
    }
    const t0 = performance.now();
    const m = buildModel({ ...data(), moves, move_times: times, total_s: n * 0.001 });
    const ms = performance.now() - t0;
    expect(m.count).toBe(n);
    expect(ms).toBeLessThan(3000);
  });
});
