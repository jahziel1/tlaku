import { beforeEach, describe, expect, it } from 'vitest';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { setApi } from '@/api';
import { screenToWorld, worldToScreen } from '@/lib/viewport';
import { createAppStore, selectActiveDevice, bindMachineEvents } from './index';
import { HISTORY_LIMIT } from './document';

let api: MockApi;
let store: ReturnType<typeof createAppStore>;

beforeEach(() => {
  api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 200, jobSpeedup: 1e6 });
  setApi(api);
  store = createAppStore();
});

const importSample = () =>
  store.getState().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });

describe('document', () => {
  it('importa objetos y capas y lo registra en el historial', () => {
    importSample();
    const s = store.getState();
    expect(s.doc.objects).toHaveLength(4);
    expect(s.doc.layers).toHaveLength(4);
    expect(s.dirty).toBe(true);
    expect(s.past).toHaveLength(1);
  });

  it('no repite ids al importar dos veces ni duplica capas', () => {
    importSample();
    importSample();
    const ids = store.getState().doc.objects.map((o) => o.id);
    expect(new Set(ids).size).toBe(8);
    expect(store.getState().doc.layers).toHaveLength(4);
  });

  it('crea la capa que usan los objetos aunque el motor no la devuelva', () => {
    store.getState().applyImport({ objects: sampleObjects(), new_layers: [], warnings: [] });
    expect(store.getState().doc.layers.length).toBeGreaterThan(0);
  });

  it('deshacer y rehacer restauran el documento', () => {
    importSample();
    store.getState().deleteObjects(['obj-placa']);
    expect(store.getState().doc.objects.some((o) => o.id === 'obj-placa')).toBe(false);
    store.getState().undo();
    expect(store.getState().doc.objects.some((o) => o.id === 'obj-placa')).toBe(true);
    store.getState().redo();
    expect(store.getState().doc.objects.some((o) => o.id === 'obj-placa')).toBe(false);
    expect(store.getState().canRedo()).toBe(false);
  });

  it('una acción nueva descarta lo que se podía rehacer', () => {
    importSample();
    store.getState().deleteObjects(['obj-placa']);
    store.getState().undo();
    store.getState().deleteObjects(['obj-foto']);
    expect(store.getState().canRedo()).toBe(false);
  });

  it('mover con la misma clave se funde en un paso; con otra, no', () => {
    importSample();
    const base = store.getState().past.length;
    store.getState().moveObjects(['obj-placa'], 1, 0, 'k');
    store.getState().moveObjects(['obj-placa'], 1, 0, 'k');
    expect(store.getState().past.length).toBe(base + 1);
    store.getState().moveObjects(['obj-placa'], 1, 0, 'otra');
    expect(store.getState().past.length).toBe(base + 2);
  });

  it('no mueve ni elimina objetos bloqueados', () => {
    importSample();
    store.getState().setObjectFlags(['obj-placa'], { locked: true });
    store.getState().moveObjects(['obj-placa'], 5, 5);
    store.getState().deleteObjects(['obj-placa']);
    const o = store.getState().doc.objects.find((x) => x.id === 'obj-placa')!;
    expect(o.transform[4]).toBe(60);
  });

  it('duplicar devuelve ids nuevos y desplazados', () => {
    importSample();
    const [id] = store.getState().duplicateObjects(['obj-placa']);
    expect(id).toBeDefined();
    const copy = store.getState().doc.objects.find((o) => o.id === id)!;
    expect(copy.transform[4]).toBe(65);
  });

  it('el historial tiene un tope', () => {
    importSample();
    for (let i = 0; i < HISTORY_LIMIT + 20; i++) store.getState().moveObjects(['obj-placa'], 1, 0, `k${i}`);
    expect(store.getState().past.length).toBeLessThanOrEqual(HISTORY_LIMIT);
  });

  it('mover a otra capa la crea si hace falta', () => {
    importSample();
    store.getState().setObjectsLayer(['obj-placa'], '#00c2e0');
    expect(store.getState().doc.objects.find((o) => o.id === 'obj-placa')!.layer).toBe('#00c2e0');
    expect(store.getState().doc.layers.some((l) => l.color === '#00c2e0')).toBe(true);
  });
});

