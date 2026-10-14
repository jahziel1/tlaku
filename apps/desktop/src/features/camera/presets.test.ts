import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import type { CameraPreset, CameraPrefs } from '@/api/types';
import { useStore } from '@/store';
import { emptyCameraSetup } from '@/store/camera';
import { applyPreset, checkPresetName, deletePreset, findPreset, matchesPreset, MAX_PRESETS, prefsFromPreset, presetFromPrefs, removePreset, savePreset, upsertPreset } from './presets';
import { createApiBackend, parseSetup, setSetupBackend } from './setup';

const startCamera = vi.fn(async () => undefined);
vi.mock('./session', () => ({ startCamera: () => startCamera() }));

const PREFS: CameraPrefs = {
  device_id: 'abc',
  device_label: 'USB Cam',
  width: 1280,
  height: 720,
  opacity: 0.4,
  visible: true,
  source: 'network',
  network_url: 'http://u:p@192.168.1.20/snap.jpg',
  dpm: 8.5,
  offset_x_mm: 1.5,
  offset_y_mm: -0.5,
  grayscale: true,
};

let api: MockApi;

beforeEach(() => {
  api = createMockApi({ latencyMs: 0 });
  setApi(api);
  setSetupBackend(createApiBackend(() => api));
  useStore.getState().setCameraSetup({ ...emptyCameraSetup(), prefs: { ...PREFS } }, true);
  useStore.setState({ notices: [], cameraStatus: 'off' });
  startCamera.mockClear();
});

describe('configuraciones de cámara con nombre', () => {
  it('guardan la fuente y el ajuste, no la opacidad, y se pueden volver a aplicar', () => {
    const p = presetFromPrefs('  Taller ', PREFS);
    expect(p).toEqual({
      name: 'Taller',
      source: 'network',
      device_id: 'abc',
      device_label: 'USB Cam',
      width: 1280,
      height: 720,
      network_url: 'http://u:p@192.168.1.20/snap.jpg',
      dpm: 8.5,
      offset_x_mm: 1.5,
      offset_y_mm: -0.5,
      grayscale: true,
    });
    expect(prefsFromPreset(p)).not.toHaveProperty('opacity');
    expect(prefsFromPreset(p)).not.toHaveProperty('visible');
    expect(matchesPreset(p, PREFS)).toBe(true);
    expect(matchesPreset(p, { ...PREFS, dpm: 9 })).toBe(false);
  });

  it('las preferencias de antes (sin los campos nuevos) dan una configuración local sin ajuste', () => {
    const p = presetFromPrefs('Vieja', { device_id: null, device_label: null, width: null, height: null, opacity: 0.6, visible: false });
    expect(p).toMatchObject({ source: 'local', network_url: null, dpm: null, offset_x_mm: 0, offset_y_mm: 0, grayscale: false });
  });

  it('el nombre se compara sin mayúsculas y reemplaza a la configuración igual', () => {
    const a = presetFromPrefs('Taller', PREFS);
    const b = presetFromPrefs('Garage', { ...PREFS, dpm: null });
    const list = upsertPreset(upsertPreset([], a), b);
    expect(list.map((p) => p.name)).toEqual(['Taller', 'Garage']);
    const again = upsertPreset(list, { ...a, name: 'taller', dpm: 3 });
    expect(again).toHaveLength(2);
    expect(again[0]).toMatchObject({ name: 'taller', dpm: 3 });
    expect(findPreset(list, 'GARAGE')?.name).toBe('Garage');
    expect(removePreset(list, ' taller ').map((p) => p.name)).toEqual(['Garage']);
  });

  it('exige un nombre de largo razonable', () => {
    expect(checkPresetName('   ')).toBe('empty');
    expect(checkPresetName('x'.repeat(61))).toBe('long');
    expect(checkPresetName('Taller')).toBeNull();
  });

  it('guardar y borrar pasan por el store y llegan al respaldo', async () => {
    expect(savePreset('')).toBe('empty');
    expect(savePreset('Taller')).toBeNull();
    expect(useStore.getState().cameraSetup.presets?.map((p) => p.name)).toEqual(['Taller']);
    expect(useStore.getState().notices.at(-1)?.level).toBe('success');
    expect(savePreset('taller')).toBeNull(); // reemplaza, no duplica
    expect(useStore.getState().cameraSetup.presets).toHaveLength(1);
    await vi.waitFor(async () => expect((await api.loadCameraSetup()).presets?.map((p) => p.name)).toEqual(['taller']));
    deletePreset('Taller');
    expect(useStore.getState().cameraSetup.presets).toEqual([]);
  });

  it('no deja pasar del máximo, pero sí actualizar una existente', () => {
    const many: CameraPreset[] = Array.from({ length: MAX_PRESETS }, (_, i) => presetFromPrefs(`p${i}`, PREFS));
    useStore.getState().setCameraSetup({ ...useStore.getState().cameraSetup, presets: many }, true);
    expect(savePreset('uno más')).toBe('full');
    expect(savePreset('P3')).toBeNull();
    expect(useStore.getState().cameraSetup.presets).toHaveLength(MAX_PRESETS);
  });

  it('aplicar fija las preferencias y reabre la cámara solo si estaba encendida', () => {
    useStore.getState().setCameraSetup({
      ...useStore.getState().cameraSetup,
      prefs: { ...PREFS, source: 'local', network_url: null, dpm: null, offset_x_mm: 0, offset_y_mm: 0, grayscale: false, opacity: 0.9 },
      presets: [presetFromPrefs('Red', PREFS)],
    }, true);
    expect(applyPreset('nada')).toBe(false);
    expect(applyPreset('red')).toBe(true);
    const p = useStore.getState().cameraSetup.prefs;
    expect(p).toMatchObject({ source: 'network', network_url: PREFS.network_url, dpm: 8.5, offset_x_mm: 1.5, grayscale: true, opacity: 0.9 });
    expect(startCamera).not.toHaveBeenCalled();
    useStore.setState({ cameraStatus: 'live' });
    applyPreset('Red');
    expect(startCamera).toHaveBeenCalledTimes(1);
  });

  it('se leen del archivo guardado y se descartan las que no sirven', () => {
    const s = parseSetup({
      presets: [
        { name: 'Bien', source: 'network', network_url: 'http://192.168.1.2/x', dpm: 4, offset_x_mm: 2 },
        { name: '   ' },
        { source: 'local' },
        { name: 'bien', dpm: -1 },
        null,
        { name: 'Otra', width: 0, dpm: 'x' },
      ],
      prefs: { source: 'network', network_url: 'http://192.168.1.2/x', dpm: 6, offset_x_mm: 1, offset_y_mm: 'z' },
    });
    expect(s.presets?.map((p) => p.name)).toEqual(['bien', 'Otra']);
    expect(s.presets?.[0]).toMatchObject({ source: 'local', dpm: null });
    expect(s.presets?.[1]).toMatchObject({ width: null, dpm: null, source: 'local' });
    expect(s.prefs).toMatchObject({ source: 'network', network_url: 'http://192.168.1.2/x', dpm: 6, offset_x_mm: 1 });
    expect(s.prefs.offset_y_mm).toBeUndefined();
    expect(parseSetup({}).presets).toBeUndefined();
  });
});
