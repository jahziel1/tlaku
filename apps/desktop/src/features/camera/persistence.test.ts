import { beforeEach, describe, expect, it } from 'vitest';
import type { LensCalibration } from '@/api/types';
import { createMockApi } from '@/api/mock';
import { commands } from '@/commands';
import { useStore } from '@/store';
import { emptyCameraSetup, type CameraSetup } from '@/store/camera';
import { applyAlignment, applyLens, loadCameraSetup, persistCameraSetup, updateCameraPrefs } from './actions';
import type { Quad } from './geometry';
import { createApiBackend, fitAlignment, parseSetup, setSetupBackend, type SetupBackend } from './setup';

const LENS: LensCalibration = {
  width: 1280, height: 720, fx: 950, fy: 960, cx: 645, cy: 355,
  k1: -0.32, k2: 0.11, p1: 0.0012, p2: -0.0008, k3: 0, rms_error_px: 0.34, views_used: 8,
};
const PX: Quad = [[100, 600], [1180, 610], [1170, 90], [110, 100]];
const MM: Quad = [[0, 0], [297, 0], [297, 210], [0, 210]];

/** Respaldo que guarda como el motor de Tauri: JSON en "disco" (aquí, una variable). */
function diskBackend(): SetupBackend & { disk: string | null } {
  const b = {
    disk: null as string | null,
    async load() {
      return b.disk ? parseSetup(JSON.parse(b.disk)) : emptyCameraSetup();
    },
    async save(setup: CameraSetup) {
      b.disk = JSON.stringify(setup);
      return parseSetup(setup);
    },
  };
  return b;
}

beforeEach(() => {
  useStore.getState().setCameraSetup(emptyCameraSetup(), false);
});

describe('parseSetup', () => {
  it('descarta lo que no tiene la forma esperada', () => {
    expect(parseSetup(null)).toEqual(emptyCameraSetup());
    expect(parseSetup({ calibration: { width: 'x' }, alignment: { image_px: 3 } }).calibration).toBeNull();
    const dirty = parseSetup({ prefs: { opacity: 9, visible: 'yes', width: 'a' } });
    expect(dirty.prefs.opacity).toBe(1);
    expect(dirty.prefs.visible).toBe(false);
    expect(dirty.prefs.width).toBeNull();
  });

  it('recalcula la homografía de la alineación guardada', () => {
    const fit = fitAlignment([1280, 720], PX, MM)!;
    const raw = { alignment: { image_size: [1280, 720], image_px: PX, bed_mm: MM, to_bed: [[0, 0, 0], [0, 0, 0], [0, 0, 0]], rms_mm: 9 } };
    expect(parseSetup(raw).alignment?.to_bed).toEqual(fit.to_bed);
  });
});

describe('respaldo por LaserApi', () => {
  it('lee y guarda por loadCameraSetup / saveCameraSetup y valida lo recibido', async () => {
    const calls: string[] = [];
    const backend = createApiBackend(() => ({
      loadCameraSetup: async () => {
        calls.push('load');
        return { calibration: { width: 'x' }, prefs: { opacity: 9 } } as unknown as CameraSetup;
      },
      saveCameraSetup: async ({ setup }) => {
        calls.push('save');
        return setup;
      },
    }));
    const loaded = await backend.load();
    expect(loaded.calibration).toBeNull();
    expect(loaded.prefs.opacity).toBe(1);
    await backend.save(emptyCameraSetup());
    expect(calls).toEqual(['load', 'save']);
  });

  it('con el mock real, lo guardado vuelve igual tras "reiniciar"', async () => {
    const api = createMockApi({ latencyMs: 0 });
    const setup: CameraSetup = {
      ...emptyCameraSetup(),
      calibration: LENS,
      alignment: fitAlignment([1280, 720], PX, MM),
      prefs: { device_id: 'cam-1', device_label: 'USB 2.0', width: 1280, height: 720, opacity: 0.35, visible: true },
    };
    expect(await createApiBackend(() => api).load()).toEqual(emptyCameraSetup());
    await createApiBackend(() => api).save(setup);
    const loaded = await createApiBackend(() => api).load();
    // Con la lente calibrada, el motor (y el mock) agregan el modelo de altura estimado.
    expect(loaded.alignment?.height?.source).toBe('modelled');
    expect(loaded.alignment?.samples).toEqual([]);
    expect({ ...loaded, alignment: { ...loaded.alignment!, height: undefined, method: undefined, samples: undefined, reference_height_mm: undefined } }).toEqual({
      ...setup,
      alignment: { ...setup.alignment!, height: undefined, method: undefined, samples: undefined, reference_height_mm: undefined },
    });
    // Sin lente no hay modelo: la alineación vuelve exactamente igual.
    const bare: CameraSetup = { ...setup, calibration: null };
    await createApiBackend(() => api).save(bare);
    expect(await createApiBackend(() => api).load()).toEqual(bare);
    api.dispose();
  });
});

