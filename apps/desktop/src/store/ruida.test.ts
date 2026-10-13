import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import type { DeviceProfile } from '@/api/types';
import { FALLBACK_DEVICE } from '@/lib/device';
import { bindMachineEvents, createAppStore } from './index';

const memory = new Map<string, string>();

const RUIDA: DeviceProfile = {
  ...FALLBACK_DEVICE,
  id: 'ruida-1',
  name: 'CO2 6040',
  kind: 'ruida',
  bed: [600, 400],
  connection: { type: 'udp', host: '192.168.1.100' },
  ruida: { magic: null, run_after_upload: false },
};

async function setup(device: DeviceProfile = RUIDA) {
  memory.clear();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => memory.get(k) ?? null,
    setItem: (k: string, v: string) => void memory.set(k, v),
  });
  const api: MockApi = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 200, jobSpeedup: 1e6 });
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

beforeEach(() => vi.unstubAllGlobals());

describe('Ruida en la tienda de la máquina', () => {
  it('conecta con el perfil: el motor recibe el dispositivo y la tienda no pide ajustes de GRBL', async () => {
    const { api, store } = await setup();
    const connect = vi.spyOn(api, 'connect');
    await store.getState().connect();
    expect(connect).toHaveBeenCalledTimes(1);
    const args = connect.mock.calls[0]![0];
    expect(args.device?.kind).toBe('ruida');
    // El «puerto» es la red del perfil, no un puerto serie que haya quedado elegido.
    expect(args.port).toBe('udp://192.168.1.100');
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    expect(store.getState().firmware).toContain('Ruida');
    // Ruida no tiene `$$`: no se pidieron.
    expect(store.getState().settings).toBeNull();
  });

  it('la máquina virtual de Ruida se conecta y se marca como virtual', async () => {
    const { store } = await setup();
    store.getState().setSelectedPort('virtual:ruida');
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    expect(store.getState().connection.port).toBe('virtual:ruida');
  });

  it('guardando (por omisión) el trabajo termina como documento y no queda corriendo', async () => {
    const { api, store } = await setup();
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().status).not.toBeNull());
    await store.getState().startJob();
    await vi.waitFor(() => expect(store.getState().jobState).toBe('finished'));
    expect(store.getState().jobMessage).toContain('guardado');
    expect((await api.ruidaFiles()).map((f) => f.index)).toEqual([1]);
    expect((await api.ruidaInfo()).documents).toBe(1);
  });

  it('ejecutando al subir el trabajo corre como siempre', async () => {
    const { store } = await setup({ ...RUIDA, ruida: { magic: null, run_after_upload: true } });
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().status).not.toBeNull());
    await store.getState().startJob();
    await vi.waitFor(() => expect(store.getState().jobState).toBe('finished'));
    expect(store.getState().jobMessage).not.toContain('guardado');
  });

  it('la consola y los ajustes en vivo de GRBL se rechazan con el motivo', async () => {
    const { api, store } = await setup();
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    await expect(api.machineCommand({ line: '$$' })).rejects.toThrow(/solo de GRBL/);
    await expect(api.setOverride({ kind: 'feed', percent: 120 })).rejects.toThrow(/solo de GRBL/);
  });

  it('con un perfil GRBL, un puerto de Ruida que quedó elegido se descarta', async () => {
    const { api, store } = await setup(FALLBACK_DEVICE);
    store.getState().setSelectedPort('virtual:ruida');
    const connect = vi.spyOn(api, 'connect');
    await store.getState().connect();
    expect(connect.mock.calls[0]![0].port).not.toContain('ruida');
  });

  it('información y lista solo existen con una controladora Ruida conectada', async () => {
    const { api, store } = await setup(FALLBACK_DEVICE);
    await expect(api.ruidaInfo()).rejects.toThrow(/No hay máquina conectada/);
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    await expect(api.ruidaInfo()).rejects.toThrow(/solo de las controladoras Ruida/);
  });
});
