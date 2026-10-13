import { describe, expect, it } from 'vitest';
import type { AlignmentSample, CameraAlignment, HeightModel, MarksReport } from '@/api/types';
import { applyH, invertH, type Mat3 } from './geometry';
import { MIN_HEIGHT_GAP_MM, alignmentFromMarks, bedToImageAt, heightStatus, maxShiftMm, mergeSample, removeSample, toBedAt } from './height';
import { parseSetup } from './setup';

/** Cama (mm) -> imagen a altura 0 de una cámara inclinada de 1280x720, y su pendiente por mm. */
const P0: Mat3 = [[3.6, 0.1, 120], [0.05, -3.5, 640], [0.0002, -0.0001, 1]];
const G: [number, number, number] = [-0.9, -0.5, -0.0035];

function model(over: Partial<HeightModel> = {}): HeightModel {
  return { ref_height_mm: 0, bed_to_image: P0, gradient: G, source: 'measured', min_height_mm: 0, max_height_mm: 20, ...over };
}

function alignment(over: Partial<CameraAlignment> = {}): CameraAlignment {
  const toBed = invertH(P0)!.map((r) => r.map((v) => v / invertH(P0)![2][2])) as Mat3;
  return {
    image_size: [1280, 720],
    image_px: [[150, 600], [1100, 610], [1080, 140], [160, 130]],
    bed_mm: [[22, 15], [278, 15], [278, 185], [22, 185]],
    to_bed: toBed,
    rms_mm: 0.05,
    height: model(),
    ...over,
  };
}

describe('toBedAt', () => {
  it('sin modelo devuelve la homografía guardada', () => {
    const a = alignment({ height: null });
    expect(toBedAt(a, 12)).toBe(a.to_bed);
  });

  it('a la altura de referencia coincide con la guardada y en otra mueve el contenido', () => {
    const a = alignment();
    const at0 = toBedAt(a, 0);
    for (const p of [[100, 100], [640, 360], [1200, 650]] as [number, number][]) {
      const [x, y] = applyH(at0, p);
      const [x0, y0] = applyH(a.to_bed, p);
      expect(Math.hypot(x - x0, y - y0)).toBeLessThan(1e-6);
    }
    // La imagen a 20 mm de altura: la misma cama proyecta en otro píxel; la inversa lo recupera.
    const m = bedToImageAt(model(), 20);
    const px = applyH(m, [150, 100]);
    const back = applyH(toBedAt(a, 20), px);
    expect(Math.hypot(back[0] - 150, back[1] - 100)).toBeLessThan(1e-6);
    // Y sin corregir (con la de 0 mm) el error es de milímetros.
    const naive = applyH(at0, px);
    expect(Math.hypot(naive[0] - 150, naive[1] - 100)).toBeGreaterThan(2);
  });

  it('interpola y extrapola de forma lineal en la tercera columna', () => {
    const a = bedToImageAt(model(), 0);
    const b = bedToImageAt(model(), 10);
    const c = bedToImageAt(model(), 20);
    for (let r = 0; r < 3; r++) {
      expect(b[r]![2]! - a[r]![2]!).toBeCloseTo(c[r]![2]! - b[r]![2]!, 9);
      expect(b[r]![0]).toBe(a[r]![0]);
      expect(b[r]![1]).toBe(a[r]![1]);
    }
    expect(P0[0]![2]).toBe(120); // no se modifica la matriz original
  });
});

describe('heightStatus y desplazamiento', () => {
  it('distingue sin modelo, estimado y medido (con extrapolación)', () => {
    expect(heightStatus(null, 3).kind).toBe('none');
    expect(heightStatus(alignment({ height: null }), 3).kind).toBe('none');
    expect(heightStatus(alignment({ height: model({ source: 'modelled', max_height_mm: 0 }) }), 3).kind).toBe('modelled');
    expect(heightStatus(alignment(), 12)).toEqual({ kind: 'measured', min: 0, max: 20, outside: 0 });
    expect(heightStatus(alignment(), 26)).toEqual({ kind: 'measured', min: 0, max: 20, outside: 6 });
  });

  it('el desplazamiento crece con el grosor y es cero en la altura de referencia', () => {
    const a = alignment();
    expect(maxShiftMm(a, 0)).toBeLessThan(1e-6);
    const s10 = maxShiftMm(a, 10);
    const s20 = maxShiftMm(a, 20);
    expect(s10).toBeGreaterThan(1);
    expect(s20).toBeGreaterThan(s10);
  });
});

describe('capturas por altura', () => {
  const sample = (h: number): AlignmentSample => ({ height_mm: h, image_px: [[0, 0], [1, 0], [1, 1], [0, 1]], bed_mm: [[0, 0], [1, 0], [1, 1], [0, 1]] });

  it('una altura igual reemplaza, una distinta y lejana se suma ordenada, una cercana se rechaza', () => {
    let r = mergeSample([sample(0)], sample(20));
    expect(r.samples.map((s) => s.height_mm)).toEqual([0, 20]);
    r = mergeSample(r.samples, sample(0.1));
    expect(r.samples.map((s) => s.height_mm)).toEqual([0.1, 20]);
    r = mergeSample(r.samples, sample(10));
    expect(r.samples.map((s) => s.height_mm)).toEqual([0.1, 10, 20]);
    const near = mergeSample(r.samples, sample(10 + MIN_HEIGHT_GAP_MM / 2));
    expect(near.error).toBe('tooClose');
    expect(near.samples).toHaveLength(3);
  });

  it('quitar la última captura quita la alineación y quitar otra deja la restante', () => {
    const a = alignment({ method: 'marks', samples: [sample(0), sample(20)] });
    const one = removeSample(a, 20)!;
    expect(one.samples).toHaveLength(1);
    expect(one.height).toBeNull();
    expect(one.reference_height_mm).toBe(0);
    expect(removeSample(one, 0)).toBeNull();
  });
});

