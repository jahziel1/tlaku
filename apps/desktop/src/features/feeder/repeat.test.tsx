import { renderToString } from 'react-dom/server';
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import type { DeviceProfile, JobArgs } from '@/api/types';
import { rectObject } from '@/features/nest/testUtils';
import { setLanguage } from '@/i18n';
import { factoryLayer } from '@/lib/layers';
import { bindMachineEvents, selectActiveDevice, useStore } from '@/store';
import { RepeatPanel } from './RepeatPanel';
import {
  cleanCount, cleanFeed, DEFAULT_REPEAT, isRepeating, MAX_REPEATS, MIN_REPEATS, repeatBlockedReason, repeatUnavailable, resetRepeat, startRepeat, stopRepeat, usesGcodeFeeder,
  useRepeat,
} from './repeat';
import { RepeatSection } from './RepeatSection';

let api: MockApi;
let baseDevice: DeviceProfile;
let sent: JobArgs[];
let off: () => void;

const rep = () => useRepeat.getState();

const FEEDER = { enabled: true, feed_gcode: 'G91\nG0 Y{mm}\nG90', feed_mm: 300 };

/** `renderToString` lee el estado inicial de los stores: se copia el de ahora mientras se pinta. */
function ssr(node: React.ReactElement): string {
  const stores = [useStore, useRepeat] as unknown as { getInitialState: () => Record<string, unknown>; getState: () => Record<string, unknown> }[];
  const saved = stores.map((s) => ({ ...s.getInitialState() }));
  stores.forEach((s) => Object.assign(s.getInitialState(), s.getState()));
  try {
    return renderToString(node);
  } finally {
    stores.forEach((s, i) => Object.assign(s.getInitialState(), saved[i]));
  }
}

function setDevice(over: Record<string, unknown>) {
  const device = { ...baseDevice, bed: [400, 400] as [number, number], feeder: FEEDER, ...over };
  useStore.setState({
    devices: [device],
    activeDeviceId: device.id,
    doc: { version: 2, device_id: null, layers: [factoryLayer('#ff0000', 'Rojo')], objects: [rectObject('a', 40, 30)] },
    cutSelectionOnly: false,
    selectedIds: [],
  });
}

beforeAll(async () => {
  api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 40, jobSpeedup: 1e6 });
  setApi(api);
  const original = api.startJob.bind(api);
  api.startJob = (a) => {
    sent.push(a);
    return original(a);
  };
  await useStore.getState().loadDevices();
  off = await bindMachineEvents(api);
  await useStore.getState().refreshPorts();
  await useStore.getState().connect();
  await vi.waitFor(() => expect(useStore.getState().status !== null).toBe(true));
  baseDevice = selectActiveDevice(useStore.getState());
});

afterAll(() => {
  off();
  api.dispose();
});

beforeEach(() => {
  sent = [];
  setLanguage('es');
  setDevice({});
  useRepeat.setState({ config: DEFAULT_REPEAT, phase: 'idle', index: 0, done: 0, total: 0, message: null, stopRequested: false });
});

describe('qué máquinas pueden repetir con avance', () => {
  it('solo las de G-code, con el alimentador activado y válido', () => {
    const dev = (kind: string, feeder = FEEDER) => ({ kind: kind as never, feeder });
    expect(usesGcodeFeeder('grbl' as never)).toBe(true);
    for (const k of ['ruida', 'lhy_micro', 'galvo']) expect(repeatUnavailable(dev(k))).toBe('kind');
    expect(repeatUnavailable(dev('grbl', { ...FEEDER, enabled: false }))).toBe('off');
    expect(repeatUnavailable(dev('grbl', { ...FEEDER, feed_gcode: 'G0 Y10' }))).toBe('invalid');
    expect(repeatUnavailable(dev('grbl'))).toBeNull();
  });

  it('valida las repeticiones y el avance', () => {
    expect(cleanCount(MIN_REPEATS)).toBe(MIN_REPEATS);
    expect(cleanCount(MAX_REPEATS)).toBe(MAX_REPEATS);
    for (const bad of [1, 0, -3, MAX_REPEATS + 1, 2.5, Number.NaN]) expect(cleanCount(bad), String(bad)).toBeNull();
    expect(cleanFeed(120, 5000)).toBe(120);
    for (const bad of [0, -1, Number.NaN, Infinity, 5001]) expect(cleanFeed(bad, 5000), String(bad)).toBeNull();
  });

  it('explica por qué no se puede empezar', () => {
    const device = { kind: 'grbl' as never, feeder: FEEDER };
    expect(repeatBlockedReason(device, { enabled: true, count: 3, feedMm: 100 })).toBeNull();
    expect(repeatBlockedReason(device, { enabled: true, count: 1, feedMm: 100 })).toContain('entero entre 2 y 99');
    expect(repeatBlockedReason(device, { enabled: true, count: 3, feedMm: 0 })).toContain('mayor que 0');
    expect(repeatBlockedReason({ ...device, feeder: { ...FEEDER, enabled: false } }, DEFAULT_REPEAT)).toContain('activa el alimentador automático');
  });
});