describe('selection', () => {
  it('seleccionar todo omite lo oculto y lo bloqueado', () => {
    importSample();
    store.getState().setObjectFlags(['obj-foto'], { hidden: true });
    store.getState().selectAll();
    expect(store.getState().selectedIds).not.toContain('obj-foto');
    expect(store.getState().selectedIds).toHaveLength(3);
  });

  it('eliminar quita lo eliminado de la selección', () => {
    importSample();
    store.getState().select(['obj-placa', 'obj-foto']);
    store.getState().deleteObjects(['obj-placa']);
    expect(store.getState().selectedIds).toEqual(['obj-foto']);
  });

  it('seleccionar una capa suma con shift', () => {
    importSample();
    store.getState().selectLayerObjects('#ff3b4a');
    store.getState().selectLayerObjects('#2f7bff', true);
    expect(store.getState().selectedIds.sort()).toEqual(['obj-engranaje', 'obj-placa']);
  });
});

describe('view', () => {
  it('zoomAt mantiene fijo el punto bajo el cursor', () => {
    const s = store.getState();
    s.setViewport(1000, 800);
    const anchor: [number, number] = [300, 200];
    const before = screenToWorld(store.getState(), anchor);
    store.getState().zoomAt(1.7, ...anchor);
    const after = screenToWorld(store.getState(), anchor);
    expect(after[0]).toBeCloseTo(before[0], 6);
    expect(after[1]).toBeCloseTo(before[1], 6);
  });

  it('worldToScreen y screenToWorld son inversas, con Y hacia arriba', () => {
    const v = { zoom: 2, panX: 10, panY: 500 };
    expect(worldToScreen(v, [0, 0])).toEqual([10, 500]);
    expect(worldToScreen(v, [0, 10])[1]).toBeLessThan(500);
    expect(screenToWorld(v, worldToScreen(v, [12.5, 7.25]))).toEqual([12.5, 7.25]);
  });

  it('encuadra la cama dentro del lienzo y reencuadra al cambiar el tamaño', () => {
    store.getState().setViewport(1000, 800);
    const [bw, bh] = selectActiveDevice(store.getState()).bed;
    const s = store.getState();
    expect(bw * s.zoom).toBeLessThanOrEqual(1000);
    expect(bh * s.zoom).toBeLessThanOrEqual(800);
    store.getState().setViewport(500, 400);
    expect(store.getState().zoom).toBeLessThan(s.zoom);
  });

  it('tras mover la vista a mano ya no reencuadra solo', () => {
    store.getState().setViewport(1000, 800);
    store.getState().panBy(5, 5);
    const z = store.getState().zoom;
    store.getState().setViewport(500, 400);
    expect(store.getState().zoom).toBe(z);
  });
});

describe('devices y preview con la API simulada', () => {
  it('carga los dispositivos y elige el del documento', async () => {
    await store.getState().loadDevices();
    expect(store.getState().devices.length).toBeGreaterThan(1);
    store.getState().setActiveDevice('taller-co2');
    expect(selectActiveDevice(store.getState()).bed).toEqual([300, 200]);
    expect(store.getState().doc.device_id).toBe('taller-co2');
  });

  it('calcula la vista previa', async () => {
    await store.getState().loadDevices();
    importSample();
    await store.getState().computePreview();
    const s = store.getState();
    expect(s.previewStatus).toBe('ready');
    expect(s.preview!.moves.length % 4).toBe(0);
    expect(s.preview!.total_s).toBeGreaterThan(0);
  });
});

describe('máquina', () => {
  it('conecta, recibe estado y termina un trabajo', async () => {
    await store.getState().loadDevices();
    importSample();
    const off = await bindMachineEvents(api, store);
    await store.getState().refreshPorts();
    await store.getState().connect();
    expect(store.getState().connection.connected).toBe(true);
    expect(store.getState().firmware).toMatch(/Grbl/);
    await store.getState().startJob();
    expect(store.getState().jobState).toBe('running');
    await new Promise((r) => setTimeout(r, 600));
    expect(store.getState().jobState).toBe('finished');
    expect(store.getState().progress?.acked).toBe(store.getState().progress?.total);
    await store.getState().disconnect();
    expect(store.getState().connection.connected).toBe(false);
    off();
  });

  it('avisa si se intenta iniciar sin conexión', async () => {
    await store.getState().loadDevices();
    importSample();
    await store.getState().startJob();
    expect(store.getState().notices.at(-1)?.level).toBe('error');
  });

  it('el registro se recorta al tope', async () => {
    for (let i = 0; i < 600; i++) store.getState().onLog({ dir: 'in', text: String(i) });
    expect(store.getState().log).toHaveLength(500);
    expect(store.getState().log.at(-1)?.text).toBe('599');
  });
});
