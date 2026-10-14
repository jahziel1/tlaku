import { describe, expect, it } from 'vitest';
import type { NetworkFrame } from '@/api/types';
import { base64ToBytes, runNetworkLoop, type NetworkDeps } from './netSource';

const FRAME: NetworkFrame = { mime: 'image/jpeg', data_base64: 'AAAA' };

function harness(script: (n: number) => NetworkFrame | Error, limit = 100) {
  const calls: string[] = [];
  const frames: { size: [number, number]; first: boolean }[] = [];
  const fails: string[] = [];
  const sleeps: number[] = [];
  let n = 0;
  let running = true;
  const deps: NetworkDeps = {
    async fetchFrame(url) {
      calls.push(url);
      const r = script(n++);
      if (r instanceof Error) throw r;
      return r;
    },
    async decode() {
      return [640, 480];
    },
    async sleep(ms) {
      sleeps.push(ms);
    },
  };
  const handlers = {
    onFrame: (size: [number, number], first: boolean) => {
      frames.push({ size, first });
      if (frames.length >= limit) running = false;
    },
    onFail: (m: string) => {
      fails.push(m);
    },
  };
  return { deps, handlers, calls, frames, fails, sleeps, stop: () => (running = false), current: () => running };
}

describe('bucle de la cámara de red', () => {
  it('pide un cuadro tras otro y marca el primero', async () => {
    const h = harness(() => FRAME, 3);
    await runNetworkLoop('http://192.168.1.2/s.jpg', h.deps, h.handlers, h.current);
    expect(h.calls).toEqual(Array(3).fill('http://192.168.1.2/s.jpg'));
    expect(h.frames.map((f) => f.first)).toEqual([true, false, false]);
    expect(h.frames[0]!.size).toEqual([640, 480]);
    expect(h.fails).toEqual([]);
  });

  it('el primer fallo se avisa de inmediato con el texto del motor', async () => {
    const h = harness(() => new Error('«8.8.8.8» no está en la red local'));
    await runNetworkLoop('http://8.8.8.8/', h.deps, h.handlers, h.current);
    expect(h.calls).toHaveLength(1);
    expect(h.fails).toEqual(['«8.8.8.8» no está en la red local']);
    expect(h.frames).toEqual([]);
  });

  it('tolera fallos sueltos después de tener imagen, pero se rinde si no vuelve', async () => {
    const h = harness((n) => (n === 0 || n === 3 ? FRAME : new Error(`fallo ${n}`)), 100);
    await runNetworkLoop('http://cam/', h.deps, h.handlers, h.current, { maxFailures: 3, retryMs: 7, intervalMs: 0 });
    // Cuadro 0 bien, 1 y 2 fallan, 3 bien (la racha se reinicia), 4, 5 y 6 fallan: se rinde.
    expect(h.frames).toHaveLength(2);
    expect(h.calls).toHaveLength(7);
    expect(h.fails).toEqual(['fallo 6']);
    expect(h.sleeps.filter((s) => s === 7)).toHaveLength(4);
  });

  it('se detiene cuando la sesión deja de ser la vigente, sin avisar de nada', async () => {
    const h = harness(() => FRAME, 1000);
    let seen = 0;
    const handlers = { ...h.handlers, onFrame: () => void seen++ };
    await runNetworkLoop('http://cam/', h.deps, handlers, () => seen < 2, { intervalMs: 0 });
    expect(seen).toBe(2);
    expect(h.fails).toEqual([]);
    // Un cuadro que llega tarde, después de apagar, se descarta.
    const late = harness(() => FRAME);
    let alive = true;
    const slow: NetworkDeps = { ...late.deps, fetchFrame: async () => { alive = false; return FRAME; } };
    await runNetworkLoop('http://cam/', slow, late.handlers, () => alive);
    expect(late.frames).toEqual([]);
  });

  it('decodifica base64 a bytes', () => {
    expect([...base64ToBytes('AQID')]).toEqual([1, 2, 3]);
  });
});
