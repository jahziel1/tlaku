import { describe, expect, it } from 'vitest';
import { buildLiveRoute, isLaserMove, layerOfMove, locate, MISSES_BEFORE_RESYNC, ON_PATH_MM, pointAt, posAtTime, RouteLocator, timeAt, type LiveRoute } from './route';
import { previewOf, rasterFill, squareCut } from './testRoutes';

function route(p = squareCut()): LiveRoute {
  const r = buildLiveRoute(p);
  if (!r) throw new Error('sin ruta');
  return r;
}

describe('ruta de la vista en vivo', () => {
  it('toma tiempos, capas y cajas del modelo de la vista previa', () => {
    const r = route();
    expect(r.count).toBe(5);
    expect(r.total).toBeCloseTo(r.model.times[4]!, 9);
    expect(r.bounds).toEqual([[0, 0], [110, 110]]);
    expect(r.layerColors).toEqual(['#ff0000']);
    expect(layerOfMove(r, 2)).toBe(0);
    expect(isLaserMove(r, 0)).toBe(false); // el viaje
    expect(isLaserMove(r, 1)).toBe(true);
  });

  it('sin movimientos no hay ruta', () => {
    expect(buildLiveRoute({ ...squareCut(), moves: [], move_times: [], total_s: 0 })).toBeNull();
  });

  it('timeAt/posAtTime/pointAt son inversos entre sí', () => {
    const r = route();
    for (const t of [0, 0.3, 1.2, r.total / 2, r.total - 0.01, r.total]) {
      const p = posAtTime(r, t);
      expect(timeAt(r, p.move, p.frac)).toBeCloseTo(t, 6);
    }
    // A mitad del primer lado de corte.
    const k = 1;
    const mid = (r.model.times[0]! + r.model.times[1]!) / 2;
    const p = posAtTime(r, mid);
    expect(p.move).toBe(k);
    expect(p.frac).toBeCloseTo(0.5, 6);
    expect(pointAt(r, p.move, p.frac)[0]).toBeCloseTo(60, 4);
    expect(pointAt(r, p.move, p.frac)[1]).toBeCloseTo(10, 4);
  });
});

describe('ubicar la posición reportada en la ruta', () => {
  it('encuentra el movimiento y la fracción; sin posición previa busca en toda la ruta', () => {
    const r = route();
    const l = locate(r, [110, 60], null)!;
    expect(l.move).toBe(2);
    expect(l.frac).toBeCloseTo(0.5, 5);
    expect(l.dist).toBeLessThan(1e-6);
    expect(l.t).toBeCloseTo(timeAt(r, 2, 0.5), 6);
  });

  it('una posición lejos de la ruta no se ubica (más de 0,5 mm)', () => {
    const r = route();
    expect(locate(r, [60, 10 + ON_PATH_MM + 0.2], null)).toBeNull();
    expect(locate(r, [60, 10 + ON_PATH_MM - 0.1], null)).not.toBeNull();
    expect(locate(r, [300, 300], null)).toBeNull();
  });

  it('hacia delante no retrocede: un punto que la ruta vuelve a cruzar elige el primero que no queda atrás', () => {
    // El trazo vuelve a pasar por (50, 0): ida y regreso por la misma línea.
    const r = route(
      previewOf([
        [1, 100, 0, 50],
        [1, 100, 50, 50],
        [1, 0, 50, 50],
        [1, 0, 0, 50],
        [1, 50, 0, 50], // cruza otra vez el punto (50, 0) del primer lado
        [1, 100, 0, 50],
      ]),
    );
    // Sin saber dónde estaba: el primero en la ruta.
    const first = locate(r, [50, 0], null)!;
    expect(first.move).toBe(0);
    // Si ya iba en el cuarto lado, el cruce que vale es el de adelante.
    const from = { move: 3, frac: 0.9, t: timeAt(r, 3, 0.9) };
    const ahead = locate(r, [50, 0], from)!;
    expect(ahead.move).toBe(4);
    expect(ahead.t).toBeGreaterThan(from.t);
    // Con la fracción del streamer como desempate, sin ubicación previa elige el cruce que le toca.
    const late = locate(r, [50, 0], null, { prior: 0.8 })!;
    expect(late.move).toBeGreaterThanOrEqual(4);
  });

  it('en el movimiento donde estaba solo vale de ahí en adelante', () => {
    const r = route();
    const from = { move: 1, frac: 0.6, t: timeAt(r, 1, 0.6) };
    // Un punto 30 % atrás de donde iba no se acepta: la máquina solo avanza.
    expect(locate(r, [10 + 100 * 0.3, 10], from)).toBeNull();
    // Un punto apenas atrás (ruido de 0,2 mm) se ubica en el mismo sitio, sin retroceder.
    const l = locate(r, [10 + 100 * 0.6 - 0.2, 10], from)!;
    expect(l.move).toBe(1);
    expect(l.frac).toBeCloseTo(0.6, 6);
    expect(l.t).toBeGreaterThanOrEqual(from.t);
  });

  it('la ventana mira hacia delante unos segundos de trabajo y no más', () => {
    const r = route();
    const from = { move: 1, frac: 0, t: timeAt(r, 1, 0) };
    // El último lado queda a ~20 s de trabajo: fuera de la ventana de 3 s.
    expect(locate(r, [10, 60], from)).toBeNull();
    expect(locate(r, [10, 60], from, { horizonS: 60 })).not.toBeNull();
  });
});

