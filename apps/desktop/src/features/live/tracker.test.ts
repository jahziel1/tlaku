import { describe, expect, it } from 'vitest';
import { CATCH_UP, LiveTracker, MAX_AGE_S, RATE_MAX, SNAP_AHEAD_S } from './tracker';

const FRAME = 1 / 60;

interface Sim {
  trk: LiveTracker;
  series: number[];
  wall: number;
}

/** Máquina que avanza `speed` s de ruta por s real; informa cada `every` s (tarda `lag` s en llegar). */
function run(opts: { total?: number; speed?: number; seconds: number; every?: number; lag?: number; start?: number; running?: (wall: number) => boolean; stopReportsAt?: number }): Sim {
  const trk = new LiveTracker(opts.total ?? 1000);
  const series: number[] = [];
  const every = opts.every ?? 0.25;
  const lag = opts.lag ?? 0.05;
  const speed = opts.speed ?? 1;
  const base = opts.start ?? 0;
  let truth = base;
  let nextReport = 0;
  let wall = 0;
  const running = opts.running ?? (() => true);
  for (; wall <= opts.seconds; wall += FRAME) {
    if (running(wall)) truth += speed * FRAME;
    if (wall >= nextReport && wall < (opts.stopReportsAt ?? Infinity)) {
      nextReport += every;
      trk.report({ t: truth - speed * lag, running: running(wall) }, wall);
    }
    series.push(trk.step(wall));
  }
  return { trk, series, wall };
}

const deltas = (xs: number[]) => xs.slice(1).map((v, i) => v - xs[i]!);

describe('interpolación del cabezal', () => {
  it('nunca retrocede y el avance entre cuadros es parejo', () => {
    const { series } = run({ seconds: 30, speed: 1 });
    const d = deltas(series);
    expect(Math.min(...d)).toBeGreaterThanOrEqual(0);
    // Pasados los primeros reportes, cada cuadro avanza cerca de 1/60 s de ruta (sin saltos ni parones).
    const steady = d.slice(120);
    expect(Math.max(...steady)).toBeLessThan((FRAME * 1.6));
    expect(Math.min(...steady)).toBeGreaterThan(FRAME * 0.4);
  });

  it('se queda cerca de lo que informa la máquina: ni muy atrás ni delante de la extrapolación', () => {
    const sim = run({ seconds: 30, speed: 1, every: 0.25, lag: 0.05 });
    const truth = 30 - 0.05;
    expect(sim.series.at(-1)!).toBeGreaterThan(truth - 0.6);
    expect(sim.series.at(-1)!).toBeLessThan(truth + 0.9);
  });

  it('corrige sin saltar cuando la máquina es más lenta que el estimado', () => {
    const slow = run({ seconds: 40, speed: 0.4 });
    expect(Math.min(...deltas(slow.series))).toBeGreaterThanOrEqual(0);
    expect(slow.series.at(-1)!).toBeLessThan(40 * 0.4 + 0.9);
    expect(slow.series.at(-1)!).toBeGreaterThan(40 * 0.4 - 0.8);
    // La velocidad medida converge a la de la máquina.
    expect(slow.trk.rate).toBeGreaterThan(0.3);
    expect(slow.trk.rate).toBeLessThan(0.55);
  });

  it('alcanza a una máquina más rápida (tope de velocidad de alcance)', () => {
    const fast = run({ seconds: 20, speed: 3 });
    expect(Math.min(...deltas(fast.series))).toBeGreaterThanOrEqual(0);
    expect(fast.series.at(-1)!).toBeGreaterThan(20 * 3 - 3);
    expect(fast.trk.rate).toBeLessThanOrEqual(RATE_MAX);
    // Ningún cuadro avanza más que el tope de alcance.
    expect(Math.max(...deltas(fast.series))).toBeLessThanOrEqual(FRAME * Math.max(fast.trk.rate, 1) * CATCH_UP + 1e-9);
  });

  it('un reporte atrasado o adelantado no hace saltar hacia atrás', () => {
    const trk = new LiveTracker(100);
    let wall = 0;
    trk.report({ t: 10, running: true }, wall);
    const seen: number[] = [];
    for (let i = 0; i < 60; i++) seen.push(trk.step((wall += FRAME)));
    // Llega un reporte que ubica a la máquina 2 s antes de donde se dibuja: se queda quieto y espera.
    trk.report({ t: 8, running: true }, wall);
    const before = seen.at(-1)!;
    for (let i = 0; i < 30; i++) seen.push(trk.step((wall += FRAME)));
    expect(Math.min(...deltas(seen))).toBeGreaterThanOrEqual(0);
    expect(trk.t).toBeGreaterThanOrEqual(before);
    expect(trk.located).toBe(10); // la ubicación tampoco retrocede sin aviso de retroceso real
  });
});