function report(over: Partial<MarksReport> = {}): MarksReport {
  const bed: [number, number][] = [[22, 15], [278, 15], [278, 185], [22, 185]];
  return {
    marks: bed.map((b, i) => ({ index: i, bed: b, image: applyH(invertH(alignment().to_bed)!, b), confidence: 0.95, used: true, residual_mm: 0.1, loo_mm: null })),
    candidates: [],
    to_bed: alignment().to_bed,
    expected: 4,
    found: 4,
    used: 4,
    rms_mm: 0.1,
    max_residual_mm: 0.12,
    loo_rms_mm: null,
    loo_max_mm: null,
    scale_px_per_mm: 3.5,
    used_prior: false,
    warnings: [],
    ...over,
  };
}

describe('alignmentFromMarks', () => {
  it('con una captura deja la homografía del reporte y las esquinas del rectángulo', () => {
    const r = alignmentFromMarks(report(), { imageSize: [1280, 720], height: 0, existing: null });
    expect(r.error).toBeNull();
    const a = r.alignment!;
    expect(a.method).toBe('marks');
    expect(a.samples).toHaveLength(1);
    expect(a.samples![0]!.height_mm).toBe(0);
    expect(a.bed_mm).toEqual([[22, 15], [278, 15], [278, 185], [22, 185]]);
    expect(a.to_bed).toEqual(alignment().to_bed);
    const p = applyH(a.to_bed, a.image_px[2]);
    expect(Math.hypot(p[0] - 278, p[1] - 185)).toBeLessThan(1e-6);
  });

  it('suma una segunda altura a la alineación de la misma imagen y no a la de otro tamaño', () => {
    const first = alignmentFromMarks(report(), { imageSize: [1280, 720], height: 0, existing: null }).alignment!;
    const second = alignmentFromMarks(report(), { imageSize: [1280, 720], height: 20, existing: first }).alignment!;
    expect(second.samples!.map((s) => s.height_mm)).toEqual([0, 20]);
    expect(second.reference_height_mm).toBe(0);
    const other = alignmentFromMarks(report(), { imageSize: [640, 360], height: 20, existing: first }).alignment!;
    expect(other.samples!.map((s) => s.height_mm)).toEqual([20]);
    const close = alignmentFromMarks(report(), { imageSize: [1280, 720], height: 1, existing: first });
    expect(close.error).toBe('tooClose');
  });

  it('exige al menos 4 marcas usadas', () => {
    const r = report();
    r.marks[0]!.used = false;
    expect(alignmentFromMarks(r, { imageSize: [1280, 720], height: 0, existing: null }).error).toBe('tooFew');
  });
});

describe('parseSetup con marcas y altura', () => {
  it('conserva las capturas, el modelo y la homografía del motor, y descarta lo mal formado', () => {
    const base = alignment({ method: 'marks', samples: [{ height_mm: 0, image_px: [[0, 0], [1, 0], [1, 1], [0, 1]], bed_mm: [[0, 0], [1, 0], [1, 1], [0, 1]] }], reference_height_mm: 0 });
    // El motor entrega una homografía ajustada con todas las marcas: la interfaz no la recalcula.
    const engine: Mat3 = [[0.28, 0.001, -30], [0.002, -0.285, 190], [1e-6, 2e-6, 1]];
    const loaded = parseSetup({ alignment: { ...base, to_bed: engine, rms_mm: 0.07 }, prefs: { material_height_mm: 7.5 } });
    expect(loaded.alignment?.to_bed).toEqual(engine);
    expect(loaded.alignment?.rms_mm).toBe(0.07);
    expect(loaded.alignment?.method).toBe('marks');
    expect(loaded.alignment?.samples).toHaveLength(1);
    expect(loaded.alignment?.height?.source).toBe('measured');
    expect(loaded.prefs.material_height_mm).toBe(7.5);

    const bad = parseSetup({ alignment: { ...base, height: { ...model(), source: 'raro' }, samples: [{ height_mm: 'x' }] } });
    expect(bad.alignment?.height ?? null).toBeNull();
    expect(bad.alignment?.samples ?? []).toEqual([]);
  });

  it('una alineación manual vieja queda exactamente como antes', () => {
    const old = { image_size: [640, 480], image_px: [[100, 400], [540, 410], [530, 60], [110, 70]], bed_mm: [[10, 10], [289, 10], [289, 225], [10, 225]], to_bed: [[0, 0, 0], [0, 0, 0], [0, 0, 0]], rms_mm: 0 };
    const a = parseSetup({ alignment: old }).alignment!;
    expect(a.method).toBeUndefined();
    expect(a.samples).toBeUndefined();
    expect(a.height).toBeUndefined();
    expect(parseSetup({ prefs: { opacity: 0.5 } }).prefs.material_height_mm).toBeUndefined();
  });
});
