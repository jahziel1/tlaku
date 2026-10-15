import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import type { HeadAlignFrame, HeadAlignPlan, HeadAlignReport, LensCalibration } from '@/api/types';
import { bindMachineEvents, useStore } from '@/store';
import { emptyCameraSetup } from '@/store/camera';
import { persistCameraSetup } from './actions';
import {
  AlignCancelled,
  edgeAllSeen,
  freshPositions,
  headAlignBlockedReason,
  headAlignConfig,
  headAlignTiming,
  liveIo,
  runHeadAlign,
  saveBlockedReason,
  saveHeadAlignment,
  startHeadAlign,
  type AlignIo,
} from './headAlign';
import { DEFAULT_HEAD_ALIGN, loadHeadAlignSettings, sanitizeSettings, useHeadAlign } from './headAlignState';
import { createApiBackend, setSetupBackend } from './setup';

let space: [number, number] | null = [800, 600];
let frames = 0;

vi.mock('./session', () => ({
  grabCorrectedPng: async () => ({ png: 'AAAA', width: 800, height: 600 }),
  correctedSpace: () => space,
  framesRendered: () => (frames += 4),
  startCamera: async () => undefined,
}));

const LENS: LensCalibration = {
  width: 800,
  height: 600,
  fx: 400,
  fy: 400,
  cx: 400,
  cy: 300,
  k1: -0.03,
  k2: 0,
  p1: 0,
  p2: 0,
  k3: 0,
  model: 'fisheye',
  k4: 0,
  view_zoom: 0.6,
  rms_error_px: 0.2,
  views_used: 10,
};

const BED: [number, number] = [400, 415];

/** Mundo falso: anota los movimientos y cada foto lleva la posición del cabezal en el texto. */
function fakeIo(log: string[] = [], onMove?: (n: number) => void): AlignIo {
  let at = '';
  return {
    moveTo: async (mm) => {
      at = `${mm[0]},${mm[1]}`;
      log.push(`move ${at}`);
      onMove?.(log.length);
    },
    capture: async () => {
      log.push(`shot ${at}`);
      return `png@${at}`;
    },
    thumbnail: async () => `thumb@${at}`,
  };
}

beforeEach(async () => {
  space = [800, 600];
  frames = 0;
  useStore.getState().setCameraSetup(emptyCameraSetup(), true);
  useHeadAlign.setState({ ...useHeadAlign.getState(), open: false, cancelRequested: false, shots: [], report: null, frames: [], error: null, settings: DEFAULT_HEAD_ALIGN });
  useHeadAlign.getState().backToConfig();
});

describe('ajustes', () => {
  it('se saneaan a valores que el motor acepta', () => {
    expect(sanitizeSettings({})).toEqual(DEFAULT_HEAD_ALIGN);
    const s = sanitizeSettings({ grid: 99, marginMm: -4, method: 'otro' as never, cameraHeightMm: -3, featureHeightMm: 9999, offset: [500, Number.NaN] });
    expect(s.grid).toBe(5);
    expect(s.marginMm).toBe(0);
    expect(s.method).toBe('auto');
    expect(s.cameraHeightMm).toBeNull();
    expect(s.featureHeightMm).toBe(500);
    expect(s.offset).toEqual([100, 0]);
    expect(sanitizeSettings({ grid: 1 }).grid).toBe(3);
  });

  it('se recuerdan y un dato dañado vuelve a lo de fábrica', () => {
    const store = new Map<string, string>();
    vi.stubGlobal('localStorage', { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v), removeItem: (k: string) => void store.delete(k) });
    useHeadAlign.getState().setSettings({ grid: 4, cameraHeightMm: 250, featureHeightMm: 40 });
    expect(loadHeadAlignSettings()).toMatchObject({ grid: 4, cameraHeightMm: 250, featureHeightMm: 40 });
    localStorage.setItem('tlaku.headAlign.settings', '{no es json');
    expect(loadHeadAlignSettings()).toEqual(DEFAULT_HEAD_ALIGN);
    vi.unstubAllGlobals();
  });

  it('el paralaje solo se manda con las dos alturas y la cámara más alta que el punto', () => {
    const s = { ...DEFAULT_HEAD_ALIGN, cameraHeightMm: 250, featureHeightMm: 40, offset: [1, 2] as [number, number] };
    expect(headAlignConfig(s, BED, LENS, null)).toMatchObject({ lens: LENS, estimate_lens: false, offset_mm: [1, 2], parallax: { camera_height_mm: 250, feature_height_mm: 40 } });
    expect(headAlignConfig({ ...s, cameraHeightMm: 42 }, BED, null, null).parallax).toBeNull();
    expect(headAlignConfig({ ...s, featureHeightMm: null }, BED, null, null)).toMatchObject({ parallax: null, estimate_lens: true, lens: null });
  });
});

