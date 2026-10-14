import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { mockMarkPositions } from '@/api/mock/camera';
import type { Document, HeadCalibration, MarkResult, MarksReport } from '@/api/types';
import { bindMachineEvents, useStore } from '@/store';
import { emptyCameraSetup } from '@/store/camera';
import { selectHeadDoc } from '@/store/machine';
import type { Vec2 } from './geometry';
import { fitHeadCalibration, type HeadObservation } from './head';
import { currentMarksFit, detectHeadMarks, HeadMarksError, marksOrigin, observationsFromReport, openHeadMarksAsDocument, saveHeadMarksCalibration, shiftDocument } from './headMarks';
import { HEAD_MARKS_MIN_USED, useHeadMarks } from './headMarksState';
import { createApiBackend, setSetupBackend } from './setup';

const SPACE: [number, number] = [1280, 720];
let frame: { png: string; width: number; height: number } | null = { png: 'AAAA', width: 640, height: 360 };

vi.mock('./session', () => ({
  grabCorrectedPng: async () => frame,
  grabCorrectedColorPng: async () => frame,
  correctedSpace: () => SPACE,
  setMosaicBackdrop: () => undefined,
  refreshSnapshot: async () => false,
}));

/** Cámara del cabezal verdadera: 1280x720, 8 px/mm, girada 1,7° y con el eje Y de la imagen invertido. */
function truth(): HeadCalibration {
  const s = 1 / 8;
  const th = (1.7 * Math.PI) / 180;
  const c = Math.cos(th) * s;
  const sn = Math.sin(th) * s;
  return { image_size: SPACE, ref_px: [640, 360], px_to_mm: [[c, sn], [sn, -c]], offset_mm: [23.4, -41.7], rms_mm: 0, observations: 0 };
}

function pixelOf(cal: HeadCalibration, laser: Vec2, mm: Vec2): Vec2 {
  const [[a, b], [c, d]] = cal.px_to_mm;
  const rx = mm[0] - laser[0] - cal.offset_mm[0];
  const ry = mm[1] - laser[1] - cal.offset_mm[1];
  const det = a * d - b * c;
  return [cal.ref_px[0] + (d * rx - b * ry) / det, cal.ref_px[1] + (-c * rx + a * ry) / det];
}

/** Informe del motor para un cuadro de 80 mm con la esquina en `origin`, visto con el láser en `laser` (píxeles de la foto, la mitad del espacio). */
function report(laser: Vec2, origin: Vec2, opts: { usedFrom?: number } = {}): MarksReport {
  const cal = truth();
  const marks: MarkResult[] = mockMarkPositions([80, 80], 9).map((bed, i) => {
    const p = pixelOf(cal, laser, [bed[0] + origin[0], bed[1] + origin[1]]);
    return { index: i, bed, image: [p[0] / 2, p[1] / 2], confidence: 0.95, used: i >= (opts.usedFrom ?? 0), residual_mm: 0.05, loo_mm: 0.1 };
  });
  return {
    marks,
    candidates: [],
    to_bed: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
    expected: 9,
    found: 9,
    used: marks.filter((m) => m.used).length,
    rms_mm: 0.05,
    max_residual_mm: 0.1,
    loo_rms_mm: 0.1,
    loo_max_mm: 0.2,
    scale_px_per_mm: 4,
    used_prior: false,
    warnings: [],
  };
}

let api: MockApi;
let unbind: (() => void) | null = null;

beforeEach(async () => {
  api = createMockApi({ latencyMs: 0, statusIntervalMs: 15, minJobMs: 100, jobSpeedup: 1e6 });
  setApi(api);
  setSetupBackend(createApiBackend(() => api));
  frame = { png: 'AAAA', width: 640, height: 360 };
  const s = useStore.getState();
  await s.loadDevices();
  unbind = await bindMachineEvents(api);
  s.setSelectedPort('virtual:falcon2');
  await s.connect();
  await vi.waitFor(() => expect(useStore.getState().status?.state).toBe('idle'));
  s.setCameraSetup({ ...emptyCameraSetup(), head: { enabled: true, calibration: null } }, true);
  useHeadMarks.getState().reset();
  useHeadMarks.getState().setSize(80);
  useHeadMarks.getState().setOrigin([100, 80]);
});

