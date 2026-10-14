import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import type { DeviceProfile } from '@/api/types';
import { JOG_DIRS } from '@/features/machine/jogMath';
import { FALLBACK_DEVICE } from '@/lib/device';
import { bindMachineEvents, createAppStore } from './index';

const memory = new Map<string, string>();

const K40: DeviceProfile = {
  ...FALLBACK_DEVICE,
  id: 'k40-1',
  name: 'Mi K40',
  kind: 'lhy_micro',
  bed: [300, 200],
  origin: 'top_left',
  lhymicro: { board: 'M2' },
};

async function setup(device: DeviceProfile = K40, opts: { minJobMs?: number } = {}) {
  memory.clear();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => memory.get(k) ?? null,
    setItem: (k: string, v: string) => void memory.set(k, v),
  });
  const api: MockApi = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: opts.minJobMs ?? 200, jobSpeedup: 1e6 });
  setApi(api);
  const store = createAppStore();
  await store.getState().loadDevices();
  await store.getState().saveDevice(device);
  store.getState().setActiveDevice(device.id);
  await bindMachineEvents(api, store);
  await store.getState().refreshPorts();
  store.getState().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });
  return { api, store };
}

async function connected(opts?: { minJobMs?: number }) {
  const ctx = await setup(K40, opts);
  ctx.store.getState().setSelectedPort('virtual:k40');
  await ctx.store.getState().connect();
  await vi.waitFor(() => expect(ctx.store.getState().connection.connected).toBe(true));
  return ctx;
}

beforeEach(() => vi.unstubAllGlobals());

describe('K40 en la tienda de la máquina', () => {
  it('la lista de puertos trae la placa USB y la virtual, y el perfil K40 solo ve esas', async () => {
    const { store } = await setup();
    expect(store.getState().ports).toEqual(expect.arrayContaining(['usb:k40:0', 'virtual:k40']));
    // La placa USB sale antes de las máquinas virtuales, como los puertos reales.
    const ports = store.getState().ports;
    expect(ports.indexOf('usb:k40:0')).toBeLessThan(ports.indexOf('virtual:falcon2'));
  });

  it('conecta con el perfil K40 y no pide ajustes `$` de GRBL', async () => {
    const { api, store } = await setup();
    const connect = vi.spyOn(api, 'connect');
    store.getState().setSelectedPort('virtual:k40');
    await store.getState().connect();
    expect(connect.mock.calls[0]![0].device?.kind).toBe('lhy_micro');
    expect(connect.mock.calls[0]![0].port).toBe('virtual:k40');
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    expect(store.getState().firmware).toContain('K40');
    expect(store.getState().settings).toBeNull();
  });

  it('con un perfil GRBL, una placa de K40 que quedó elegida se descarta', async () => {
    const { api, store } = await setup(FALLBACK_DEVICE);
    store.getState().setSelectedPort('virtual:k40');
    const connect = vi.spyOn(api, 'connect');
    await store.getState().connect();
    expect(connect.mock.calls[0]![0].port).not.toContain('k40');
  });

  it('sin home no se mueve nada; el home deja la posición contada', async () => {
    const { api, store } = await connected();
    await expect(api.jog({ dx: 5, dy: 0, speed: 3000 })).rejects.toThrow(/Haz home primero/);
    expect((await api.lhyState()).position_known).toBe(false);
    // El home pasa por su confirmación, que manda la orden de la placa y no `$H`.
    await store.getState().confirmAlarmAction('home');
    expect((await api.lhyState()).position_known).toBe(true);
    await vi.waitFor(() => expect(store.getState().status?.work_pos).toEqual([0, 0]));
    await store.getState().jog(5, 3);
    await vi.waitFor(() => expect(store.getState().status?.work_pos).toEqual([5, 3]));
    expect(store.getState().log.some((l) => l.text.includes('home hecho'))).toBe(true);
    expect(store.getState().log.some((l) => l.text.includes('$H'))).toBe(false);
  });

  it('la consola, los overrides, el disparo de prueba y los ejes extra se rechazan con el motivo', async () => {
    const { api } = await connected();
    await expect(api.machineCommand({ line: '$$' })).rejects.toThrow(/solo de GRBL/);
    await expect(api.setOverride({ kind: 'feed', percent: 120 })).rejects.toThrow(/solo de GRBL/);
    await expect(api.fireTest({ kind: 'lhy_micro', power_pct: 5, s_max: 1000, duration_ms: 100 })).rejects.toThrow(/solo de GRBL/);
    await expect(api.jogAxis({ kind: 'lhy_micro', axis: 'z', delta: 1, speed: 100 })).rejects.toThrow(/solo de GRBL/);
  });

  it('enviar sin home hace home primero y el trabajo termina', async () => {
    const { api, store } = await connected();
    await vi.waitFor(() => expect(store.getState().status).not.toBeNull());
    await store.getState().startJob();
    await vi.waitFor(() => expect(store.getState().jobState).toBe('finished'));
    expect((await api.lhyState()).position_known).toBe(true);
  });

  it('el aviso previo trae la potencia ignorada de cada capa', async () => {
    const { api, store } = await connected();
    const s = store.getState();
    const p = await api.lhyPreview({ doc: s.doc, device: K40, options: s.jobOptions });
    expect(p.board).toBe('M2');
    expect(p.warnings.some((w) => w.kind === 'power_ignored' && (w.value ?? 0) > 0)).toBe(true);
    expect(p.homes_first).toBe(true);
  });

  it('detener un trabajo apaga el láser y pierde la posición', async () => {
    const { api, store } = await connected({ minJobMs: 5000 });
    await store.getState().confirmAlarmAction('home');
    await store.getState().startJob();
    await vi.waitFor(() => expect(store.getState().jobState).toBe('running'));
    await store.getState().cancelJob();
    await vi.waitFor(() => expect(store.getState().jobState).toBe('cancelled'));
    expect(store.getState().jobMessage).toContain('haz home');
    expect(store.getState().positionLost).toBe(true);
    expect((await api.lhyState()).position_known).toBe(false);
  });

  it('mantener pulsado un botón del pad da un paso, sin jog continuo que haya que cancelar', async () => {
    const { api, store } = await connected();
    await store.getState().confirmAlarmAction('home');
    const jog = vi.spyOn(api, 'jog');
    const realtime = vi.spyOn(api, 'machineRealtime');
    await store.getState().jogContinuousStart(JOG_DIRS.right);
    await store.getState().jogStop();
    expect(jog).toHaveBeenCalledTimes(1);
    expect(realtime).not.toHaveBeenCalled();
    expect(store.getState().jogging).toBe(false);
  });

  it('el estado y el home solo existen con un K40 conectado', async () => {
    const { api, store } = await setup(FALLBACK_DEVICE);
    await expect(api.lhyState()).rejects.toThrow(/No hay máquina conectada/);
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    await expect(api.lhyState()).rejects.toThrow(/solo del K40/);
    await expect(api.lhyHome()).rejects.toThrow(/solo del K40/);
  });
});
