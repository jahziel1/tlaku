import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { mockPlanTiles } from '@/api/mock/tilePlan';
import type { JobArgs } from '@/api/types';
import { rectObject } from '@/features/nest/testUtils';
import { factoryLayer } from '@/lib/layers';
import { bindMachineEvents, selectActiveDevice, useStore } from '@/store';
import { beginSession, resumeSession, restartSession, selectTile, sendCurrent, skipCurrent, tileJobArgs } from './run';
import { loadSession } from './session';
import { closeTiling, openTiling, useTilingUi } from './store';

const ui = () => useTilingUi.getState();

/** Espera a que el motor (aquí la maqueta) responda con el plan de las opciones de ahora. */
const planned = () => vi.waitFor(() => expect(ui().planning).toBe(false));

/** Las cruces de alineación de un diseño de 100 x 60 en tramos de 55 x 35 con 10 mm de solape. */
function crosses() {
  const r = mockPlanTiles({ design: [[0, 0], [100, 60]], size: [55, 35], overlap: 10, order: 'rows', margin: 0 });
  if (!r.ok) throw new Error(r.error.kind);
  return r.plan.marks;
}
let api: MockApi;
let sent: JobArgs[];
let off: () => void;

/** Almacenamiento local en memoria (las pruebas corren en Node, sin `localStorage`). */
const store = new Map<string, string>();
vi.stubGlobal('localStorage', {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => void store.set(k, v),
  removeItem: (k: string) => void store.delete(k),
});

/** Pone un diseño de `w` x `h` mm, una cama de 60 x 40 y el alimentador como se pida. */
async function setup(w: number, h: number, feeder: boolean) {
  const device = { ...selectActiveDevice(useStore.getState()), bed: [60, 40] as [number, number], feeder: { enabled: feeder, feed_gcode: 'G91\nG0 Y{mm}\nG90', feed_mm: 300 } };
  useStore.setState({
    devices: [device],
    activeDeviceId: device.id,
    doc: { version: 2, device_id: null, layers: [factoryLayer('#ff0000', 'Rojo')], objects: [rectObject('a', w, h)] },
    cutSelectionOnly: false,
    selectedIds: [],
  });
  restartSession();
  openTiling();
  await planned();
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
});

afterAll(() => {
  off();
  api.dispose();
});

beforeEach(() => {
  sent = [];
});