afterEach(async () => {
  unbind?.();
  unbind = null;
  await useStore.getState().disconnect();
});

describe('ajuste con varias marcas por foto', () => {
  it('una sola foto con 9 marcas basta para recuperar el desfase y la escala', () => {
    const cal = truth();
    const laser: Vec2 = [60, 70];
    const obs: HeadObservation[] = mockMarkPositions([80, 80], 9).map((m) => {
      const mark: Vec2 = [m[0] + 100, m[1] + 80];
      return { laser, px: pixelOf(cal, laser, mark), mark };
    });
    const fit = fitHeadCalibration(obs, null, SPACE);
    if (!('calibration' in fit)) throw new Error(fit.error);
    const got = fit.calibration;
    expect(got.offset_mm[0]).toBeCloseTo(cal.offset_mm[0], 6);
    expect(got.offset_mm[1]).toBeCloseTo(cal.offset_mm[1], 6);
    got.px_to_mm.flat().forEach((v, i) => expect(v).toBeCloseTo(cal.px_to_mm.flat()[i]!, 8));
    expect(got.rms_mm).toBeLessThan(1e-6);
    expect(got.observations).toBe(9);
  });

  it('mezcla marcas propias con la marca única de siempre y exige una marca para cada observación', () => {
    const cal = truth();
    const single: Vec2 = [150, 100];
    const obs: HeadObservation[] = [[0, 0], [8, 0], [8, 8], [0, 8]].map(([dx, dy]) => {
      const laser: Vec2 = [140 + dx!, 90 + dy!];
      return { laser, px: pixelOf(cal, laser, single) };
    });
    expect('calibration' in fitHeadCalibration(obs, single, SPACE)).toBe(true);
    // Sin marca única ni propia: observación inválida.
    expect(fitHeadCalibration(obs, null, SPACE)).toEqual({ error: 'invalid' });
  });
});

describe('observaciones de una detección', () => {
  it('solo cuentan las marcas usadas y llevan su posición absoluta en la cama', () => {
    const r = report([60, 70], [100, 80], { usedFrom: 3 });
    const obs = observationsFromReport(r, [60, 70], [100, 80]);
    expect(obs).toHaveLength(6);
    expect(obs[0]!.mark).toEqual([r.marks[3]!.bed[0] + 100, r.marks[3]!.bed[1] + 80]);
    expect(obs.every((o) => o.laser[0] === 60 && o.laser[1] === 70)).toBe(true);
    // Una marca sin imagen no sirve aunque esté marcada como usada.
    r.marks[4] = { ...r.marks[4]!, image: null };
    expect(observationsFromReport(r, [0, 0], [0, 0])).toHaveLength(5);
  });

  it('el cuadro por omisión queda centrado en la cama y sin salirse de ella', () => {
    expect(marksOrigin(80, null, [300, 200])).toEqual([110, 60]);
    expect(marksOrigin(80, null, [50, 50])).toEqual([0, 0]);
    expect(marksOrigin(80, [10, 20], [300, 200])).toEqual([10, 20]);
  });
});