describe('plan y rondas', () => {
  const plan: HeadAlignPlan = { points: [[20, 20], [200, 20], [380, 20], [380, 207], [200, 207], [20, 207], [20, 395], [200, 395], [380, 395]], reference: [110, 20], area: [20, 20, 380, 395] };

  it('las posiciones con foto no se repiten', () => {
    const done: HeadAlignFrame[] = [{ png_base64: 'x', machine_mm: [20, 20.4] }];
    const fresh = freshPositions(plan, done);
    expect(fresh).toHaveLength(9);
    expect(fresh).not.toContainEqual([20, 20]);
    expect(fresh.at(-1)).toEqual([110, 20]);
  });

  it('se amplía la rejilla solo si el cabezal se vio en todo su borde', () => {
    expect(edgeAllSeen(plan, plan.points)).toBe(true);
    expect(edgeAllSeen(plan, plan.points.filter((p) => p[0] !== 380))).toBe(false);
    // El punto del centro no está en el borde: no hace falta verlo.
    expect(edgeAllSeen(plan, plan.points.filter((p) => !(p[0] === 200 && p[1] === 207)))).toBe(true);
  });
});

describe('runHeadAlign', () => {
  let api: MockApi;
  beforeEach(() => {
    api = createMockApi({ latencyMs: 0 });
    setApi(api);
  });

  it('recorre la rejilla, toma una foto por punto y resuelve con todas', async () => {
    const log: string[] = [];
    const solve = vi.spyOn(api, 'cameraHeadAlignSolve');
    const r = await runHeadAlign({ api, io: fakeIo(log), settings: { ...DEFAULT_HEAD_ALIGN, marginMm: 60 }, bed: BED, lens: LENS });
    // Primera ronda: 3x3 más la referencia, alternando mover y fotografiar.
    expect(log.slice(0, 4)).toEqual(['move 60,60', 'shot 60,60', 'move 200,60', 'shot 200,60']);
    expect(log.slice(0, 20).filter((l) => l.startsWith('move'))).toHaveLength(10);
    expect(r.frames.map((f) => f.png_base64)).toEqual(log.filter((l) => l.startsWith('shot')).map((l) => `png@${l.slice(5)}`));
    expect(r.frames[2].machine_mm).toEqual([340, 60]);
    expect(r.frames[9].machine_mm).toEqual([130, 60]);
    expect(solve).toHaveBeenCalledWith({ frames: r.frames, config: expect.objectContaining({ lens: LENS, safe_area: [60, 60, 340, 355] }) });
    // El mock «ve» el cabezal en la zona central: el resto se descarta y la rejilla se afina.
    expect(r.rounds).toBeGreaterThanOrEqual(1);
    const ui = useHeadAlign.getState();
    expect(ui.shots.length).toBe(r.frames.length);
    expect(ui.shots.every((s) => s.state === 'shot')).toBe(true);
    await vi.waitFor(() => expect(useHeadAlign.getState().shots.every((s) => s.thumb?.startsWith('thumb@'))).toBe(true));
    expect(r.report.detected).toBeGreaterThanOrEqual(5);
    expect(r.report.alignment?.method).toBe('head_sweep');
    expect(r.area).toEqual([60, 60, 340, 355]);
  });

  it('con todo a la vista no hay rondas de afinado', async () => {
    const full = { ...api, cameraHeadAlignSolve: async ({ frames: f }: { frames: HeadAlignFrame[] }) => ({ ...(await api.cameraHeadAlignSolve({ frames: f, config: { bed: [100, 100] } })), detected: 10, seen_mm: [] as [number, number][] }) };
    const refine = vi.spyOn(api, 'cameraHeadAlignRefine');
    const r = await runHeadAlign({ api: { ...api, cameraHeadAlignPlan: api.cameraHeadAlignPlan, cameraHeadAlignRefine: api.cameraHeadAlignRefine, cameraHeadAlignSolve: full.cameraHeadAlignSolve as never }, io: fakeIo(), settings: DEFAULT_HEAD_ALIGN, bed: BED, lens: LENS });
    expect(r.rounds).toBe(0);
    expect(refine).not.toHaveBeenCalled();
  });

  it('afina con una rejilla nueva donde se vio el cabezal y no repite posiciones', async () => {
    const log: string[] = [];
    const refine = vi.spyOn(api, 'cameraHeadAlignRefine');
    const r = await runHeadAlign({ api, io: fakeIo(log), settings: { ...DEFAULT_HEAD_ALIGN, marginMm: 60 }, bed: BED, lens: null });
    expect(refine).toHaveBeenCalled();
    const visited = log.filter((l) => l.startsWith('move'));
    expect(new Set(visited).size).toBe(visited.length);
    expect(r.frames.length).toBe(visited.length);
  });

  it('detenerse corta entre dos movimientos y no deja nada guardado', async () => {
    const log: string[] = [];
    const io = fakeIo(log, (n) => {
      if (n === 5) useHeadAlign.getState().requestCancel(true);
    });
    await expect(runHeadAlign({ api, io, settings: DEFAULT_HEAD_ALIGN, bed: BED, lens: null })).rejects.toBeInstanceOf(AlignCancelled);
    // Tras la orden, ni un movimiento más.
    expect(log.filter((l) => l.startsWith('move'))).toHaveLength(3);
    expect(log.at(-1)).toBe('move 380,20');
  });
});

