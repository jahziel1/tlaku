import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import type { HeadCalibration } from '@/api/types';
import { bindMachineEvents, selectActiveDevice, useStore } from '@/store';
import { emptyCameraSetup } from '@/store/camera';
import { selectHeadDoc } from '@/store/machine';
import type { Vec2 } from './geometry';
import { createApiBackend, setSetupBackend } from './setup';
import { headPointMm, planScan } from './head';
import { addObservation, currentFit, moveToNextObservation, saveHeadCalibration, startMeasuring } from './headCal';
import { machineDeps, runScan, scanBed, shotsToTiles, waitForArrival, type Frame } from './headScan';
import { useHeadCal, useHeadUi } from './headState';

const space: [number, number] = [640, 360];

vi.mock('./session', () => ({
  grabCorrectedPng: async () => null,
  grabCorrectedColorPng: async () => null,
  correctedSpace: () => [640, 360] as [number, number],
  setMosaicBackdrop: () => undefined,
  refreshSnapshot: async () => false,
}));

function fakePng(w: number, h: number): string {
  const bytes = new Uint8Array(33);
  bytes.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52]);
  new DataView(bytes.buffer).setUint32(16, w);
  new DataView(bytes.buffer).setUint32(20, h);
  return btoa(String.fromCharCode(...bytes));
}

/** Cámara del cabezal verdadera: 640x360 a 2 px/mm (una foto cubre 320x180 mm), con el desfase de siempre. */
const TRUTH: HeadCalibration = {
  image_size: [640, 360],
  ref_px: [320, 180],
  px_to_mm: [[0.5, 0.004], [0.004, -0.5]],
  offset_mm: [12, -28],
  rms_mm: 0,
  observations: 0,
};

let api: MockApi;
let unbind: (() => void) | null = null;

beforeEach(async () => {
  api = createMockApi({ latencyMs: 0, statusIntervalMs: 15, minJobMs: 100, jobSpeedup: 1e6 });
  setApi(api);
  setSetupBackend(createApiBackend(() => api));
  const s = useStore.getState();
  await s.loadDevices();
  unbind = await bindMachineEvents(api);
  s.setSelectedPort('virtual:falcon2');
  await s.connect();
  await vi.waitFor(() => expect(useStore.getState().status?.state).toBe('idle'));
  s.setCameraSetup({ ...emptyCameraSetup(), head: { enabled: true, calibration: TRUTH } }, true);
  useHeadUi.setState({ confirm: null, progress: null, error: null, cancelRequested: false, mosaic: null });
  useHeadCal.getState().reset();
  useHeadCal.getState().setMark(null);
});

afterEach(async () => {
  unbind?.();
  await useStore.getState().disconnect();
  vi.restoreAllMocks();
});

const frame = (): Frame => ({ png: fakePng(640, 360), width: 640, height: 360 });

describe('recorrido de la cámara del cabezal contra la máquina virtual', () => {
  it('visita las posiciones en orden, con la máquina quieta, y arma el mosaico en la cama', async () => {
    const store = useStore;
    const bed = selectActiveDevice(store.getState()).bed;
    const plan = planScan(bed, TRUTH);
    if ('error' in plan) throw new Error(plan.error);
    expect(plan.stops.length).toBeGreaterThanOrEqual(2);
    const jogs: Parameters<MockApi['jogTo']>[0][] = [];
    const jogTo = api.jogTo.bind(api);
    vi.spyOn(api, 'jogTo').mockImplementation((a) => {
      jogs.push(a);
      return jogTo(a);
    });
    const seen: { state: string | undefined; at: Vec2 | null }[] = [];
    const progress: number[] = [];
    const deps = {
      ...machineDeps(store, (step) => progress.push(step)),
      capture: async () => {
        seen.push({ state: store.getState().status?.state, at: selectHeadDoc(store.getState()) });
        return frame();
      },
    };
    const { shots, cancelled } = await runScan(plan, deps, 0);
    expect(cancelled).toBe(false);
    expect(shots).toHaveLength(plan.stops.length);
    expect(jogs).toHaveLength(plan.stops.length);
    expect(progress).toEqual(plan.stops.map((_, i) => i + 1));
    // En cada foto la máquina estaba en reposo y en el punto previsto (±0.5 mm), en el orden del plan.
    shots.forEach((shot, i) => {
      const want = plan.stops[i]!.laser;
      expect(seen[i]!.state).toBe('idle');
      expect(Math.hypot(shot.laser[0] - want[0], shot.laser[1] - want[1])).toBeLessThan(0.5);
    });
    // La cámara no se movió con el láser encendido: solo jog (láser apagado), sin trabajo.
    expect(store.getState().jobState).not.toBe('running');
    // El mosaico compuesto con las posiciones reales cabe en la cama y cubre la zona.
    const tiles = shotsToTiles(shots, TRUTH, space);
    expect(tiles).toHaveLength(shots.length);
    const img = await api.composeBedImage({
      tiles,
      bed,
      px_per_mm: null,
      grayscale: false,
      feather_px: 20,
      crop_px: 10,
      rect: plan.region,
      dir: null,
      stem: 'prueba',
      save: false,
    });
    expect(img.origin_mm[0]).toBeCloseTo(plan.region[0], 6);
    expect(img.size_mm[0]).toBeGreaterThanOrEqual(plan.region[2] - plan.region[0] - 1e-6);
  });

  it('si se cancela entre fotos, se detiene y devuelve lo capturado', async () => {
    const bed = selectActiveDevice(useStore.getState()).bed;
    const plan = planScan(bed, TRUTH);
    if ('error' in plan) throw new Error(plan.error);
    let taken = 0;
    const deps = {
      ...machineDeps(useStore),
      capture: async () => {
        taken++;
        return frame();
      },
      cancelled: () => taken >= 1,
    };
    const r = await runScan(plan, deps, 0);
    expect(r.cancelled).toBe(true);
    expect(r.shots).toHaveLength(1);
  });

  it('con la máquina en alarma el recorrido se detiene con un error claro', async () => {
    const bed = selectActiveDevice(useStore.getState()).bed;
    const plan = planScan(bed, TRUTH);
    if ('error' in plan) throw new Error(plan.error);
    await api.simulateFault({ fault: 'alarm3' });
    await vi.waitFor(() => expect(useStore.getState().status?.state).toBe('alarm'));
    await expect(runScan(plan, { ...machineDeps(useStore), capture: async () => frame() }, 0)).rejects.toThrow(/no aceptó el movimiento/);
  });

  it('esperar la llegada falla con un mensaje si la máquina no llega a tiempo', async () => {
    await expect(waitForArrival(useStore, [999, 999], 60)).rejects.toThrow(/demasiado en llegar/);
    // Ya en el punto: regresa al instante.
    const here = selectHeadDoc(useStore.getState())!;
    await expect(waitForArrival(useStore, here, 1000)).resolves.toBeUndefined();
  });

  it('scanBed completo: sin calibración o con otra resolución explica por qué no', async () => {
    const s = useStore.getState();
    s.setCameraSetup({ ...emptyCameraSetup(), head: { enabled: true, calibration: null } }, true);
    await expect(scanBed({ save: false, useSelection: false })).rejects.toThrow(/Calibra primero/);
    s.setCameraSetup({ ...emptyCameraSetup(), head: { enabled: true, calibration: { ...TRUTH, image_size: [1280, 720] } } }, true);
    await expect(scanBed({ save: false, useSelection: false })).rejects.toThrow(/otra resolución/);
  });
});