describe('pausa, cancelado y fin', () => {
  it('en Hold la animación se detiene en seco', () => {
    const sim = run({ seconds: 6, speed: 1, running: (w) => w < 3 });
    // Pasado el último movimiento, todo el tramo en pausa queda constante.
    const tail = sim.series.slice(Math.round(4 * 60));
    expect(Math.max(...tail) - Math.min(...tail)).toBe(0);
    expect(sim.trk.mode).toBe('hold');
    // Y no se pasó de lo que la máquina había hecho más la extrapolación corta.
    expect(sim.trk.t).toBeLessThan(3 + 0.9);
  });

  it('al reanudar sigue desde donde quedó', () => {
    const sim = run({ seconds: 10, speed: 1, running: (w) => w < 3 || w > 5 });
    expect(sim.trk.mode).toBe('running');
    const hold = sim.series[Math.round(4.5 * 60)]!;
    expect(hold).toBeLessThan(3.9);
    expect(sim.series.at(-1)!).toBeGreaterThan(hold + 4);
    expect(Math.min(...deltas(sim.series))).toBeGreaterThanOrEqual(0);
  });

  it('si los reportes dejan de llegar la cabeza se detiene a lo sumo MAX_AGE_S más allá', () => {
    const sim = run({ seconds: 10, speed: 1, stopReportsAt: 4 });
    const stop = sim.series.at(-1)!;
    expect(stop).toBeLessThan(4 + MAX_AGE_S * 1.2 + 0.1);
    const tail = sim.series.slice(-120);
    expect(Math.max(...tail) - Math.min(...tail)).toBeLessThan(1e-9);
  });

  it('cancelado o con error queda congelado y no hace caso a más reportes', () => {
    const trk = new LiveTracker(100);
    trk.report({ t: 20, running: true }, 0);
    for (let i = 1; i < 30; i++) trk.step(i * FRAME);
    const at = trk.t;
    trk.freeze();
    trk.report({ t: 60, running: true }, 1);
    for (let i = 30; i < 90; i++) expect(trk.step(i * FRAME)).toBe(at);
    expect(trk.mode).toBe('frozen');
  });

  it('al terminar va al final de la ruta y se queda', () => {
    const trk = new LiveTracker(100);
    trk.report({ t: 70, running: true }, 0);
    trk.finish();
    expect(trk.step(0.5)).toBe(100);
    trk.report({ t: 10, running: true }, 1);
    expect(trk.step(2)).toBe(100);
    expect(trk.mode).toBe('done');
  });
});

describe('primer reporte y retroceso real', () => {
  it('el primer reporte salta directo (vista activada a mitad del trabajo)', () => {
    const trk = new LiveTracker(500);
    expect(trk.started).toBe(false);
    trk.report({ t: 321.5, running: true }, 12);
    expect(trk.started).toBe(true);
    expect(trk.t).toBe(321.5);
    expect(trk.step(12)).toBe(321.5);
  });

  it('un salto adelante muy grande se salta directo; uno chico se alcanza animado', () => {
    const trk = new LiveTracker(500);
    trk.report({ t: 10, running: true }, 0);
    trk.report({ t: 10 + SNAP_AHEAD_S + 1, running: true }, 0.25);
    expect(trk.t).toBe(10 + SNAP_AHEAD_S + 1);
    const small = new LiveTracker(500);
    small.report({ t: 10, running: true }, 0);
    small.report({ t: 12, running: true }, 0.25);
    expect(small.t).toBe(10); // todavía no llegó: la animación lo alcanza
    for (let i = 0; i < 60; i++) small.step(0.25 + i * FRAME);
    expect(small.t).toBeGreaterThan(11.5);
  });

  it('un retroceso real de la máquina sí lleva la cabeza hacia atrás', () => {
    const trk = new LiveTracker(500);
    trk.report({ t: 100, running: true }, 0);
    trk.report({ t: 101, running: true }, 0.25);
    trk.report({ t: 5, running: true, retreat: true }, 0.5);
    expect(trk.t).toBe(5);
    expect(trk.located).toBe(5);
  });

  it('en marcha pero sin avanzar (dwell) deja de extrapolar a la misma velocidad', () => {
    const trk = new LiveTracker(500);
    let wall = 0;
    trk.report({ t: 50, running: true }, wall);
    for (const t of [51, 52, 52, 52, 52, 52]) {
      wall += 0.25;
      trk.report({ t, running: true }, wall);
    }
    expect(trk.rate).toBeLessThan(0.6);
  });
});
