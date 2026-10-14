import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { MOCK_GALVO_ACK_REQUIRED } from '@/api/mock/galvo';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import type { DeviceProfile, GalvoEvent } from '@/api/types';
import { commands } from '@/commands';
import { handleAutomationStart } from '@/features/automation/automation';
import { FALLBACK_DEVICE } from '@/lib/device';
import { bindMachineEvents, createAppStore, useStore } from './index';

const memory = new Map<string, string>();

const GALVO: DeviceProfile = {
  ...structuredClone(FALLBACK_DEVICE),
  id: 'galvo-1',
  name: 'Mi galvo',
  kind: 'galvo',
  bed: [110, 110],
  origin: 'bottom_left',
};

const FULL = { eyewear: true, enclosure: true, emergency_stop: true };

async function setup(device: DeviceProfile = GALVO, opts: { minJobMs?: number } = {}) {
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

/** Lo que emite el motor por `machine://galvo` (el mock no lo genera solo). */
const emitGalvo = (api: MockApi, ev: GalvoEvent) => (api.machine as unknown as { emit: (e: string, p: unknown) => void }).emit('machine://galvo', ev);

beforeEach(() => vi.unstubAllGlobals());

describe('conexión del galvo en la tienda', () => {
  it('la lista de puertos trae la tarjeta USB y la virtual, y el perfil galvo solo ve esas', async () => {
    const { store } = await setup();
    const ports = store.getState().ports;
    expect(ports).toEqual(expect.arrayContaining(['usb:galvo:0', 'virtual:galvo']));
    expect(ports.indexOf('usb:galvo:0')).toBeLessThan(ports.indexOf('virtual:falcon2'));
  });

  it('la tarjeta simulada conecta sin diálogo de seguridad y sin pedir ajustes `$`', async () => {
    const { api, store } = await setup();
    const connect = vi.spyOn(api, 'connect');
    store.getState().setSelectedPort('virtual:galvo');
    await store.getState().connect();
    expect(store.getState().galvoAckPending).toBe(false);
    expect(connect.mock.calls[0]![0].device?.kind).toBe('galvo');
    expect(connect.mock.calls[0]![0]).not.toHaveProperty('galvoAck');
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    expect(store.getState().firmware).toContain('Galvo');
    expect(store.getState().settings).toBeNull();
  });

  it('una tarjeta USB real NO conecta hasta contestar el diálogo de seguridad', async () => {
    const { api, store } = await setup();
    const connect = vi.spyOn(api, 'connect');
    store.getState().setSelectedPort('usb:galvo:0');
    const pending = store.getState().connect();
    await vi.waitFor(() => expect(store.getState().galvoAckPending).toBe(true));
    expect(connect).not.toHaveBeenCalled();
    expect(store.getState().connection.connected).toBe(false);
    expect(store.getState().connecting).toBe(false);
    store.getState().answerGalvoAck(FULL);
    await pending;
    expect(connect).toHaveBeenCalledTimes(1);
    expect(connect.mock.calls[0]![0].galvoAck).toEqual(FULL);
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    expect(store.getState().galvoAckPending).toBe(false);
  });

  it('cancelar el diálogo no conecta ni toca el USB', async () => {
    const { api, store } = await setup();
    const connect = vi.spyOn(api, 'connect');
    store.getState().setSelectedPort('usb:galvo:0');
    const pending = store.getState().connect();
    await vi.waitFor(() => expect(store.getState().galvoAckPending).toBe(true));
    store.getState().answerGalvoAck(null);
    await pending;
    expect(connect).not.toHaveBeenCalled();
    expect(store.getState().connection.connected).toBe(false);
    expect(store.getState().galvoAckPending).toBe(false);
  });

  it('un reconocimiento incompleto se trata como cancelar', async () => {
    const { api, store } = await setup();
    const connect = vi.spyOn(api, 'connect');
    store.getState().setSelectedPort('usb:galvo:0');
    for (const partial of [{ ...FULL, eyewear: false }, { ...FULL, enclosure: false }, { ...FULL, emergency_stop: false }]) {
      const pending = store.getState().connect();
      await vi.waitFor(() => expect(store.getState().galvoAckPending).toBe(true));
      store.getState().answerGalvoAck(partial);
      await pending;
    }
    expect(connect).not.toHaveBeenCalled();
    expect(store.getState().connection.connected).toBe(false);
  });

  it('el reconocimiento no se recuerda: cada conexión lo vuelve a pedir', async () => {
    const { api, store } = await setup();
    const connect = vi.spyOn(api, 'connect');
    store.getState().setSelectedPort('usb:galvo:0');
    for (let i = 1; i <= 2; i++) {
      const pending = store.getState().connect();
      await vi.waitFor(() => expect(store.getState().galvoAckPending).toBe(true));
      store.getState().answerGalvoAck(FULL);
      await pending;
      await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
      expect(connect).toHaveBeenCalledTimes(i);
      await store.getState().disconnect();
      await vi.waitFor(() => expect(store.getState().connection.connected).toBe(false));
    }
    // Cada diálogo es nuevo (las casillas parten desmarcadas): el contador de peticiones sube.
    expect(store.getState().galvoAckId).toBe(2);
  });

  it('el motor (simulado) también rechaza el USB sin las tres confirmaciones', async () => {
    const { api } = await setup();
    await expect(api.connect({ port: 'usb:galvo:0', baud: 115200, device: GALVO })).rejects.toThrow(MOCK_GALVO_ACK_REQUIRED);
    await expect(api.connect({ port: 'usb:galvo:0', baud: 115200, device: GALVO, galvoAck: { ...FULL, emergency_stop: false } })).rejects.toThrow(/seguridad láser/);
    await expect(api.connect({ port: 'usb:galvo:0', baud: 115200, device: GALVO, galvoAck: FULL })).resolves.toMatchObject({ firmware: expect.stringContaining('Galvo') });
  });

  it('con un perfil GRBL no se pide el diálogo ni se ofrece la tarjeta galvo', async () => {
    const { store } = await setup(FALLBACK_DEVICE);
    store.getState().setSelectedPort('usb:galvo:0');
    await store.getState().connect();
    expect(store.getState().galvoAckPending).toBe(false);
  });
});

describe('qué no aplica al galvo', () => {
  it('no hay pausa: el comando dice por qué y el motor la rechaza', async () => {
    const { api, store } = await setup();
    const st = store.getState();
    expect(commands.availability('machine.pause', st)).toBe('galvo.pause.reason');
    expect(commands.availability('machine.home', st)).toBe('galvo.home.reason');
    expect(commands.availability('machine.unlock', st)).toBe('galvo.mset.reason');
    expect(commands.availability('machine.settings', st)).toBe('galvo.mset.reason');
    store.getState().setSelectedPort('virtual:galvo');
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    await expect(api.pauseJob()).rejects.toThrow(/no tiene pausa/);
    await expect(api.machineCommand({ line: '$$' })).rejects.toThrow(/solo de GRBL/);
    await expect(api.setOverride({ kind: 'feed', percent: 120 })).rejects.toThrow(/solo de GRBL/);
  });

  it('con un perfil GRBL la pausa y el home siguen igual', async () => {
    const { store } = await setup(FALLBACK_DEVICE);
    expect(commands.availability('machine.pause', store.getState())).toBe('reason.noJob');
    expect(commands.availability('machine.home', store.getState())).toBe('reason.needMachine');
  });

  it('el encuadre del galvo se repite solo en el motor: el bucle de la interfaz no se arma', async () => {
    const { store } = await setup();
    store.getState().setSelectedPort('virtual:galvo');
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    await vi.waitFor(() => expect(store.getState().status).not.toBeNull());
    store.getState().setFrameLoop(true);
    await store.getState().frameJob();
    expect(store.getState().frameLooping).toBe(false);
  });

  it('el estado, el recorrido en rojo y el puntero solo existen con un galvo conectado', async () => {
    const { api, store } = await setup(FALLBACK_DEVICE);
    await expect(api.galvoState()).rejects.toThrow(/No hay máquina conectada/);
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    await expect(api.galvoState()).rejects.toThrow(/solo de las máquinas galvo/);
    await expect(api.galvoRedLight({ on: true })).rejects.toThrow(/solo de las máquinas galvo/);
  });

  it('la automatización nunca inicia un galvo sin el diálogo de inicio, aunque esté permitida', () => {
    // `handleAutomationStart` usa la tienda global.
    const connected = { connected: true, port: 'virtual:galvo', error: null };
    const idle = { state: 'idle' as const, other_state: undefined, machine_pos: [0, 0] as [number, number], work_pos: [0, 0] as [number, number], feed: 0, spindle: 0, buffer: { planner: 0, rx: 0 }, overrides: undefined };
    const run = (device: DeviceProfile) => {
      const startJob = vi.fn(() => Promise.resolve());
      const requestStart = vi.fn();
      useStore.setState({ devices: [device], activeDeviceId: device.id, connection: connected, status: idle, jobState: 'idle', startJob, requestStart });
      useStore.getState().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });
      handleAutomationStart(true);
      return { startJob, requestStart };
    };
    const galvo = run(GALVO);
    expect(galvo.requestStart).toHaveBeenCalledTimes(1);
    expect(galvo.startJob).not.toHaveBeenCalled();
    // Con una máquina GRBL, confirmado, sí arranca directo (el comportamiento de siempre).
    const grbl = run(FALLBACK_DEVICE);
    expect(grbl.startJob).toHaveBeenCalledTimes(1);
    expect(grbl.requestStart).not.toHaveBeenCalled();
  });
});

