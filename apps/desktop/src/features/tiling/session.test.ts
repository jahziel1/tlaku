import { describe, expect, it } from 'vitest';
import { counts, isComplete, mark, newSession, nextPending, parseSession, sessionKey, type TilingConfig } from './session';
import type { Bounds, TileSpec } from './plan';

const design: Bounds = [[0, 0], [100, 60]];
const config: TilingConfig = { size: [55, 35], overlap: 10, order: 'rows', marks: true, margin: 5 };
const specs: TileSpec[] = [0, 1, 2, 3].map((i) => ({ rect: [[i, 0], [i + 1, 1]], offset: [0, 0] }));
const make = () => newSession(specs, design, config, 'dev', false);

describe('sesión de tiling', () => {
  it('empieza con todo pendiente en el primer tramo', () => {
    const s = make();
    expect(s.status).toEqual(['pending', 'pending', 'pending', 'pending']);
    expect(s.current).toBe(0);
    expect(isComplete(s)).toBe(false);
  });

  it('marca tramos y busca el siguiente pendiente, dando la vuelta', () => {
    let s = mark(make(), 0, 'done');
    expect(nextPending(s)).toBe(1);
    s = mark({ ...s, current: 3 }, 3, 'skipped');
    expect(nextPending(s)).toBe(1);
    s = mark(mark(s, 1, 'done'), 2, 'done');
    expect(nextPending(s)).toBeNull();
    expect(isComplete(s)).toBe(true);
    expect(counts(s)).toEqual({ done: 3, skipped: 1, pending: 0 });
  });

  it('repetir un tramo lo devuelve a pendiente', () => {
    const s = mark(mark(make(), 0, 'done'), 0, 'pending');
    expect(s.status[0]).toBe('pending');
  });

  it('la firma cambia con el diseño, las opciones o la máquina', () => {
    const k = sessionKey(design, config, 'dev');
    expect(sessionKey(design, config, 'otra')).not.toBe(k);
    expect(sessionKey(design, { ...config, overlap: 8 }, 'dev')).not.toBe(k);
    expect(sessionKey([[0, 0], [100, 61]], config, 'dev')).not.toBe(k);
    expect(sessionKey(design, { ...config }, 'dev')).toBe(k);
  });

  it('se guarda y se lee; lo roto se descarta', () => {
    const s = mark(make(), 0, 'done');
    expect(parseSession(JSON.stringify(s))).toEqual(s);
    expect(parseSession(null)).toBeNull();
    expect(parseSession('')).toBeNull();
    expect(parseSession('{')).toBeNull();
    expect(parseSession(JSON.stringify({ ...s, status: ['done'] }))).toBeNull();
    expect(parseSession(JSON.stringify({ ...s, current: 9 }))).toBeNull();
    expect(parseSession(JSON.stringify({ ...s, specs: [{ rect: 1 }] }))).toBeNull();
  });
});
