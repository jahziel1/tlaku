import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi, type JobPreview, type LaserApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { createAppStore } from '@/store';
import { startPreviewSync } from './sync';

/** API cuyo `computeJob` se resuelve a mano, para controlar el orden de las respuestas. */
function controlledApi() {
  const pending: { resolve: (p: JobPreview) => void; reject: (e: unknown) => void }[] = [];
  const base = createMockApi({ latencyMs: 0 });
  const api: LaserApi = {
    ...base,
    computeJob: () => new Promise<JobPreview>((resolve, reject) => pending.push({ resolve, reject })),
  };
  return { api, pending };
}

const preview = (total: number): JobPreview => ({
  layers: [],
  moves: [0, 1, 1, 0],
  move_times: [total],
  total_s: total,
  bounds: [[0, 0], [1, 1]],
  order_note: null,
  warnings: [],
});

describe('cálculo de la vista previa: respuestas obsoletas', () => {
  it('una respuesta vieja no pisa a una nueva', async () => {
    const { api, pending } = controlledApi();
    setApi(api);
    const store = createAppStore();
    const first = store.getState().computePreview();
    const second = store.getState().computePreview();
    expect(pending).toHaveLength(2);
    pending[1]!.resolve(preview(20));
    await second;
    expect(store.getState().preview?.total_s).toBe(20);
    pending[0]!.resolve(preview(10)); // llega tarde
    await first;
    expect(store.getState().preview?.total_s).toBe(20);
    expect(store.getState().previewStatus).toBe('ready');
  });

  it('un error viejo tampoco pisa un resultado nuevo', async () => {
    const { api, pending } = controlledApi();
    setApi(api);
    const store = createAppStore();
    const first = store.getState().computePreview();
    const second = store.getState().computePreview();
    pending[1]!.resolve(preview(5));
    await second;
    pending[0]!.reject(new Error('se sale de la cama'));
    await first;
    expect(store.getState().previewStatus).toBe('ready');
    expect(store.getState().previewError).toBeNull();
  });

  it('invalidar descarta el cálculo en vuelo y marca la vista previa como vencida', async () => {
    const { api, pending } = controlledApi();
    setApi(api);
    const store = createAppStore();
    const inflight = store.getState().computePreview();
    expect(store.getState().previewStale).toBe(false);
    store.getState().invalidatePreview();
    expect(store.getState().previewStale).toBe(true);
    pending[0]!.resolve(preview(99));
    await inflight;
    expect(store.getState().preview).toBeNull();
    expect(store.getState().previewStatus).toBe('computing'); // sigue esperando el cálculo nuevo
  });

  it('un error del motor queda como texto legible y quita la vista previa', async () => {
    const { api, pending } = controlledApi();
    setApi(api);
    const store = createAppStore();
    const p = store.getState().computePreview();
    pending[0]!.reject('[bed_overflow] El trabajo, con el margen de overscan, se sale de la cama.');
    await p;
    const s = store.getState();
    expect(s.previewStatus).toBe('error');
    expect(s.previewError).toContain('se sale de la cama');
    expect(s.previewError).not.toContain('[bed_overflow]');
    expect(s.previewErrorCode).toBe('bed_overflow');
    expect(s.preview).toBeNull();
  });
});