describe('acciones: la alineación sobrevive al reinicio', () => {
  it('guardar y volver a cargar restaura todo', async () => {
    const disk = diskBackend();
    setSetupBackend(disk);
    applyLens(LENS);
    applyAlignment(fitAlignment([1280, 720], PX, MM));
    updateCameraPrefs({ opacity: 0.4, visible: true, device_label: 'USB 2.0' });
    await persistCameraSetup();
    const before = useStore.getState().cameraSetup;
    expect(JSON.parse(disk.disk!).alignment.image_px).toEqual(PX);

    // Reinicio: el estado se vacía y se carga de nuevo desde el respaldo.
    useStore.getState().setCameraSetup(emptyCameraSetup(), false);
    expect(useStore.getState().cameraSetup.alignment).toBeNull();
    await loadCameraSetup();
    const after = useStore.getState().cameraSetup;
    expect(after).toEqual(before);
    expect(after.alignment?.bed_mm).toEqual(MM);
    expect(after.prefs.opacity).toBe(0.4);
    setSetupBackend(null);
  });

  it('una calibración nueva invalida la alineación y lo avisa', async () => {
    setSetupBackend(diskBackend());
    applyAlignment(fitAlignment([1280, 720], PX, MM));
    const notices = useStore.getState().notices?.length ?? 0;
    applyLens({ ...LENS, fx: 1000 });
    expect(useStore.getState().cameraSetup.alignment).toBeNull();
    expect(useStore.getState().cameraSetup.calibration?.fx).toBe(1000);
    expect((useStore.getState().notices?.length ?? 0)).toBeGreaterThan(notices);
    await persistCameraSetup();
    setSetupBackend(null);
  });

  it('si el respaldo falla al cargar, sigue con una configuración vacía y avisa', async () => {
    setSetupBackend({
      load: () => Promise.reject(new Error('disco ilegible')),
      save: () => Promise.reject(new Error('sin permisos')),
    });
    await loadCameraSetup();
    expect(useStore.getState().cameraSetupLoaded).toBe(true);
    expect(useStore.getState().cameraSetup).toEqual(emptyCameraSetup());
    updateCameraPrefs({ opacity: 0.2 });
    await persistCameraSetup();
    setSetupBackend(null);
  });
});

describe('comandos de cámara', () => {
  it('el fondo exige una alineación y el congelado exige la cámara encendida', () => {
    const s = useStore.getState();
    expect(commands.availability('camera.backdrop', s)).toBe('camera.reason.noAlignment');
    expect(commands.availability('camera.freeze', s)).toBe('camera.reason.notLive');
    useStore.getState().setCameraSetup({ ...emptyCameraSetup(), alignment: fitAlignment([1280, 720], PX, MM) });
    useStore.getState().setCameraState({ cameraStatus: 'live' });
    const t = useStore.getState();
    expect(commands.availability('camera.backdrop', t)).toBe(true);
    expect(commands.availability('camera.freeze', t)).toBe(true);
    expect(commands.label('camera.toggle', t)).toBe('camera.cmd.stop');
    useStore.getState().setCameraState({ cameraStatus: 'off' });
  });

  it('abrir la calibración marca el asistente como abierto', async () => {
    await commands.run('camera.calibrate');
    expect(useStore.getState().cameraCalibrationOpen).toBe(true);
    useStore.getState().setCameraCalibrationOpen(false);
  });

  it('setCameraBackdrop sigue funcionando para el lienzo', () => {
    const canvas = {} as CanvasImageSource;
    const b = { source: canvas, width: 2, height: 2, corners: [[0, 0], [1, 0], [1, 1], [0, 1]] as Quad, opacity: 1, frame: 1 };
    useStore.getState().setCameraBackdrop(b);
    expect(useStore.getState().cameraBackdrop).toBe(b);
    useStore.getState().setCameraBackdrop(null);
  });
});
