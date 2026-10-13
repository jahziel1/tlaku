import { describe, expect, it, vi } from 'vitest';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { setApi } from '@/api';
import { DEFAULT_OPTIMIZATION } from '@/features/preview/optimization';
import { bindMachineEvents, buildJobArgs, createAppStore, hasOutput, selectJobOptions, selectJobSelection, selectionBlockedReason } from './index';
import { overrideBlockedReason, positionLaserBlockedReason } from './machine';

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
  await store.getState().connect();
  await vi.waitFor(() => expect(store.getState().status).not.toBeNull());
  store.getState().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });
  return { api, store };
}

describe('solo la selección', () => {
  it('sin selección no envía ids y dice por qué no se puede usar', async () => {
    const { store } = await setup();
    store.getState().setCutSelectionOnly(true);
    expect(selectJobSelection(store.getState())).toBeNull();
    expect(selectionBlockedReason(store.getState())).toBe('machine.selection.reason.none');
    expect(buildJobArgs(store.getState()).options.selection).toBeNull();
  });

  it('con selección viaja en las opciones a compute, start, frame y exportación', async () => {
    const { store } = await setup();
    const id = store.getState().doc.objects[0]!.id;
    store.getState().select([id]);
    store.getState().setCutSelectionOnly(true);
    store.getState().setUseSelectionOrigin(true);
    const o = selectJobOptions(store.getState());
    expect(o.selection).toEqual([id]);
    expect(o.use_selection_origin).toBe(true);
    expect(buildJobArgs(store.getState()).options).toEqual(o);
    // Apagada, la selección no viaja ni el origen de la selección.
    store.getState().setCutSelectionOnly(false);
    expect(selectJobOptions(store.getState())).toMatchObject({ selection: null, use_selection_origin: false });
  });

  it('"hay algo que enviar" mira solo lo seleccionado', async () => {
    const { store } = await setup();
    const s = store.getState();
    expect(hasOutput(s)).toBe(true);
    s.setCutSelectionOnly(true);
    s.select([s.doc.objects[0]!.id]);
    expect(hasOutput(store.getState())).toBe(true);
    // Una selección sin objetos con salida no deja enviar nada.
    store.setState({ selectedIds: ['no-existe'] });
    expect(hasOutput(store.getState())).toBe(false);
  });
});

describe('ajustes de optimización', () => {
  it('se validan, se recuerdan y viajan en las opciones', async () => {
    const { store } = await setup();
    expect(store.getState().jobOptions.optimization).toEqual(DEFAULT_OPTIMIZATION);
    store.getState().setOptimization({ hide_backlash: true, overlap_tolerance: 50, by_group: 'sí' as never });
    const o = store.getState().jobOptions.optimization;
    expect(o).toMatchObject({ hide_backlash: true, overlap_tolerance: 1, by_group: false });
    expect(buildJobArgs(store.getState()).options.optimization).toEqual(o);
    // Una sesión nueva lee lo guardado.
    const next = createAppStore();
    expect(next.getState().jobOptions.optimization).toEqual(o);
    store.getState().resetOptimization();
    expect(store.getState().jobOptions.optimization).toEqual(DEFAULT_OPTIMIZATION);
  });

  it('cambiar la optimización invalida la vista previa', async () => {
    const { store } = await setup();
    store.setState({ previewStale: false });
    const before = store.getState().jobOptions;
    store.getState().setOptimization({ best_start: false });
    expect(store.getState().jobOptions).not.toBe(before);
  });
});