describe('calibración del desfase cámara-láser contra la máquina virtual', () => {
  /** Lo que verá la cámara verdadera: el píxel donde cae la marca con el láser donde está ahora. */
  function clickOn(mark: Vec2): Vec2 {
    const laser = selectHeadDoc(useStore.getState())!;
    const [[a, b], [c, d]] = TRUTH.px_to_mm;
    const rx = mark[0] - laser[0] - TRUTH.offset_mm[0];
    const ry = mark[1] - laser[1] - TRUTH.offset_mm[1];
    const det = a * d - b * c;
    return [TRUTH.ref_px[0] + (d * rx - b * ry) / det, TRUTH.ref_px[1] + (-c * rx + a * ry) / det];
  }

  it('mover el cabezal, marcar la cruz cuatro veces y guardar da el desfase real', async () => {
    const mark: Vec2 = [180, 120];
    useStore.getState().setCameraSetup({ ...emptyCameraSetup(), head: { enabled: true, calibration: null } }, true);
    useHeadCal.getState().setMark(mark);
    // El usuario lleva el cabezal hasta que la cruz se ve (la cámara va 12 mm a la derecha y 28 abajo del láser).
    await useStore.getState().goToDocPoint(mark[0] - TRUTH.offset_mm[0] + 2, mark[1] - TRUTH.offset_mm[1] - 1.5);
    await vi.waitFor(() => expect(useStore.getState().status?.state).toBe('idle'));
    startMeasuring();
    for (let i = 0; i < 4; i++) {
      await vi.waitFor(() => expect(useStore.getState().status?.state).toBe('idle'));
      // Un clic con 0.3 px de error.
      const p = clickOn(mark);
      expect(addObservation([p[0] + 0.3 * Math.sin(i * 7), p[1] + 0.3 * Math.cos(i * 5)])).toBeNull();
      if (i < 3) expect(await moveToNextObservation()).toBe(true);
    }
    const fit = currentFit();
    if (!('calibration' in fit)) throw new Error(String(fit.error));
    const c = fit.calibration;
    expect(Math.hypot(c.offset_mm[0] - TRUTH.offset_mm[0], c.offset_mm[1] - TRUTH.offset_mm[1])).toBeLessThan(0.15);
    expect(c.rms_mm).toBeLessThan(0.15);
    saveHeadCalibration(c);
    const saved = useStore.getState().cameraSetup.head!;
    expect(saved.enabled).toBe(true);
    expect(saved.calibration?.offset_mm[0]).toBeCloseTo(c.offset_mm[0], 9);
    expect(useHeadCal.getState().obs).toHaveLength(0);
    // Con la calibración, el píxel donde se ve la marca cae sobre la marca (< 0.2 mm) desde otra posición.
    const laser = selectHeadDoc(useStore.getState())!;
    const est = headPointMm(saved.calibration!, laser, clickOn(mark));
    expect(Math.hypot(est[0] - mark[0], est[1] - mark[1])).toBeLessThan(0.2);
  });

  it('sin posición del cabezal (máquina desconectada) no registra puntos', async () => {
    await useStore.getState().disconnect();
    expect(addObservation([10, 10])).toBe('noPosition');
  });
});