describe('tanda de repeticiones', () => {
  it('manda el trabajo N veces y avanza el material entre una y otra, no después de la última', async () => {
    useRepeat.getState().patchConfig({ enabled: true, count: 3, feedMm: 120 });
    const fed0 = api.machine.fedMm;
    await startRepeat();
    expect(sent).toHaveLength(3);
    expect(rep()).toMatchObject({ phase: 'finished', done: 3, total: 3, index: 2 });
    // Dos avances de 120 mm: entre la 1 y la 2, y entre la 2 y la 3.
    expect(api.machine.fedMm - fed0).toBeCloseTo(240);
    expect(isRepeating()).toBe(false);
  }, 30000);

  it('no manda una segunda tanda mientras hay una en curso', async () => {
    useRepeat.getState().patchConfig({ enabled: true, count: 2, feedMm: 50 });
    const first = startRepeat();
    await vi.waitFor(() => expect(isRepeating()).toBe(true));
    await startRepeat(); // se ignora
    await first;
    expect(sent).toHaveLength(2);
  }, 30000);

  it('un trabajo rechazado por el motor detiene la tanda con su mensaje', async () => {
    useRepeat.getState().patchConfig({ enabled: true, count: 3, feedMm: 50 });
    const original = api.startJob;
    api.startJob = () => Promise.reject(new Error('preflight: alarma activa'));
    await startRepeat();
    api.startJob = original;
    expect(rep()).toMatchObject({ phase: 'failed', done: 0 });
    expect(rep().message).toContain('preflight');
  }, 30000);

  it('detener corta la repetición en curso y no deja salir ninguna más', async () => {
    useRepeat.getState().patchConfig({ enabled: true, count: 4, feedMm: 50 });
    const run = startRepeat();
    await vi.waitFor(() => expect(rep().phase).toBe('sending'));
    await stopRepeat();
    await run;
    // Salga la primera completa o cortada, la tanda se detiene ahí.
    expect(rep().phase).toBe('cancelled');
    expect(sent).toHaveLength(1);
    expect(rep().done).toBeLessThanOrEqual(1);
    expect(isRepeating()).toBe(false);
  }, 30000);

  it('sin máquina conectada avisa y no empieza', async () => {
    setDevice({});
    useRepeat.getState().patchConfig({ enabled: true, count: 2, feedMm: 50 });
    const connected = useStore.getState().connection;
    useStore.setState({ connection: { ...connected, connected: false } });
    await startRepeat();
    useStore.setState({ connection: connected });
    expect(rep().phase).toBe('idle');
    expect(sent).toHaveLength(0);
  });

  it('con el alimentador apagado no empieza', async () => {
    setDevice({ feeder: { ...FEEDER, enabled: false } });
    useRepeat.getState().patchConfig({ enabled: true, count: 2, feedMm: 50 });
    await startRepeat();
    expect(rep().phase).toBe('idle');
    expect(sent).toHaveLength(0);
  });

  it('reset deja el estado listo para otra tanda, salvo si hay una en curso', () => {
    useRepeat.setState({ phase: 'finished', done: 3, total: 3, index: 2 });
    resetRepeat();
    expect(rep()).toMatchObject({ phase: 'idle', done: 0, total: 0, index: 0 });
    useRepeat.setState({ phase: 'sending', done: 1, total: 3, index: 1 });
    resetRepeat();
    expect(rep().phase).toBe('sending');
  });
});

describe('interfaz', () => {
  it('la sección se oculta en máquinas sin G-code y explica cómo activarla si falta el alimentador', () => {
    setDevice({ kind: 'ruida' });
    expect(ssr(<RepeatSection />)).toBe('');
    setDevice({ feeder: { ...FEEDER, enabled: false } });
    expect(ssr(<RepeatSection />)).toContain('activa el alimentador automático');
  });

  it('con alimentador ofrece la casilla; activada muestra repeticiones, avance y el resumen', () => {
    const off = ssr(<RepeatSection />);
    expect(off).toContain('Repetir con el alimentador');
    expect(off).toContain('Repetir el trabajo y avanzar el material entre repeticiones');
    expect(off).not.toContain('Repeticiones');
    useRepeat.getState().patchConfig({ enabled: true, count: 3, feedMm: 120 });
    const on = ssr(<RepeatSection />);
    for (const s of ['Repeticiones', 'Avance entre repeticiones', 'El trabajo se graba 3 veces', 'G91 / G0 Y120 / G90', 'Usar el alto del trabajo (30,0 mm)']) expect(on).toContain(s);
    useRepeat.getState().patchConfig({ count: 1 });
    expect(ssr(<RepeatSection />)).toContain('entero entre 2 y 99');
  });

  it('en inglés se traduce', () => {
    setLanguage('en');
    useRepeat.getState().patchConfig({ enabled: true, count: 2, feedMm: 80 });
    const out = ssr(<RepeatSection />);
    expect(out).toContain('Repeat with the feeder');
    expect(out).toContain('The job is run 2 times');
  });

  it('el panel cuenta las repeticiones y cambia con la fase', () => {
    const show = (patch: object) => {
      useRepeat.setState({ config: { enabled: true, count: 3, feedMm: 120 }, total: 3, ...patch });
      return ssr(<RepeatPanel onClose={() => {}} />);
    };
    expect(show({ phase: 'sending', index: 1, done: 1 })).toContain('Grabando la repetición 2 de 3…');
    expect(show({ phase: 'sending', index: 1, done: 1 })).toContain('Detener');
    expect(show({ phase: 'feeding', index: 1, done: 1 })).toContain('Avanzando el material 120,0 mm antes de la repetición 2 de 3…');
    expect(show({ phase: 'finished', index: 2, done: 3 })).toContain('Terminaron las 3 repeticiones.');
    expect(show({ phase: 'cancelled', index: 1, done: 1 })).toContain('salieron 1 de 3');
    const failed = show({ phase: 'failed', index: 1, done: 1, message: 'alarma activa' });
    expect(failed).toContain('La repetición 2 no salió: alarma activa');
    expect(failed).toContain('Cerrar');
  });
});
