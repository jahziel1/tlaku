import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { TonalPreview } from './types';
import type { DesignObject, Document } from '@/api/types';
import { mockComputeJob } from '@/api/mock/job';
import { mockTonalRender } from '@/api/mock/tonal';
import { polylineBez } from '@/lib/bezpath';
import { FALLBACK_DEVICE } from '@/lib/device';
import { makeLayer } from '@/lib/layers';
import { useStore } from '@/store';
import { useMaterials } from '@/features/materials/state';
import { emptyLibrary } from '@/features/materials/library';
import { presetCurve } from './curve';
import { clearTonalLayers, getTonalLayers } from './layers';
import { startTonalService, DETAIL_DEBOUNCE_MS } from './service';
import { useTonal } from './store';

const RED = '#ff3b4a';
const BLUE = '#2f7bff';

const rect = (id: string, layer: string, x: number, y: number, w: number, h: number): DesignObject => ({
  id,
  layer,
  transform: [1, 0, 0, 1, 0, 0],
  kind: { type: 'vector', paths: [polylineBez([[x, y], [x + w, y], [x + w, y + h], [x, y + h]], true)] },
  locked: false,
  hidden: false,
  group: null,
  priority: null,
  power_scale: null,
  shape: null,
  start: null,
  manual_tabs: [],
  array: null,
});

const image = (id: string, layer: string, x: number, y: number, w: number, h: number): DesignObject => ({
  ...rect(id, layer, 0, 0, 1, 1),
  transform: [1, 0, 0, 1, x, y],
  kind: { type: 'image', source_path: 'foto.png', width_px: 100, height_px: 100, size_mm: [w, h], mask: [], variable: null },
});

function docWith(objects: DesignObject[], ...layers: ReturnType<typeof makeLayer>[]): Document {
  return { version: 2, objects, layers, device_id: null } as unknown as Document;
}

/** Las operaciones del trabajo simulado de un documento, como las usa la vista por tonos del modo simulado. */
function opsOf(doc: Document) {
  const ops: Parameters<typeof mockTonalRender>[0] = [];
  mockComputeJob(doc, FALLBACK_DEVICE, { start_from: 'absolute', job_origin: 6, user_origin: null, selection: null, layer_order: 'auto' } as never, (b) => ops.push(b));
  return ops;
}

const request = (patch: Partial<Parameters<typeof mockTonalRender>[1]> = {}) => ({ curve: presetCurve('light_wood'), spot_mm: 0.1, region_mm: null, max_px: 400, ...patch });

describe('simulación del modo simulado', () => {
  const square = (patch: Parameters<typeof makeLayer>[2]) => docWith([rect('a', RED, 10, 10, 20, 20)], makeLayer(RED, 'Corte', patch));
  const dose = (patch: Parameters<typeof makeLayer>[2]) => mockTonalRender(opsOf(square(patch)), request({ max_px: 800 })).max_dose;

  it('la dosis sube con la potencia y las pasadas y baja con la velocidad', () => {
    const base = dose({ power: 30, speed: 1200, passes: 1 });
    expect(base).toBeGreaterThan(0);
    expect(dose({ power: 60, speed: 1200, passes: 1 }) / base).toBeCloseTo(2, 1);
    expect(dose({ power: 30, speed: 600, passes: 1 }) / base).toBeCloseTo(2, 1);
    expect(dose({ power: 30, speed: 1200, passes: 3 }) / base).toBeCloseTo(3, 1);
  });

  it('la hoja es lo grabado más 3 mm y la imagen cubre la región pedida', () => {
    const out = mockTonalRender(opsOf(square({ power: 30 })), request());
    expect(out.sheet_mm).toEqual([7, 7, 33, 33]);
    expect(out.width_px).toBeLessThanOrEqual(400);
    const zoom = mockTonalRender(opsOf(square({ power: 30 })), request({ region_mm: [8, 8, 14, 14], max_px: 120 }));
    expect(zoom.covered_mm[0]).toBe(8);
    expect(zoom.px_mm).toBeLessThan(out.px_mm);
  });

  it('una foto tramada tiene puntos: píxeles encendidos y apagados lado a lado', () => {
    const doc = docWith([image('foto', BLUE, 20, 20, 30, 30)], makeLayer(BLUE, 'Foto', { mode: 'image', interval: 0.1, speed: 1000, power: 100 }));
    const zoom = mockTonalRender(opsOf(doc), request({ region_mm: [30, 30, 32, 32], max_px: 200 }));
    expect(zoom.max_dose).toBeGreaterThan(0.3);
    const full = mockTonalRender(opsOf(doc), request({ max_px: 300 }));
    expect(full.max_dose).toBeGreaterThan(0);
  });

  it('sin nada que grabar o con la zona fuera de la hoja explica el motivo', () => {
    expect(() => mockTonalRender([], request())).toThrow();
    expect(() => mockTonalRender(opsOf(square({ power: 30 })), request({ region_mm: [500, 500, 520, 520] }))).toThrow();
  });
});

