import { describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import type { DeviceProfile } from '@/api/types';
import { docDeltaToWork, docToWork, FALLBACK_DEVICE, hasPointerOffset, normalizeDevice, speedFromDisplay, speedToDisplay, workToDoc } from '@/lib/device';
import { bindMachineEvents, createAppStore } from './index';

const memory = new Map<string, string>();

async function setup(port?: string) {
  memory.clear();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => memory.get(k) ?? null,
    setItem: (k: string, v: string) => void memory.set(k, v),
  });
  const api: MockApi = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 200, jobSpeedup: 1e6 });
  setApi(api);
  const store = createAppStore();
  await store.getState().loadDevices();
  await bindMachineEvents(api, store);
  await store.getState().refreshPorts();
  if (port) store.getState().setSelectedPort(port);
  return { api, store };
}

const profile = (id: string, patch: Partial<DeviceProfile> = {}): DeviceProfile => ({ ...FALLBACK_DEVICE, id, name: id, ...patch });

describe('perfil predeterminado y orden (M3)', () => {
  it('un perfil guardado antes de la ola 6C carga con los valores neutros', () => {
    const { is_default: _a, icon: _b, pointer_offset: _c, speed_unit: _d, swap_xy: _e, home_on_connect: _f, no_dtr_reset: _g, sync_streaming: _h, u_offset: _i, ...old } = FALLBACK_DEVICE;
    const d = normalizeDevice(old);
    expect(d).toMatchObject({ is_default: false, icon: '', pointer_offset: [0, 0], speed_unit: 'mm_per_min', swap_xy: false, home_on_connect: false, no_dtr_reset: false, sync_streaming: false, u_offset: 0 });
  });

  it('marcar uno por defecto se lo quita a los demás y es el que se activa al abrir', async () => {
    const { store } = await setup();
    const first = store.getState().devices[0]!;
    expect(first.is_default).toBe(false);
    expect(await store.getState().saveDevice(profile('otro', { is_default: true }))).toBe(true);
    expect(await store.getState().saveDevice({ ...first, is_default: true })).toBe(true);
    expect(store.getState().devices.filter((d) => d.is_default).map((d) => d.id)).toEqual([first.id]);
    // Una app nueva (sin documento ni perfil activo) arranca con el predeterminado.
    expect(await store.getState().saveDevice(profile('zeta', { is_default: true }))).toBe(true);
    const fresh = createAppStore();
    await fresh.getState().loadDevices();
    expect(fresh.getState().activeDeviceId).toBe('zeta');
  });

  it('reordenar guarda el orden y los que no se nombran quedan al final', async () => {
    const { store } = await setup();
    await store.getState().saveDevice(profile('b'));
    await store.getState().saveDevice(profile('c'));
    const ids = () => store.getState().devices.map((d) => d.id);
    const before = ids();
    expect(before.slice(-2)).toEqual(['b', 'c']);
    expect(await store.getState().reorderDevices(['c', 'b'])).toBe(true);
    expect(ids().slice(0, 2)).toEqual(['c', 'b']);
    expect(ids()).toHaveLength(before.length);
    await store.getState().reorderDevices(['b']);
    expect(ids()[0]).toBe('b');
  });
});

describe('ejes cruzados y unidades (M5)', () => {
  const swapped = profile('x', { bed: [400, 300], swap_xy: true });

  it('workToDoc y docToWork cruzan X e Y y son inversas', () => {
    expect(docToWork(swapped, [100, 30])).toEqual([30, 100]);
    expect(workToDoc(swapped, [30, 100])).toEqual([100, 30]);
    for (const origin of ['bottom_left', 'top_left', 'bottom_right', 'top_right'] as const) {
      const d = { ...swapped, origin };
      expect(workToDoc(d, docToWork(d, [120, 40]))).toEqual([120, 40]);
    }
    // El movimiento manual también se cruza; sin intercambio (o con rotativo) todo queda como antes.
    expect(docDeltaToWork(swapped, 5, 0)).toEqual([0, 5]);
    expect(docDeltaToWork({ ...swapped, swap_xy: false }, 5, 0)).toEqual([5, 0]);
    expect(docToWork({ ...swapped, rotary: { ...swapped.rotary, enabled: true } }, [100, 30])).toEqual([100, 30]);
  });

  it('las velocidades se muestran en mm/s sin cambiar lo guardado', () => {
    expect(speedToDisplay(6000, 'mm_per_sec')).toBe(100);
    expect(speedFromDisplay(100, 'mm_per_sec')).toBe(6000);
    expect(speedToDisplay(6000, 'mm_per_min')).toBe(6000);
  });

  it('el desfase del puntero se detecta y «posicionar con el puntero» resta el desfase', async () => {
    expect(hasPointerOffset(FALLBACK_DEVICE)).toBe(false);
    const { api, store } = await setup();
    const d = store.getState().devices[0]!;
    store.setState({ devices: [{ ...d, origin: 'bottom_left', bed: [400, 300], pointer_offset: [8, 3] }], activeDeviceId: d.id });
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().status).not.toBeNull());
    store.getState().setPositioningLaser(true);
    store.getState().setPositionAtPointer(true);
    await store.getState().positionLaserAt(100, 50);
    expect(api.machine.pos).toEqual([92, 47]);
    store.getState().setPositionAtPointer(false);
    await store.getState().positionLaserAt(100, 50);
    expect(api.machine.pos).toEqual([100, 50]);
  });
});

describe('homing al conectar (M6)', () => {
  it('abre la confirmación del homing con la opción del perfil y no la abre sin ella', async () => {
    const { store } = await setup();
    const d = store.getState().devices[0]!;
    store.setState({ devices: [{ ...d, home_on_connect: false }], activeDeviceId: d.id });
    await store.getState().connect();
    expect(store.getState().dialog).toBeNull();
    await store.getState().disconnect();
    store.setState({ devices: [{ ...d, home_on_connect: true }], activeDeviceId: d.id });
    await store.getState().connect();
    // Nunca se manda sola: queda el diálogo para confirmar `$H`.
    expect(store.getState().dialog).toEqual({ kind: 'alarm', action: 'home' });
  });
});
