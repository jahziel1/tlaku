import { describe, expect, it } from 'vitest';
import { buildLiveRoute, posAtTime, pointAt } from './route';
import { squareCut } from './testRoutes';
import { EMPTY_SNAPSHOT, type LiveSnapshot } from './source';
import { base64ToBytes, bytesToBase64, decodeRoute, decodeState, encodeRoute, encodeState, MAX_WIRE_MOVES } from './wire';

const route = buildLiveRoute(squareCut())!;
const snap: LiveSnapshot = { ...EMPTY_SNAPSHOT, jobSeq: 3, route: 'ready', source: 'position', fraction: 0.4, emitting: true, job: 'running', total: 12.5, bed: [400, 415], showTravel: true, theme: 'light', lang: 'en', timing: { jobStartedAt: 1000, jobPausedAt: null, jobPausedMs: 250, jobEndedAt: null } };

describe('base64', () => {
  it('ida y vuelta, también con muchos bytes', () => {
    const bytes = Uint8Array.from({ length: 200_000 }, (_, i) => i % 251);
    expect(Array.from(base64ToBytes(bytesToBase64(bytes)))).toEqual(Array.from(bytes));
  });
});

describe('ruta', () => {
  it('ida y vuelta: mismas posiciones, clases, capas, caja y tiempos (a precisión de Float32)', () => {
    const text = encodeRoute(7, route)!;
    const got = decodeRoute(text)!;
    expect(got.seq).toBe(7);
    expect(got.route.count).toBe(route.count);
    expect(got.route.total).toBe(route.total);
    expect(got.route.bounds).toEqual(route.bounds);
    expect(got.route.layerColors).toEqual(route.layerColors);
    expect(Array.from(got.route.model.cls)).toEqual(Array.from(route.model.cls.subarray(0, route.count)));
    expect(Array.from(got.route.model.pos)).toEqual(Array.from(route.model.pos));
    for (let i = 0; i < route.count; i++) expect(got.route.model.times[i]).toBeCloseTo(route.model.times[i]!, 3);
    // Dibujar y seguir con la ruta decodificada da lo mismo que con la original.
    for (const t of [0, route.total / 3, route.total * 0.99]) {
      const a = posAtTime(route, t);
      const b = posAtTime(got.route, t);
      expect(b.move).toBe(a.move);
      const pa = pointAt(route, a.move, a.frac);
      const pb = pointAt(got.route, b.move, b.frac);
      expect(pb[0]).toBeCloseTo(pa[0], 2);
      expect(pb[1]).toBeCloseTo(pa[1], 2);
    }
  });
  it('lo que no es válido se rechaza sin lanzar', () => {
    const ok = JSON.parse(encodeRoute(1, route)!);
    for (const bad of ['', 'no es json', '{}', JSON.stringify({ ...ok, v: 2 }), JSON.stringify({ ...ok, count: ok.count + 1 }), JSON.stringify({ ...ok, pos: 'AAAA' }), JSON.stringify({ ...ok, layers: ['rojo'] }), JSON.stringify({ ...ok, bounds: [[0, 0]] }), JSON.stringify({ ...ok, total: 0 }), JSON.stringify({ ...ok, cls: 5 })]) {
      expect(decodeRoute(bad)).toBeNull();
    }
  });
  it('una ruta demasiado grande no se manda', () => {
    expect(encodeRoute(1, { ...route, count: MAX_WIRE_MOVES + 1 })).toBeNull();
  });
});

describe('estado', () => {
  it('ida y vuelta de la foto y del reporte', () => {
    const reading = { jobSeq: 3, t: 4.2, mode: 'running' as const, retreatSeq: 1, spindle: 0 };
    const got = decodeState(encodeState(snap, reading))!;
    expect(got.snap).toEqual(snap);
    expect(got.reading).toEqual(reading);
    expect(decodeState(encodeState(snap, null))!.reading).toBeNull();
  });
  it('sanea lo raro y rechaza lo inválido', () => {
    const bad = (patch: object) => decodeState(JSON.stringify({ v: 1, snap: { ...snap, ...patch }, reading: null }));
    expect(bad({ fraction: 7 })!.snap.fraction).toBe(1);
    expect(bad({ theme: 'x', lang: 'fr', source: 'otra' })!.snap).toMatchObject({ theme: 'dark', lang: 'es', source: null });
    expect(bad({ bed: [0, 5] })).toBeNull();
    expect(bad({ route: 'quizá' })).toBeNull();
    expect(bad({ job: 'volando' })).toBeNull();
    expect(decodeState('{"v":2}')).toBeNull();
    expect(decodeState('no')).toBeNull();
    expect(decodeState(JSON.stringify({ v: 1, snap, reading: { jobSeq: 1, t: 1, mode: 'raro', retreatSeq: 0 } }))!.reading).toBeNull();
  });
});