describe('posicionar el láser', () => {
  it('lleva la cabeza al punto del documento según el origen de la máquina', async () => {
    const { api, store } = await setup();
    store.getState().setPositioningLaser(true);
    expect(store.getState().positioningLaser).toBe(true);
    const d = store.getState().devices[0]!;
    store.setState({ devices: [{ ...d, origin: 'top_left', bed: [400, 300] }], activeDeviceId: d.id });
    await store.getState().positionLaserAt(100, 250);
    // Arriba a la izquierda: la Y de la máquina crece hacia abajo (300 - 250).
    expect(api.machine.pos).toEqual([100, 50]);
    // Sigue activo para el siguiente clic.
    expect(store.getState().positioningLaser).toBe(true);
  });

  it('no hace nada si el modo está apagado y avisa si el clic cae fuera de la cama', async () => {
    const { api, store } = await setup();
    const before = [...api.machine.pos];
    await store.getState().positionLaserAt(10, 10);
    expect(api.machine.pos).toEqual(before);
    store.getState().setPositioningLaser(true);
    await store.getState().positionLaserAt(9999, 10);
    expect(api.machine.pos).toEqual(before);
    expect(store.getState().notices.some((n) => n.level === 'warning')).toBe(true);
  });

  it('no se activa sin máquina, con rotativo ni durante un trabajo; y se apaga al iniciar uno', async () => {
    const { store } = await setup();
    store.getState().setPositioningLaser(true);
    await store.getState().frameHull();
    expect(store.getState().positioningLaser).toBe(false);
    const s = store.getState();
    expect(positionLaserBlockedReason({ ...s, connection: { connected: false, port: null, error: null } })).toBe('reason.needMachine');
    const d = s.devices[0]!;
    expect(positionLaserBlockedReason({ ...s, devices: [{ ...d, rotary: { ...d.rotary, enabled: true } }], activeDeviceId: d.id })).toBe('machine.position.reason.rotary');
  });
});

describe('ajustes en vivo (overrides)', () => {
  it('velocidad y potencia llegan a la máquina y vuelven en el estado', async () => {
    const { store } = await setup();
    await store.getState().setOverride('feed', 130);
    await store.getState().setOverride('spindle', 60);
    await vi.waitFor(() => expect(store.getState().status?.overrides).toMatchObject({ feed: 130, spindle: 60 }));
  });

  it('en Marlin y Smoothieware están deshabilitados con el motivo y no se envía nada', async () => {
    const { store } = await setup('virtual:marlin');
    const s = store.getState();
    expect(overrideBlockedReason(s)).toBe('machine.override.reason.family');
    await s.setOverride('feed', 150);
    expect(store.getState().status?.overrides).toBeUndefined();
    expect(store.getState().notices.some((n) => n.level === 'warning')).toBe(true);
  });

  it('sin conexión piden conectar', async () => {
    const { store } = await setup();
    await store.getState().disconnect();
    expect(overrideBlockedReason(store.getState())).toBe('reason.needMachine');
  });
});

describe('posición final y pasos del jog', () => {
  it('al terminar bien el trabajo la cabeza va a la posición elegida; cancelado, no', async () => {
    const { api, store } = await setup();
    store.getState().setFinishPosition({ mode: 'point', x: 33, y: 44 });
    api.machine.pos = [1, 1];
    store.getState().onJob({ state: 'cancelled', message: '', position_lost: false });
    await new Promise((r) => setTimeout(r, 60));
    expect(api.machine.pos).toEqual([1, 1]);
    store.getState().onJob({ state: 'finished', message: 'ok', position_lost: false });
    await vi.waitFor(() => expect(api.machine.pos).toEqual([33, 44]));
  });

  it('"quedarse" no mueve y "volver al origen" usa el de usuario o el cero', async () => {
    const { api, store } = await setup();
    api.machine.pos = [5, 5];
    await store.getState().goToFinishPosition();
    expect(api.machine.pos).toEqual([5, 5]);
    store.getState().setFinishPosition({ mode: 'origin' });
    await store.getState().goToFinishPosition();
    expect(api.machine.pos).toEqual([0, 0]);
    store.getState().setJobOptions({ user_origin: [20, 10] });
    await store.getState().goToFinishPosition();
    expect(api.machine.pos).toEqual([20, 10]);
  });

  it('los pasos configurables se recuerdan y el paso actual cae en uno válido', async () => {
    const { store } = await setup();
    store.getState().setJogStep(10);
    store.getState().setJogSteps([0.5, 5, 25]);
    expect(store.getState().jogSteps).toEqual([0.5, 5, 25]);
    expect(store.getState().jogStep).toBe(5);
    expect(createAppStore().getState().jogSteps).toEqual([0.5, 5, 25]);
  });
});

describe('encuadre del contorno', () => {
  it('recorre el casco del trabajo con la máquina simulada', async () => {
    const { api, store } = await setup();
    const spy = vi.spyOn(api.machine, 'frameHull');
    await store.getState().frameHull();
    expect(spy).toHaveBeenCalledTimes(1);
    expect(store.getState().jobError).toBeNull();
  });
});