describe('seguimiento reporte a reporte (RouteLocator)', () => {
  it('sigue la cabeza, mantiene la última buena en un reporte suelto y recupera la ruta', () => {
    const r = route();
    const loc = new RouteLocator(r);
    let ms = 0;
    const at = (t: number) => pointAt(r, posAtTime(r, t).move, posAtTime(r, t).frac);
    const first = loc.update(at(5), (ms += 250));
    expect(first.status).toBe('resync'); // la primera búsqueda es en toda la ruta
    const a = loc.update(at(5.5), (ms += 250));
    expect(a.status).toBe('tracking');
    expect(a.pos!.t).toBeCloseTo(5.5, 3);
    // Un reporte raro (fuera de la ruta): se conserva la última ubicación buena.
    const bad = loc.update([500, 500], (ms += 250));
    expect(bad.status).toBe('held');
    expect(bad.pos!.t).toBeCloseTo(5.5, 3);
    const back = loc.update(at(6), ms + 250);
    expect(back.status).toBe('tracking');
    expect(back.pos!.t).toBeCloseTo(6, 3);
  });

  it('la ubicación nunca retrocede mientras la máquina avanza (aunque el reporte tiemble)', () => {
    const r = route(rasterFill(200, 40, 0.1));
    const loc = new RouteLocator(r);
    let last = -1;
    let ms = 0;
    for (let t = 0.2; t < r.total; t += 0.07) {
      const p = posAtTime(r, t);
      const [x, y] = pointAt(r, p.move, p.frac);
      // Ruido del reporte de 0,03 mm.
      const res = loc.update([x + 0.03 * Math.sin(t * 40), y + 0.03 * Math.cos(t * 31)], (ms += 70));
      expect(res.status === 'tracking' || res.status === 'resync').toBe(true);
      expect(res.pos!.t).toBeGreaterThanOrEqual(last - 1e-9);
      last = res.pos!.t;
    }
    expect(last).toBeGreaterThan(r.total * 0.95);
  });

  it('tras varios reportes fuera de la ventana busca en toda la ruta y avisa si la máquina retrocedió', () => {
    const r = route();
    const loc = new RouteLocator(r);
    const at = (t: number) => pointAt(r, posAtTime(r, t).move, posAtTime(r, t).frac);
    loc.update(at(25), 0);
    loc.update(at(26), 250);
    // La máquina volvió al principio (reinicio del trabajo): el primer lado está lejos hacia atrás.
    const early = at(2);
    let res = loc.update(early, 500);
    expect(res.status).toBe('held');
    res = loc.update(early, 750);
    expect(res.status).toBe('held');
    expect(MISSES_BEFORE_RESYNC).toBe(3);
    res = loc.update(early, 1500);
    expect(res.status).toBe('resync');
    expect(res.retreat).toBe(true);
    expect(res.pos!.t).toBeLessThan(5);
  });

  it('sin ubicar nada seguido es «perdida»', () => {
    const r = route();
    const loc = new RouteLocator(r);
    expect(loc.update([900, 900], 0).status).toBe('lost');
    expect(loc.pos).toBeNull();
  });
});
