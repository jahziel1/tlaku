import { describe, expect, it } from 'vitest';
import { createMockApi } from './index';
import { STORAGE_KEY, createSetupStore, mockMarkPositions, pngSize } from './camera';

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

describe('mock de la cámara (ola 4)', () => {
  const api = createMockApi({ latencyMs: 0 });
  const imgs = Array.from({ length: 5 }, () => fakePng(1280, 720));

  it('la calibración automática elige Brown-Conrady y el ojo de pez solo si se pide', async () => {
    const auto = await api.calibrateLens({ images_png_base64: imgs, board: [9, 6], square_mm: 25 });
    expect(auto.calibration.model).toBe('brown_conrady');
    expect(auto.model_scores).toHaveLength(2);
    expect(auto.model_scores!.filter((m) => m.chosen)).toHaveLength(1);
    const fish = await api.calibrateLens({ images_png_base64: imgs, board: [9, 6], square_mm: 25, model: 'fisheye' });
    expect(fish.calibration.model).toBe('fisheye');
    expect(fish.calibration.view_zoom).toBeLessThan(1);
    expect(fish.model_scores).toHaveLength(1);
  });

  it('compone la imagen de la cama dentro de la cama y con el tamaño de los píxeles', async () => {
    // 1280x720 px a 0,25 mm/px: 320x180 mm con la esquina superior izquierda en (40, 250).
    const tile = { image_png_base64: fakePng(1280, 720), to_bed: [[0.25, 0, 40], [0, -0.25, 250], [0, 0, 1]] as [[number, number, number], [number, number, number], [number, number, number]] };
    const args = { tiles: [tile], bed: [400, 300] as [number, number], px_per_mm: 2, grayscale: false, feather_px: null, crop_px: null, rect: null, dir: null, stem: 'p', save: true };
    const a = await api.composeBedImage(args);
    expect(a.width_px).toBe(640);
    expect(a.height_px).toBe(360);
    expect(a.origin_mm[0]).toBeCloseTo(40, 9);
    expect(a.origin_mm[1]).toBeCloseTo(70, 9);
    expect(a.size_mm).toEqual([320, 180]);
    expect(a.path).toMatch(/p-camara-\d+\.png$/);
    expect((await api.composeBedImage({ ...args, save: false })).path).toBe('');
    await expect(api.composeBedImage({ ...args, tiles: [] })).rejects.toThrow(/ninguna foto/);
    const outside = { ...tile, to_bed: [[0.25, 0, 900], [0, -0.25, 900], [0, 0, 1]] as typeof tile.to_bed };
    await expect(api.composeBedImage({ ...args, tiles: [outside] })).rejects.toThrow(/dentro de la cama/);
  });

  it('detecta un círculo en el indicio o en el centro de la imagen', async () => {
    const c = await api.detectCircle({ image_png_base64: fakePng(640, 480), hint: { center: [200, 100], radius: 50 } });
    expect(c.center).toEqual([200, 100]);
    expect(c.radius).toBe(50);
    expect(c.points).toHaveLength(48);
    const d = await api.detectCircle({ image_png_base64: fakePng(640, 480) });
    expect(d.center).toEqual([320, 240]);
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

describe('mock de marcas grabadas y altura', () => {
  const api = createMockApi({ latencyMs: 0 });
  const BED: [number, number] = [300, 200];

  it('genera el documento de marcas con una capa line de baja potencia', async () => {
    const doc = await api.cameraMarks({ bed: BED, count: 9 });
    expect(doc.objects).toHaveLength(9);
    expect(doc.layers).toHaveLength(1);
    expect(doc.layers[0]!.mode).toBe('line');
    expect(doc.layers[0]!.power).toBeLessThanOrEqual(15);
    // La marca 1 lleva círculo (3 trazos); las demás, solo la cruz (2).
    const strokes = doc.objects.map((o) => (o.kind.type === 'vector' ? o.kind.paths.length : 0));
    expect(strokes).toEqual([3, 2, 2, 2, 2, 2, 2, 2, 2]);
    expect((await api.cameraMarks({ bed: BED, count: 4 })).objects).toHaveLength(4);
    await expect(api.cameraMarks({ bed: [20, 20], count: 4 })).rejects.toThrow();
  });

  it('las posiciones de las marcas cubren el 85 % de la cama, como el motor', () => {
    const pos = mockMarkPositions(BED, 9);
    expect(pos[0]).toEqual([22.5, 15]);
    expect(pos[2]).toEqual([277.5, 185]);
    expect(pos[8]).toEqual([150, 100]);
  });

  it('detecta las marcas dentro de la imagen con errores plausibles', async () => {
    const r = await api.detectCameraMarks({ image_png_base64: fakePng(1280, 720), bed: BED, count: 9, prior_to_bed: null });
    expect(r.marks).toHaveLength(9);
    expect(r.used).toBe(9);
    for (const m of r.marks) {
      expect(m.image![0]).toBeGreaterThan(0);
      expect(m.image![0]).toBeLessThan(1280);
      expect(m.image![1]).toBeGreaterThan(0);
      expect(m.image![1]).toBeLessThan(720);
    }
    expect(r.loo_rms_mm).not.toBeNull();
    expect(r.loo_max_mm!).toBeLessThan(0.6);
    const four = await api.detectCameraMarks({ image_png_base64: fakePng(1280, 720), bed: BED, count: 4, prior_to_bed: null });
    expect(four.loo_rms_mm).toBeNull();
    expect(four.marks).toHaveLength(4);
  });

  it('al guardar completa el modelo de altura: medido con dos alturas, estimado con lente, ninguno sin lente', async () => {
    const det = await api.detectCameraMarks({ image_png_base64: fakePng(1280, 720), bed: BED, count: 9, prior_to_bed: null });
    const sample = (h: number) => ({ height_mm: h, image_px: det.marks.map((m) => m.image!), bed_mm: det.marks.map((m) => m.bed) });
    const blank = await api.loadCameraSetup();
    const lens = (await api.calibrateLens({ images_png_base64: Array(3).fill(fakePng(1280, 720)), board: [9, 6], square_mm: 25 })).calibration;
    const align = (samples: ReturnType<typeof sample>[]) => ({
      image_size: [1280, 720] as [number, number],
      image_px: [det.marks[0]!.image!, det.marks[1]!.image!, det.marks[2]!.image!, det.marks[3]!.image!] as [[number, number], [number, number], [number, number], [number, number]],
      bed_mm: [det.marks[0]!.bed, det.marks[1]!.bed, det.marks[2]!.bed, det.marks[3]!.bed] as [[number, number], [number, number], [number, number], [number, number]],
      to_bed: det.to_bed,
      rms_mm: det.rms_mm,
      method: 'marks' as const,
      samples,
    });
    const two = await api.saveCameraSetup({ setup: { ...blank, alignment: align([sample(20), sample(0)]), prefs: { ...blank.prefs, material_height_mm: 500 } } });
    expect(two.alignment!.height!.source).toBe('measured');
    expect([two.alignment!.height!.min_height_mm, two.alignment!.height!.max_height_mm]).toEqual([0, 20]);
    expect(two.alignment!.reference_height_mm).toBe(0);
    expect(two.prefs.material_height_mm).toBe(300);
    const one = await api.saveCameraSetup({ setup: { ...blank, calibration: lens, alignment: align([sample(0)]) } });
    expect(one.alignment!.height!.source).toBe('modelled');
    const none = await api.saveCameraSetup({ setup: { ...blank, alignment: align([sample(0)]) } });
    expect(none.alignment!.height).toBeNull();
    await api.saveCameraSetup({ setup: blank });
  });
});
