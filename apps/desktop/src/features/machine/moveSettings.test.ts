import { beforeEach, describe, expect, it, vi } from 'vitest';
import { JOG_STEPS } from './jogMath';
import { DEFAULT_FINISH, parseJogSteps, readFinish, readJogSteps, sanitizeFinish, sanitizeJogSteps, writeFinish, writeJogSteps } from './moveSettings';

/** Almacenamiento en memoria: el entorno de pruebas es node, sin `localStorage`. */
const store = new Map<string, string>();
beforeEach(() => {
  store.clear();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
  });
});

describe('posición final', () => {
  it('sanea modos y coordenadas', () => {
    expect(sanitizeFinish(null)).toEqual(DEFAULT_FINISH);
    expect(sanitizeFinish({ mode: 'point', x: 12.34567, y: -5 })).toEqual({ mode: 'point', x: 12.346, y: -5 });
    expect(sanitizeFinish({ mode: 'volar', x: 'a', y: Number.NaN })).toEqual(DEFAULT_FINISH);
    expect(sanitizeFinish({ mode: 'origin', x: 1e9, y: -1e9 })).toEqual({ mode: 'origin', x: 100000, y: -100000 });
  });

  it('se recuerda entre sesiones y un valor dañado cae al de fábrica', () => {
    writeFinish({ mode: 'origin', x: 3, y: 4 });
    expect(readFinish()).toEqual({ mode: 'origin', x: 3, y: 4 });
    store.set('tlaku.finishPosition', '{no es json');
    expect(readFinish()).toEqual(DEFAULT_FINISH);
  });
});

describe('pasos del jog', () => {
  it('sanea: positivos, sin repetir, ordenados, acotados y a lo más seis', () => {
    expect(sanitizeJogSteps([10, 1, 1, 0.1, -3, 'x', Number.NaN])).toEqual([0.01, 0.1, 1, 10]);
    expect(sanitizeJogSteps([1000])).toEqual([500]);
    expect(sanitizeJogSteps([1, 2, 3, 4, 5, 6, 7, 8])).toEqual([1, 2, 3, 4, 5, 6]);
    expect(sanitizeJogSteps([])).toEqual([...JOG_STEPS]);
    expect(sanitizeJogSteps('nada')).toEqual([...JOG_STEPS]);
  });

  it('lee el texto con distintos separadores y decimales', () => {
    expect(parseJogSteps('0,1; 1; 10')).toEqual([0.1, 1, 10]);
    expect(parseJogSteps('0.1 1 10 50')).toEqual([0.1, 1, 10, 50]);
    expect(parseJogSteps('0.1,1,10')).toEqual([0.1, 1, 10]);
    expect(parseJogSteps('0.1, 1, 10')).toEqual([0.1, 1, 10]);
    expect(parseJogSteps('')).toBeNull();
    expect(parseJogSteps('1, dos')).toBeNull();
    expect(parseJogSteps('0 5')).toBeNull();
  });

  it('se recuerdan entre sesiones', () => {
    expect(readJogSteps()).toEqual([...JOG_STEPS]);
    writeJogSteps([0.5, 5, 25]);
    expect(readJogSteps()).toEqual([0.5, 5, 25]);
  });
});