describe('detectar las marcas con la cámara del cabezal', () => {
  it('detecta, pasa los píxeles al espacio de la imagen corregida y calibra', async () => {
    const laser = selectHeadDoc(useStore.getState())!;
    expect(laser).not.toBeNull();
    api.detectCameraMarks = vi.fn(async () => report(laser, [100, 80]));
    const r = await detectHeadMarks();
    expect(r.found).toBe(9);
    // Se pidió el cuadro de marcas con su propio tamaño, no el de la cama.
    expect(api.detectCameraMarks).toHaveBeenCalledWith(expect.objectContaining({ bed: [80, 80], count: 9, prior_to_bed: null }));
    expect(useHeadMarks.getState().obs).toHaveLength(9);
    const fit = currentMarksFit();
    if (!('calibration' in fit)) throw new Error(JSON.stringify(fit));
    expect(fit.calibration.offset_mm[0]).toBeCloseTo(truth().offset_mm[0], 5);
    expect(fit.calibration.offset_mm[1]).toBeCloseTo(truth().offset_mm[1], 5);
    expect(fit.calibration.rms_mm).toBeLessThan(1e-5);

    saveHeadMarksCalibration(fit.calibration);
    const head = useStore.getState().cameraSetup.head!;
    expect(head.enabled).toBe(true);
    expect(head.calibration!.offset_mm[0]).toBeCloseTo(truth().offset_mm[0], 5);
    expect(useHeadMarks.getState().obs).toHaveLength(0);
  });

  it('con pocas marcas explica qué pasó y no acumula nada', async () => {
    const laser = selectHeadDoc(useStore.getState())!;
    api.detectCameraMarks = vi.fn(async () => report(laser, [100, 80], { usedFrom: 9 - (HEAD_MARKS_MIN_USED - 1) }));
    await expect(detectHeadMarks()).rejects.toBeInstanceOf(HeadMarksError);
    await expect(detectHeadMarks()).rejects.toThrow(/solo 3 sirvieron/);
    expect(useHeadMarks.getState().obs).toHaveLength(0);
  });

  it('sin posición del láser o sin imagen no hay detección', async () => {
    api.detectCameraMarks = vi.fn();
    frame = null;
    await expect(detectHeadMarks()).rejects.toMatchObject({ kind: 'noImage' });
    await useStore.getState().disconnect();
    await expect(detectHeadMarks()).rejects.toMatchObject({ kind: 'noPosition' });
    expect(api.detectCameraMarks).not.toHaveBeenCalled();
  });

  it('varias posiciones se suman', async () => {
    const laser = selectHeadDoc(useStore.getState())!;
    api.detectCameraMarks = vi.fn(async () => report(laser, [100, 80]));
    await detectHeadMarks();
    await detectHeadMarks();
    expect(useHeadMarks.getState().detections).toHaveLength(2);
    expect(useHeadMarks.getState().obs).toHaveLength(18);
  });
});

describe('documento con el cuadro de marcas', () => {
  const doc = (): Document => ({
    version: 2,
    layers: [],
    device_id: null,
    objects: [
      { id: 'a', layer: '#0066ff', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [] }, locked: false, hidden: false, group: null },
      { id: 'b', layer: '#0066ff', transform: [1, 0, 0, 1, 5, 7], kind: { type: 'vector', paths: [] }, locked: false, hidden: false, group: null },
    ],
  });

  it('mueve todas las marcas a su esquina sin tocar el resto', () => {
    const moved = shiftDocument(doc(), 100, 80);
    expect(moved.objects.map((o) => o.transform)).toEqual([[1, 0, 0, 1, 100, 80], [1, 0, 0, 1, 105, 87]]);
    expect(doc().objects[0]!.transform).toEqual([1, 0, 0, 1, 0, 0]);
  });

  it('se abre como documento nuevo con las 9 marcas en su lugar y rechaza un cuadro que no cabe', async () => {
    useStore.getState().replaceDocument({ version: 2, layers: [], objects: [], device_id: null } as never, null);
    expect(await openHeadMarksAsDocument()).toBe(true);
    const objs = useStore.getState().doc.objects;
    expect(objs).toHaveLength(9);
    expect(objs[0]!.transform.slice(4)).toEqual([100, 80]);
    // Una esquina que deja el cuadro fuera de la cama no abre nada.
    useHeadMarks.getState().setOrigin([10_000, 80]);
    useStore.getState().replaceDocument({ version: 2, layers: [], objects: [], device_id: null } as never, null);
    expect(await openHeadMarksAsDocument()).toBe(false);
    expect(useStore.getState().doc.objects).toHaveLength(0);
    expect(useStore.getState().notices.at(-1)?.level).toBe('error');
  });
});
