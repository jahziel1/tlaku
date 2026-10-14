import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import type { HeadCalibration } from '@/api/types';
import { selectActiveDevice, useStore } from '@/store';
import { emptyCameraSetup } from '@/store/camera';
import { planScan } from './head';
import { currentScanArea, scanBed, scanRegion, type ScanDeps } from './headScan';
import { useHeadUi } from './headState';
import { clipZone, MIN_ZONE_MM, rectFromDrag, zoneSize } from './zone';

vi.mock('./session', () => ({
  grabCorrectedPng: async () => null,
  grabCorrectedColorPng: async () => null,
  correctedSpace: () => [640, 360] as [number, number],
  setMosaicBackdrop: () => undefined,
  refreshSnapshot: async () => false,
}));

/** Cámara del cabezal: 640x360 a 2 px/mm (una foto cubre 320x180 mm). */
const CAL: HeadCalibration = { image_size: [640, 360], ref_px: [320, 180], px_to_mm: [[0.5, 0], [0, -0.5]], offset_mm: [12, -28], rms_mm: 0, observations: 0 };

let api: MockApi;

beforeEach(async () => {
  api = createMockApi({ latencyMs: 0 });
  setApi(api);
  await useStore.getState().loadDevices();
  useStore.getState().setCameraSetup({ ...emptyCameraSetup(), head: { enabled: true, calibration: CAL } }, true);
  useHeadUi.setState({ confirm: null, progress: null, error: null, cancelRequested: false, mosaic: null, useSelection: false, zone: null, useZone: false, drawingZone: false });
});

describe('rectángulo de la zona', () => {
  it('se normaliza entre dos esquinas cualesquiera y se recorta a la cama', () => {
    expect(rectFromDrag([50, 40], [10, 20], [300, 200])).toEqual([10, 20, 50, 40]);
    expect(rectFromDrag([-30, -5], [400, 90], [300, 200])).toEqual([0, 0, 300, 90]);
  });

  it('un clic o un trazo muy fino no es una zona', () => {
    expect(rectFromDrag([10, 10], [10, 10], [300, 200])).toBeNull();
    expect(rectFromDrag([10, 10], [10 + MIN_ZONE_MM - 0.1, 90], [300, 200])).toBeNull();
    expect(rectFromDrag([10, 10], [10 + MIN_ZONE_MM, 10 + MIN_ZONE_MM], [300, 200])).not.toBeNull();
    // Del todo fuera de la cama no queda nada.
    expect(rectFromDrag([400, 300], [500, 400], [300, 200])).toBeNull();
    expect(rectFromDrag([Number.NaN, 0], [10, 10], [300, 200])).toBeNull();
  });

  it('una zona guardada se recorta si la cama nueva es más chica', () => {
    expect(clipZone([10, 10, 250, 150], [200, 100])).toEqual([10, 10, 200, 100]);
    expect(clipZone([210, 10, 250, 150], [200, 100])).toBeNull();
    expect(zoneSize([10, 20, 50, 80])).toEqual([40, 60]);
  });
});

describe('qué parte de la cama se recorre', () => {
  it('la zona dibujada tiene prioridad; sin ella, la selección o toda la cama', () => {
    expect(scanRegion(false)).toBeUndefined();
    expect(scanRegion(false, [10, 20, 90, 70])).toEqual([10, 20, 90, 70]);
    expect(scanRegion(true, [10, 20, 90, 70])).toEqual([10, 20, 90, 70]);
  });

  it('dibujar una zona la elige; borrarla vuelve a la cama; zona y selección se excluyen', () => {
    const ui = useHeadUi.getState();
    ui.setUseSelection(true);
    ui.setZone([10, 20, 90, 70]);
    expect(useHeadUi.getState()).toMatchObject({ useZone: true, useSelection: false, zone: [10, 20, 90, 70] });
    expect(currentScanArea()).toEqual({ useSelection: false, zone: [10, 20, 90, 70] });
    ui.setUseSelection(true);
    expect(useHeadUi.getState()).toMatchObject({ useZone: false, useSelection: true });
    expect(currentScanArea()).toEqual({ useSelection: true, zone: null });
    // La zona sigue guardada y se puede volver a elegir.
    ui.setUseZone(true);
    expect(useHeadUi.getState()).toMatchObject({ useZone: true, useSelection: false });
    ui.setZone(null);
    expect(useHeadUi.getState()).toMatchObject({ useZone: false, zone: null });
    expect(currentScanArea()).toEqual({ useSelection: false, zone: null });
    // No se puede elegir una zona que no existe.
    ui.setUseZone(true);
    expect(useHeadUi.getState().useZone).toBe(false);
  });

  it('el recorrido solo cubre la zona: menos posiciones y la imagen pedida es esa zona', async () => {
    const bed = selectActiveDevice(useStore.getState()).bed;
    const zone: [number, number, number, number] = [60, 50, 160, 110];
    const full = planScan(bed, CAL);
    const part = planScan(bed, CAL, zone);
    if ('error' in full || 'error' in part) throw new Error('plan');
    expect(part.region).toEqual(zone);
    expect(part.stops.length).toBeLessThan(full.stops.length);

    const visited: [number, number][] = [];
    const deps: ScanDeps = {
      moveTo: async (l) => void visited.push(l),
      position: () => visited.at(-1) ?? null,
      capture: async () => ({ png: 'AAAA', width: 640, height: 360 }),
      settle: async () => undefined,
      cancelled: () => false,
      onProgress: () => undefined,
    };
    const compose = vi.spyOn(api, 'composeBedImage');
    await scanBed({ save: false, useSelection: false, zone }, deps);
    expect(visited).toHaveLength(part.stops.length);
    expect(compose).toHaveBeenCalledTimes(1);
    expect(compose.mock.calls[0]![0].rect).toEqual(zone);
    expect(compose.mock.calls[0]![0].tiles).toHaveLength(part.stops.length);

    // Sin zona, toda la cama.
    visited.length = 0;
    compose.mockClear();
    await scanBed({ save: false, useSelection: false }, deps);
    expect(visited).toHaveLength(full.stops.length);
    expect(compose.mock.calls[0]![0].rect).toEqual(full.region);
  });

  it('una zona fuera de la cama da el error de zona vacía', async () => {
    await expect(scanBed({ save: false, useSelection: false, zone: [5000, 5000, 5100, 5100] })).rejects.toThrow();
  });
});