describe('startHeadAlign', () => {
  let api: MockApi;
  beforeEach(() => {
    api = createMockApi({ latencyMs: 0 });
    setApi(api);
    useStore.setState({ cameraStatus: 'live', cameraFrozen: false, connection: { ...useStore.getState().connection, connected: true } });
  });
  afterEach(() => useStore.setState({ cameraStatus: 'off' }));

  it('no empieza sin cámara viva, descongelada y con la máquina libre', () => {
    useStore.setState({ cameraStatus: 'off' });
    expect(headAlignBlockedReason()).toBe('camera.reason.notLive');
    useStore.setState({ cameraStatus: 'live', cameraFrozen: true });
    expect(headAlignBlockedReason()).toBe('camera.headAlign.reason.frozen');
    useStore.setState({ cameraFrozen: false, connection: { ...useStore.getState().connection, connected: false } });
    expect(headAlignBlockedReason()).toBe('reason.needMachine');
  });

  it('un motivo de bloqueo deja el aviso y no mueve nada', async () => {
    useStore.setState({ cameraStatus: 'off' });
    const io = fakeIo();
    const move = vi.spyOn(io, 'moveTo');
    expect(await startHeadAlign(io, api)).toBe(false);
    expect(move).not.toHaveBeenCalled();
    expect(useHeadAlign.getState().error).toBeTruthy();
  });

  it('un fallo a mitad del recorrido vuelve a la preparación con el motivo', async () => {
    const io = fakeIo();
    io.capture = async () => {
      throw new Error('cámara desconectada');
    };
    useStore.setState({ status: { ...(useStore.getState().status ?? ({} as never)), state: 'idle' } as never });
    expect(await startHeadAlign(io, api)).toBe(false);
    const ui = useHeadAlign.getState();
    expect(ui.phase).toBe('config');
    expect(ui.error).toContain('cámara desconectada');
  });

  it('un recorrido completo deja el resultado', async () => {
    expect(await startHeadAlign(fakeIo(), api)).toBe(true);
    const ui = useHeadAlign.getState();
    expect(ui.phase).toBe('result');
    expect(ui.report?.fit).not.toBeNull();
    expect(ui.frames.length).toBe(ui.report?.frames.length);
    expect(ui.area).not.toBeNull();
  });
});

