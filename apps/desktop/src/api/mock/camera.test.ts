import { describe, expect, it } from 'vitest';
import { createMockApi } from './index';
import { STORAGE_KEY, createSetupStore, pngSize } from './camera';

/** PNG 1x1 mínimo con la cabecera IHDR cambiada al tamaño pedido (solo se lee la cabecera). */
function fakePng(w: number, h: number): string {
  const bytes = new Uint8Array(33);
  bytes.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52]);
  new DataView(bytes.buffer).setUint32(16, w);
  new DataView(bytes.buffer).setUint32(20, h);
  return btoa(String.fromCharCode(...bytes));
}

describe('mock de cámara', () => {
  const api = createMockApi({ latencyMs: 0 });

  it('lee el tamaño de un PNG', () => {
    expect(pngSize(fakePng(1280, 720))).toEqual([1280, 720]);
    expect(pngSize(`data:image/png;base64,${fakePng(640, 480)}`)).toEqual([640, 480]);
    expect(pngSize('AAAA')).toBeNull();
  });

  it('detecta un tablero de 9x6 dentro de la imagen', async () => {
    const pts = (await api.detectBoard({ image_png_base64: fakePng(1280, 720), board: [9, 6] }))!;
    expect(pts).toHaveLength(54);
    for (const [x, y] of pts) {
      expect(x).toBeGreaterThan(0);
      expect(x).toBeLessThan(1280);
      expect(y).toBeGreaterThan(0);
      expect(y).toBeLessThan(720);
    }
  });

  it('calibra con la forma de CalibrationReport', async () => {
    const imgs = Array.from({ length: 6 }, () => fakePng(1280, 720));
    const r = await api.calibrateLens({ images_png_base64: imgs, board: [9, 6], square_mm: 25 });
    expect(r.calibration.width).toBe(1280);
    expect(r.used_views).toHaveLength(6);
    expect(r.view_rms_px).toHaveLength(6);
    await expect(api.calibrateLens({ images_png_base64: imgs.slice(0, 2), board: [9, 6], square_mm: 25 })).rejects.toThrow();
  });
});

describe('mock de la configuración de cámara', () => {
  const memoryStorage = () => {
    const data = new Map<string, string>();
    return { data, getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => void data.set(k, v) };
  };

  it('guarda y recupera por localStorage, y un JSON dañado o sin almacenamiento no rompe la carga', () => {
    const storage = memoryStorage();
    const store = createSetupStore(storage);
    const blank = store.load();
    expect(blank.calibration).toBeNull();
    store.save({ ...blank, prefs: { ...blank.prefs, opacity: 0.2 } });
    expect(storage.data.has(STORAGE_KEY)).toBe(true);
    expect(createSetupStore(storage).load().prefs.opacity).toBe(0.2);
    storage.setItem(STORAGE_KEY, '{no es json');
    expect(createSetupStore(storage).load().calibration).toBeNull();
    const none = createSetupStore(null);
    none.save({ ...blank, prefs: { ...blank.prefs, opacity: 0.9 } });
    expect(none.load().prefs.opacity).toBe(0.9); // queda en memoria
  });

  it('saveBoardFile solo acepta .svg y .pdf', async () => {
    const api = createMockApi({ latencyMs: 0 });
    await expect(api.saveBoardFile({ path: '/x/tablero.pdf', content_base64: '' })).resolves.toBeUndefined();
    await expect(api.saveBoardFile({ path: '/x/tablero.png', content_base64: '' })).rejects.toThrow(/svg o .pdf/);
    api.dispose();
  });
});