describe('sincronización con debounce', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  function setup(visible: boolean) {
    const { api, pending } = controlledApi();
    setApi(api);
    const store = createAppStore();
    store.getState().setPreviewVisible(visible);
    const stop = startPreviewSync(store, { visibleMs: 400, hiddenMs: 1500 });
    return { store, pending, stop };
  }
  const edit = (store: ReturnType<typeof createAppStore>) =>
    store.getState().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });

  it('agrupa varios cambios seguidos en un solo cálculo', async () => {
    const { store, pending, stop } = setup(true);
    await vi.advanceTimersByTimeAsync(400);
    expect(pending).toHaveLength(1); // el cálculo inicial
    pending[0]!.resolve(preview(1));
    await vi.advanceTimersByTimeAsync(0);
    edit(store);
    await vi.advanceTimersByTimeAsync(200);
    edit(store);
    await vi.advanceTimersByTimeAsync(399);
    expect(pending).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(pending).toHaveLength(2);
    stop();
  });

  it('un cambio durante un cálculo en vuelo descarta esa respuesta', async () => {
    const { store, pending, stop } = setup(true);
    await vi.advanceTimersByTimeAsync(400);
    expect(pending).toHaveLength(1);
    edit(store); // el documento cambia mientras el motor calcula
    pending[0]!.resolve(preview(7)); // la respuesta vieja llega antes del recálculo
    await vi.advanceTimersByTimeAsync(0);
    expect(store.getState().preview).toBeNull();
    await vi.advanceTimersByTimeAsync(400);
    expect(pending).toHaveLength(2);
    pending[1]!.resolve(preview(8));
    await vi.advanceTimersByTimeAsync(0);
    expect(store.getState().preview?.total_s).toBe(8);
    expect(store.getState().previewStale).toBe(false);
    stop();
  });

  it('con la vista previa cerrada espera más, y al abrirla calcula de inmediato', async () => {
    const { store, pending, stop } = setup(false);
    await vi.advanceTimersByTimeAsync(1499);
    expect(pending).toHaveLength(0);
    store.getState().setPreviewVisible(true); // abrir: no espera el debounce
    expect(pending).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1500);
    expect(pending).toHaveLength(1); // el temporizador pendiente ya no hace falta
    stop();
  });

  it('al abrir con el resultado al día no vuelve a calcular', async () => {
    const { store, pending, stop } = setup(false);
    await vi.advanceTimersByTimeAsync(1500);
    expect(pending).toHaveLength(1);
    pending[0]!.resolve(preview(3));
    await vi.advanceTimersByTimeAsync(0);
    store.getState().setPreviewVisible(true);
    expect(pending).toHaveLength(1);
    stop();
  });

  it('cambiar el dispositivo activo o las opciones del trabajo también recalcula', async () => {
    const { store, pending, stop } = setup(true);
    await vi.advanceTimersByTimeAsync(400);
    pending[0]!.resolve(preview(1));
    await vi.advanceTimersByTimeAsync(0);
    store.getState().setJobOptions({ layer_order: 'file' });
    await vi.advanceTimersByTimeAsync(400);
    expect(pending).toHaveLength(2);
    stop();
  });

  it('detenerlo cancela los temporizadores', async () => {
    const { store, pending, stop } = setup(true);
    edit(store);
    stop();
    await vi.advanceTimersByTimeAsync(5000);
    expect(pending).toHaveLength(0);
  });
});

describe('reproducción en el store', () => {
  beforeEach(() => setApi(createMockApi({ latencyMs: 0 })));
  const withPreview = () => {
    const store = createAppStore();
    store.setState({ preview: preview(10), previewVisible: true });
    return store;
  };

  it('reproducir al final reinicia desde el principio y sin vista previa no hace nada', () => {
    const empty = createAppStore();
    empty.getState().setPlaying(true);
    expect(empty.getState().playing).toBe(false);
    const store = withPreview();
    store.getState().setPlaying(true);
    expect(store.getState().playing).toBe(true);
    expect(store.getState().scrub).toBe(0);
  });

  it('cerrar la vista previa detiene la reproducción', () => {
    const store = withPreview();
    store.getState().setPlaying(true);
    store.getState().setPreviewVisible(false);
    expect(store.getState().playing).toBe(false);
  });

  it('la velocidad sube y baja sin salirse de 1x..50x', () => {
    const store = createAppStore();
    for (let i = 0; i < 10; i++) store.getState().stepPlaySpeed(1);
    expect(store.getState().playSpeed).toBe(50);
    store.getState().stepPlaySpeed(-1);
    expect(store.getState().playSpeed).toBe(10);
    for (let i = 0; i < 10; i++) store.getState().stepPlaySpeed(-1);
    expect(store.getState().playSpeed).toBe(1);
  });

  it('cambiar una opción visual renueva `previewView` sin tocar los datos de `preview`', () => {
    const store = withPreview();
    const before = store.getState().preview;
    const view = store.getState().previewView;
    store.getState().setPreviewView({ showTravel: false });
    expect(store.getState().previewView).not.toBe(view);
    expect(store.getState().preview).toBe(before);
    expect(store.getState().previewView.showTravel).toBe(false);
  });
});