describe('guardar', () => {
  let api: MockApi;
  let report: HeadAlignReport;
  beforeEach(async () => {
    api = createMockApi({ latencyMs: 0 });
    setApi(api);
    setSetupBackend(createApiBackend(() => api));
    // Lo que quedó guardando de la prueba anterior termina antes de empezar.
    await persistCameraSetup();
    useStore.getState().setCameraSetup(emptyCameraSetup(), true);
    // El mock lee el tamaño de un PNG de verdad; con fotos falsas usa 640 x 480.
    space = [640, 480];
    const r = await runHeadAlign({ api, io: fakeIo(), settings: { ...DEFAULT_HEAD_ALIGN, marginMm: 60 }, bed: BED, lens: null });
    report = r.report;
  });

  it('guarda la alineación como la de la cámara y avisa', () => {
    expect(saveBlockedReason(report)).toBeNull();
    expect(saveHeadAlignment(report)).toBe(true);
    const s = useStore.getState();
    expect(s.cameraSetup.alignment?.method).toBe('head_sweep');
    expect(s.cameraSetup.calibration).toBeNull();
    expect(useHeadAlign.getState().saved).toBe(true);
  });

  it('no guarda con otro tamaño de imagen', () => {
    space = [1280, 720];
    expect(saveBlockedReason(report)).toBe('size');
    expect(saveHeadAlignment(report)).toBe(false);
    expect(useStore.getState().cameraSetup.alignment).toBeNull();
  });

  it('con una lente estimada guarda las dos juntas y borra la calibración de la cámara del cabezal', () => {
    useStore.getState().setCameraSetup({ ...emptyCameraSetup(), head: { enabled: false, calibration: { image_size: [800, 600], ref_px: [400, 300], px_to_mm: [[1, 0], [0, 1]], offset_mm: [0, 0], rms_mm: 0, observations: 3 } } }, true);
    space = [1280, 720];
    expect(saveHeadAlignment({ ...report, lens: LENS })).toBe(true);
    const s = useStore.getState();
    expect(s.cameraSetup.calibration).toEqual(LENS);
    expect(s.cameraSetup.alignment?.method).toBe('head_sweep');
    expect(s.cameraSetup.head?.calibration).toBeNull();
  });

  it('sin ajuste no hay nada que guardar', () => {
    const none = { ...report, alignment: null, fit: null };
    expect(saveBlockedReason(none)).toBe('noFit');
    expect(saveHeadAlignment(none)).toBe(false);
  });
});

describe('liveIo con la máquina virtual', () => {
  let api: MockApi;
  let unbind: (() => void) | null = null;
  beforeEach(async () => {
    headAlignTiming.settleMs = 0;
    api = createMockApi({ latencyMs: 0, statusIntervalMs: 15, minJobMs: 100, jobSpeedup: 1e6 });
    setApi(api);
    const s = useStore.getState();
    await s.loadDevices();
    unbind = await bindMachineEvents(api);
    s.setSelectedPort('virtual:falcon2');
    await s.connect();
    await vi.waitFor(() => expect(useStore.getState().status?.state).toBe('idle'));
  });
  afterEach(async () => {
    headAlignTiming.settleMs = 900;
    unbind?.();
    await useStore.getState().disconnect();
  });

  it('mueve el cabezal a la posición pedida y espera a que se asiente', async () => {
    const io = liveIo();
    await io.moveTo([150, 120]);
    const s = useStore.getState();
    expect(s.status?.state).toBe('idle');
    expect(s.status?.work_pos[0]).toBeCloseTo(150, 0);
    expect(s.status?.work_pos[1]).toBeCloseTo(120, 0);
    expect(await io.capture()).toBe('AAAA');
  });

  it('no mueve fuera de la cama', async () => {
    await expect(liveIo().moveTo([5000, 10])).rejects.toThrow();
    expect(useStore.getState().status?.work_pos[0]).not.toBeCloseTo(5000, 0);
  });
});