describe('eventos del galvo', () => {
  it('el paso en curso y la espera de mover el material llegan por machine://galvo', async () => {
    const { api, store } = await setup();
    const step = { step: 2, steps: 3, repeat: 1, repeats: 1, piece: 2, pieces: 3, dx: -100, dy: 0, rotary_deg: 0, pause_ms: 0 };
    emitGalvo(api, { kind: 'wait_move', info: step });
    expect(store.getState().galvoWait).toEqual(step);
    emitGalvo(api, { kind: 'step', info: step });
    expect(store.getState().galvoStep).toEqual(step);
    expect(store.getState().galvoWait).toBeNull();
    emitGalvo(api, { kind: 'light', loops: 4, adjust_gen: 0, adjust_rejected: false });
    expect(store.getState().galvoLoops).toBe(4);
  });

  it('confirmar el movimiento llama al motor y borra la espera', async () => {
    const { api, store } = await setup();
    const confirm = vi.spyOn(api, 'galvoConfirmMove').mockResolvedValue();
    store.setState({ galvoWait: { step: 2, steps: 3, repeat: 1, repeats: 1, piece: 2, pieces: 3, dx: -100, dy: 0, rotary_deg: 0, pause_ms: 0 } });
    await store.getState().confirmGalvoMove();
    expect(confirm).toHaveBeenCalledTimes(1);
    expect(store.getState().galvoWait).toBeNull();
  });

  it('el aviso de cortar la energía es persistente: sobrevive a la desconexión y solo lo cierra el usuario', async () => {
    const { api, store } = await setup();
    store.getState().setSelectedPort('virtual:galvo');
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    emitGalvo(api, { kind: 'laser_may_be_on', message: '[galvo_stop_unconfirmed] NO hubo confirmación de que la fuente se apagara' });
    expect(store.getState().galvoLaserWarning).toContain('NO hubo confirmación');
    await store.getState().disconnect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(false));
    expect(store.getState().galvoLaserWarning).toContain('NO hubo confirmación');
    // Un trabajo posterior que termina bien no lo borra.
    store.getState().onJob({ state: 'finished', message: 'Trabajo terminado', position_lost: false });
    expect(store.getState().galvoLaserWarning).not.toBeNull();
    store.getState().dismissGalvoLaserWarning();
    expect(store.getState().galvoLaserWarning).toBeNull();
  });

  it('un error de trabajo o de conexión con el código de parada sin confirmar también activa el aviso', async () => {
    const { store } = await setup();
    store.getState().onJob({ state: 'error', message: '[galvo_stop_unconfirmed] cancelado (tandas enviadas: 1 de 5). Se intentó apagar la fuente pero NO hubo confirmación: puede seguir habilitada', position_lost: false });
    expect(store.getState().galvoLaserWarning).toContain('NO hubo confirmación');
    // El código no se muestra: es para la interfaz, no para el usuario.
    expect(store.getState().galvoLaserWarning).not.toContain('galvo_stop_unconfirmed');
    expect(store.getState().jobMessage).not.toContain('[');
    store.getState().dismissGalvoLaserWarning();
    store.getState().onConnection({ connected: false, port: 'usb:galvo:0', error: '[galvo_stop_unconfirmed] se perdió la conexión. NO hubo confirmación de que la fuente quedara apagada' });
    expect(store.getState().galvoLaserWarning).toContain('NO hubo confirmación');
    expect(store.getState().connection.error).not.toContain('[');
  });

  it('con el motor en inglés la parada sin confirmar se detecta y se muestra igual', async () => {
    const { api, store } = await setup();
    store.getState().onJob({ state: 'error', message: '[galvo_stop_unconfirmed] canceled (batches sent: 1 of 5). There was NO confirmation that the source turned off: it may still be enabled', position_lost: false });
    expect(store.getState().galvoLaserWarning).toBe('canceled (batches sent: 1 of 5). There was NO confirmation that the source turned off: it may still be enabled');
    store.getState().dismissGalvoLaserWarning();
    store.getState().onConnection({ connected: false, port: 'usb:galvo:0', error: '[galvo_stop_unconfirmed] connection lost. There was NO confirmation that the source turned off' });
    expect(store.getState().galvoLaserWarning).toContain('There was NO confirmation');
    store.getState().dismissGalvoLaserWarning();
    emitGalvo(api, { kind: 'laser_may_be_on', message: '[galvo_stop_unconfirmed] There was NO confirmation that the source turned off.' });
    expect(store.getState().galvoLaserWarning).toBe('There was NO confirmation that the source turned off.');
  });

  it('el texto sin código no activa el aviso', async () => {
    const { store } = await setup();
    store.getState().onJob({ state: 'error', message: 'Se intentó apagar la fuente pero NO hubo confirmación', position_lost: false });
    expect(store.getState().galvoLaserWarning).toBeNull();
  });

  it('un error cualquiera no activa el aviso', async () => {
    const { store } = await setup();
    store.getState().onJob({ state: 'error', message: 'la tarjeta está ocupada con otro trabajo', position_lost: false });
    expect(store.getState().galvoLaserWarning).toBeNull();
  });

  it('el puntero rojo: encender y apagar pasan por el motor', async () => {
    const { api, store } = await setup();
    store.getState().setSelectedPort('virtual:galvo');
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    await store.getState().setGalvoRedLight(true);
    expect(store.getState().galvoRedOn).toBe(true);
    expect((await api.galvoState()).red_light_on).toBe(true);
    await store.getState().setGalvoRedLight(false);
    expect(store.getState().galvoRedOn).toBe(false);
    expect((await api.galvoState()).red_light_on).toBe(false);
  });

  it('el recorrido en rojo marca el trabajo con la fuente deshabilitada', async () => {
    const { api, store } = await setup();
    store.getState().setSelectedPort('virtual:galvo');
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    const red = vi.spyOn(api, 'galvoRedRun');
    await store.getState().galvoRedRun();
    expect(red).toHaveBeenCalledTimes(1);
    await vi.waitFor(() => expect(store.getState().jobState).toBe('finished'));
  });

  it('la vista previa trae los pasos del marcado dividido', async () => {
    const { api, store } = await setup();
    const g = structuredClone(GALVO);
    g.galvo.split.enabled = true;
    g.galvo.repeat.count = 2;
    const s = store.getState();
    const p = await api.galvoPreview({ doc: s.doc, device: g, options: s.jobOptions });
    expect(p.steps).toBe(4);
    expect(p.repeats).toBe(2);
    expect(p.manual_moves).toBe(3);
    const plain = await api.galvoPreview({ doc: s.doc, device: GALVO, options: s.jobOptions });
    expect(plain.steps).toBe(1);
    expect(plain.manual_moves).toBe(0);
  });

  it('el tipo de máquina se cambia a galvo en el gestor y el perfil queda completo', async () => {
    const { store } = await setup();
    expect(store.getState().devices.find((d) => d.id === 'galvo-1')?.galvo.field_mm).toBe(110);
  });
});