describe('envío por tramos', () => {
  it('manda tramo por tramo pidiendo la colocación entre uno y otro, con repetir y saltar', async () => {
    await setup(100, 60, false);
    ui().patchConfig({ size: [55, 35], overlap: 10, margin: 0, marks: false });
    await planned();
    const plan = ui().plan;
    expect(plan?.ok ? plan.plan.tiles.length : 0).toBe(4);
    beginSession();
    expect(ui().phase).toBe('place');
    expect(ui().session?.feeder).toBe(false);

    await sendCurrent();
    expect(sent).toHaveLength(1);
    expect(sent[0]!.options.tile).toEqual(ui().session!.specs[0]);
    expect(sent[0]!.options.start_from).toBe('absolute');
    // Terminó el tramo 1: pide colocar el 2 sin enviarlo solo.
    expect(ui().phase).toBe('place');
    expect(ui().session).toMatchObject({ current: 1, status: ['done', 'pending', 'pending', 'pending'] });
    expect(sent).toHaveLength(1);

    skipCurrent();
    expect(ui().session).toMatchObject({ current: 2, status: ['done', 'skipped', 'pending', 'pending'] });
    // El avance queda guardado para retomar.
    expect(loadSession()?.status).toEqual(['done', 'skipped', 'pending', 'pending']);

    await sendCurrent();
    selectTile(0); // repetir el primero
    expect(ui().session).toMatchObject({ current: 0, status: ['pending', 'skipped', 'done', 'pending'] });
    await sendCurrent();
    expect(ui().session!.current).toBe(3);
    await sendCurrent();
    expect(ui().phase).toBe('finished');
    expect(sent).toHaveLength(4); // 0, 2, el 0 repetido y 3
    // Terminada la sesión, no queda nada que retomar.
    expect(loadSession()).toBeNull();
  }, 20000);

  it('un tramo rechazado por el motor deja el asistente en «failed» y permite reintentar', async () => {
    await setup(100, 60, false);
    ui().patchConfig({ size: [55, 35], overlap: 10, margin: 0, marks: false });
    await planned();
    beginSession();
    const original = api.startJob;
    api.startJob = () => Promise.reject(new Error('preflight: alarma activa'));
    await sendCurrent();
    expect(ui().phase).toBe('failed');
    expect(ui().message).toContain('preflight');
    expect(ui().session!.status[0]).toBe('pending');
    api.startJob = original;
    await sendCurrent();
    expect(ui().session!.status[0]).toBe('done');
    expect(ui().phase).toBe('place');
  }, 20000);

  it('con alimentador y una sola columna avanza el material entre tramos sin pedir confirmación', async () => {
    await setup(50, 150, true);
    ui().patchConfig({ size: [55, 70], overlap: 10, margin: 0, marks: false });
    await planned();
    const plan = ui().plan;
    const rows = plan?.ok ? plan.plan.rows : 0;
    expect(rows).toBeGreaterThan(1);
    beginSession();
    expect(ui().session?.feeder).toBe(true);
    const fed0 = api.machine.fedMm;
    await sendCurrent();
    // Un solo clic envió todos los tramos.
    expect(ui().phase).toBe('finished');
    expect(sent).toHaveLength(rows);
    const specs = sent.map((a) => a.options.tile!);
    // Entre tramos: alto menos solape (60), salvo el último, que avanza lo que falta.
    const expected = specs.slice(1).reduce((sum, s, i) => sum + (specs[i]!.rect[0][1] - s.rect[0][1]), 0);
    expect(api.machine.fedMm - fed0).toBeCloseTo(expected);
    expect(api.machine.fedMm - fed0).toBeGreaterThan(0);
  }, 30000);

  it('varias columnas por filas: nunca avanza solo y abre la guía al cambiar de columna', async () => {
    await setup(100, 60, true);
    ui().patchConfig({ size: [55, 35], overlap: 10, margin: 0, marks: true, order: 'rows' });
    await planned();
    beginSession();
    await vi.waitFor(() => expect(ui().marks.length).toBeGreaterThan(0));
    const fed0 = api.machine.fedMm;
    await sendCurrent();
    // Del tramo 1 al 2 cambia la columna: pausa guiada, sin avance del alimentador.
    expect(ui().phase).toBe('place');
    expect(ui().guide).toBe(true);
    expect(api.machine.fedMm).toBe(fed0);
    expect(sent).toHaveLength(1);
    // Enviar cierra la guía; la del tramo siguiente (otra fila y columna) se abre de nuevo.
    await sendCurrent();
    expect(sent).toHaveLength(2);
    expect(ui().session).toMatchObject({ current: 2, last: 1 });
    expect(ui().guide).toBe(true);

  }, 20000);

  it('en serpentina el alimentador avanza entre filas de la misma columna y la guía solo sale al cambiar de columna', async () => {
    await setup(100, 60, true);
    ui().patchConfig({ size: [55, 35], overlap: 10, margin: 0, marks: true, order: 'serpentine' });
    await planned();
    beginSession();
    expect(ui().session?.feeder).toBe(true);
    await vi.waitFor(() => expect(ui().marks.length).toBeGreaterThan(0));
    const fed0 = api.machine.fedMm;
    await sendCurrent(); // tramo 1 -> columna 2: guía
    expect(ui().guide).toBe(true);
    await sendCurrent(); // tramo 2, avance del alimentador y tramo 3 solos
    expect(sent).toHaveLength(3);
    expect(api.machine.fedMm - fed0).toBeGreaterThan(0);
    // Del tramo 3 al 4 vuelve a cambiar la columna.
    expect(ui().phase).toBe('place');
    expect(ui().guide).toBe(true);
    expect(ui().session).toMatchObject({ current: 3, last: 2 });
  }, 30000);

  it('retoma una sesión guardada donde se quedó', async () => {
    await setup(100, 60, false);
    ui().patchConfig({ size: [55, 35], overlap: 10, margin: 0, marks: false });
    await planned();
    beginSession();
    await sendCurrent();
    closeTiling();
    // Como tras reiniciar la app: el estado en memoria se pierde y queda lo guardado.
    ui().set({ session: null, phase: 'config', saved: null });
    openTiling();
    await planned();
    expect(ui().saved?.status[0]).toBe('done');
    resumeSession();
    expect(ui().phase).toBe('place');
    expect(ui().session).toMatchObject({ current: 1 });
    // Con otro diseño ya no se ofrece.
    restartSession();
    useStore.setState({ doc: { version: 2, device_id: null, layers: [factoryLayer('#ff0000', 'Rojo')], objects: [rectObject('a', 120, 60)] } });
    ui().set({ phase: 'config' });
    openTiling();
    await planned();
    expect(ui().saved).toBeNull();
  }, 20000);
});

describe('tileJobArgs', () => {
  const doc = { version: 2 as const, device_id: null, layers: [factoryLayer('#ff0000', 'Rojo')], objects: [rectObject('a', 100, 60)] };
  const args = (selection: string[] | null): JobArgs => ({
    doc,
    device: selectActiveDevice(useStore.getState()),
    options: { start_from: 'current_position', job_origin: 4, user_origin: null, selection, layer_order: 'auto', optimization: useStore.getState().jobOptions.optimization, use_selection_origin: true, transform: null, tile: null },
  });
  const tile = { rect: [[0, 0], [55, 35]], offset: [1, 1] } as JobArgs['options']['tile'];

  it('pone el tramo, fuerza el modo absoluto y añade las marcas en su capa', () => {
    const marks = crosses();
    const out = tileJobArgs(args(null), tile!, marks);
    expect(out.options).toMatchObject({ tile, start_from: 'absolute', use_selection_origin: false, selection: null });
    expect(out.doc.objects).toHaveLength(1 + marks.length / 2);
    expect(out.doc.layers.map((l) => l.color)).toContain('#00a0ff');
    // El documento del usuario no se toca.
    expect(doc.objects).toHaveLength(1);
  });

  it('con «Solo la selección» las marcas entran a la selección', () => {
    const marks = crosses();
    const out = tileJobArgs(args(['a']), tile!, marks);
    expect(out.options.selection).toEqual(['a', ...out.doc.objects.filter((o) => o.id.startsWith('tile-mark-')).map((o) => o.id)]);
  });

  it('sin marcas el documento queda igual', () => {
    expect(tileJobArgs(args(null), tile!, []).doc).toBe(doc);
  });
});
