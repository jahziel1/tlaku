import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import type { DeviceProfile } from '@/api/types';
import { FALLBACK_DEVICE } from '@/lib/device';
import { createAppStore } from './index';

const memory = new Map<string, string>();
const profile = (id: string, patch: Partial<DeviceProfile> = {}): DeviceProfile => ({ ...FALLBACK_DEVICE, id, name: id, ...patch });

/** Un «arranque» de la app: store nuevo, misma lista de máquinas y mismas preferencias del equipo. */
async function boot(api: MockApi = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 50, jobSpeedup: 1e6 })) {
  setApi(api);
  const store = createAppStore();
  await store.getState().loadDevices();
  return { api, store };
}

beforeEach(() => {
  memory.clear();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => memory.get(k) ?? null,
    setItem: (k: string, v: string) => void memory.set(k, v),
  });
});

describe('máquina activa al abrir la app', () => {
  it('sin conexiones previas manda la marcada «por defecto»', async () => {
    const { api, store } = await boot();
    await store.getState().saveDevice(profile('vieja', { is_default: true }));
    await store.getState().saveDevice(profile('nueva'));
    const again = await boot(api);
    expect(again.store.getState().activeDeviceId).toBe('vieja');
  });

  it('al conectar queda recordada y la próxima vez se abre esa, aunque otra tenga la estrella', async () => {
    const { api, store } = await boot();
    await store.getState().saveDevice(profile('vieja', { is_default: true }));
    await store.getState().saveDevice(profile('nueva'));
    store.getState().setActiveDevice('nueva');
    await store.getState().refreshPorts();
    store.getState().setSelectedPort('virtual:grbl');
    await store.getState().connect();
    expect(memory.get('tlaku.lastDevice')).toBe('nueva');
    const again = await boot(api);
    expect(again.store.getState().activeDeviceId).toBe('nueva');
  });

  it('si la recordada ya no existe vuelve a la marcada por defecto', async () => {
    const { api, store } = await boot();
    await store.getState().saveDevice(profile('vieja', { is_default: true }));
    memory.set('tlaku.lastDevice', 'borrada');
    const again = await boot(api);
    expect(again.store.getState().activeDeviceId).toBe('vieja');
  });
});