describe('servicio de la vista por tonos', () => {
  let stop: () => void = () => {};
  let calls: unknown[] = [];

  beforeEach(() => {
    vi.useFakeTimers();
    (globalThis as { createImageBitmap?: unknown }).createImageBitmap = async () => ({ width: 1, height: 1 });
    const api = createMockApi({ latencyMs: 0 });
    const original = api.tonalPreview;
    api.tonalPreview = (a): Promise<TonalPreview> => {
      calls.push(a.request);
      return original(a);
    };
    setApi(api);
    calls = [];
    useMaterials.setState({ library: emptyLibrary() });
    useTonal.setState({ mode: 'normal', material: 'light_wood', spotMm: 0.1, status: 'idle', error: null, calibrateOpen: false });
    const s = useStore.getState();
    s.replaceDocument(docWith([rect('a', RED, 10, 10, 20, 20)], makeLayer(RED, 'Corte', { power: 40, speed: 1000 })), null);
    useStore.setState({ previewVisible: true });
  });

  afterEach(async () => {
    stop();
    clearTonalLayers();
    vi.useRealTimers();
  });

  /** Calcula el trabajo (la API simulada responde con un temporizador, que aquí es falso). */
  const compute = async () => {
    const p = useStore.getState().computePreview();
    await vi.advanceTimersByTimeAsync(50);
    await p;
  };

  const settle = async () => {
    for (let i = 0; i < 20; i++) await vi.advanceTimersByTimeAsync(DETAIL_DEBOUNCE_MS);
  };

  it('no pide nada en modo normal ni sin vista previa; al pasar a «por tonos» pide la hoja entera', async () => {
    stop = startTonalService();
    await compute();
    await settle();
    expect(calls).toHaveLength(0);
    useTonal.getState().setMode('tonal');
    await settle();
    expect(calls).toHaveLength(1);
    expect((calls[0] as { region_mm: unknown }).region_mm).toBeNull();
    expect(useTonal.getState().status).toBe('ready');
    expect(getTonalLayers().full).not.toBeNull();
    expect(getTonalLayers().sheet).toEqual([7, 7, 33, 33]);
  });

  it('cambiar de material o de punto vuelve a pedir; volver a normal suelta la imagen', async () => {
    stop = startTonalService();
    await compute();
    useTonal.getState().setMode('tonal');
    await settle();
    const before = calls.length;
    useTonal.getState().setMaterial('leather');
    await settle();
    expect(calls.length).toBeGreaterThan(before);
    expect((calls.at(-1) as { curve: unknown }).curve).toEqual(presetCurve('leather'));
    const n = calls.length;
    useTonal.getState().setSpot(0.2);
    await settle();
    expect(calls.length).toBeGreaterThan(n);
    expect((calls.at(-1) as { spot_mm: number }).spot_mm).toBe(0.2);
    useTonal.getState().setMode('normal');
    await settle();
    expect(getTonalLayers().full).toBeNull();
    expect(useTonal.getState().status).toBe('idle');
  });

  it('una calibración guardada cambia la curva que se pide', async () => {
    stop = startTonalService();
    await compute();
    useTonal.getState().setMode('tonal');
    await settle();
    const tuned = { ...presetCurve('light_wood'), dose_char: 0.77 };
    useMaterials.setState({ library: { ...emptyLibrary(), tones: [{ material: 'light_wood', curve: tuned, samples: [] }] } });
    await settle();
    expect((calls.at(-1) as { curve: unknown }).curve).toEqual(tuned);
  });

  it('al acercar el zoom pide el detalle de lo que se ve', async () => {
    stop = startTonalService();
    await compute();
    useTonal.getState().setMode('tonal');
    await settle();
    const full = calls.length;
    // 400 px/mm es mucho más fino que la imagen general: hace falta el detalle de una zona pequeña.
    useStore.setState({ zoom: 400, panX: -7200, panY: 8400, viewport: { width: 800, height: 600 } });
    await settle();
    expect(calls.length).toBe(full + 1);
    const detail = calls.at(-1) as { region_mm: number[] | null; max_px: number };
    expect(detail.region_mm).not.toBeNull();
    expect(getTonalLayers().detail).not.toBeNull();
  });
});
