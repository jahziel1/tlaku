import { afterEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { bindMachineEvents, useStore } from '@/store';
import { getLiveService, startLive, useLive } from './service';

/**
 * La vista en vivo con la máquina simulada de la app (modo mock): el trabajo corre por los mismos
 * eventos `machine://*` que una máquina real y la vista tiene que llegar al 100 % con el cabezal en
 * la última posición de la ruta.
 */

const memory = new Map<string, string>();
const s = () => useStore.getState();

afterEach(async () => {
  await s().disconnect();
  s().setPref('liveView', false);
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

async function runMockJob(): Promise<{ stop: () => void }> {
  memory.clear();
  vi.stubGlobal('localStorage', { getItem: (k: string) => memory.get(k) ?? null, setItem: (k: string, v: string) => void memory.set(k, v) });
  const api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 1500, jobSpeedup: 1e6 });
  setApi(api);
  await s().loadDevices();
  await bindMachineEvents(api, useStore);
  await s().refreshPorts();
  s().setSelectedPort('virtual:falcon2');
  await s().connect();
  await vi.waitFor(() => expect(s().status?.state).toBe('idle'));
  s().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });
  await s().computePreview();
  expect(s().previewStatus).toBe('ready');
  return { stop: startLive(useStore) };
}

describe('máquina simulada', () => {
  it('sigue un trabajo completo: llega al 100 % y el cabezal acaba en la última posición', async () => {
    const { stop } = await runMockJob();
    s().setPref('liveView', true);
    await s().startJob();
    const svc = getLiveService()!;
    const fractions: number[] = [];
    const sources = new Set<string | null>();
    await vi.waitFor(
      () => {
        const f = svc.frame();
        if (f) fractions.push(f.fraction);
        sources.add(useLive.getState().source);
        expect(s().jobState).toBe('finished');
      },
      { timeout: 6000, interval: 16 },
    );
    const f = svc.frame()!;
    const route = svc.route!;
    expect(f.fraction).toBe(1);
    expect(f.x).toBeCloseTo(route.model.pos[2 * route.count]!, 3);
    expect(f.y).toBeCloseTo(route.model.pos[2 * route.count + 1]!, 3);
    expect(useLive.getState().fraction).toBe(1);
    expect(useLive.getState().ended).toBe('finished');
    // La posición real encajó con la ruta (no hizo falta caer al progreso ni al reloj) y el avance nunca retrocedió.
    expect(sources.has('position')).toBe(true);
    expect(sources.has('clock')).toBe(false);
    expect(Math.min(...fractions.slice(1).map((v, i) => v - fractions[i]!))).toBeGreaterThanOrEqual(0);
    expect(fractions.length).toBeGreaterThan(10);
    stop();
  });

  it('activada a mitad del trabajo muestra lo ya hecho', async () => {
    const { stop } = await runMockJob();
    await s().startJob();
    const svc = getLiveService()!;
    await vi.waitFor(() => expect((s().progress?.acked ?? 0) / (s().progress?.total ?? 1)).toBeGreaterThan(0.4), { timeout: 4000, interval: 20 });
    expect(svc.route).toBeNull(); // apagada: ni siquiera se armó la ruta
    s().setPref('liveView', true);
    await vi.waitFor(() => expect(svc.frame()).not.toBeNull(), { timeout: 2000 });
    const f = svc.frame()!;
    expect(f.fraction).toBeGreaterThan(0.3);
    expect(f.fraction).toBeLessThan(1);
    expect(useLive.getState().source).toBe('position');
    stop();
  });

  it('cancelar a mitad la deja congelada en el punto donde quedó', async () => {
    const { stop } = await runMockJob();
    s().setPref('liveView', true);
    await s().startJob();
    const svc = getLiveService()!;
    await vi.waitFor(() => expect(svc.frame()?.fraction ?? 0).toBeGreaterThan(0.2), { timeout: 4000, interval: 20 });
    await s().cancelJob();
    await vi.waitFor(() => expect(useLive.getState().ended).toBe('cancelled'));
    const a = svc.frame()!;
    await new Promise((r) => setTimeout(r, 200));
    const b = svc.frame()!;
    expect(b.t).toBe(a.t);
    expect(a.fraction).toBeLessThan(1);
    stop();
  });
});
