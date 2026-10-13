/**
 * Persistencia de la calibración y la alineación para que no se pierdan al reiniciar.
 *
 * Todo pasa por `LaserApi` (`loadCameraSetup` / `saveCameraSetup`): en Tauri es `camera.json` en la
 * carpeta de configuración (src-tauri/src/edit/camera.rs, que valida y recalcula la homografía) y
 * en modo mock, memoria y `localStorage` (api/mock/camera.ts). Aquí solo se valida lo recibido.
 */
import { getApi, type LaserApi } from '@/api';
import type { AlignmentSample, HeightModel } from '@/api/types';
import { emptyCameraSetup, type CameraAlignment, type CameraPrefs, type CameraSetup } from '@/store/camera';
import { applyH, homographyFrom4, isConvexQuad, type Mat3, type Quad } from './geometry';

export interface SetupBackend {
  load(): Promise<CameraSetup>;
  save(setup: CameraSetup): Promise<CameraSetup>;
}

const isNum = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const isPair = (v: unknown): v is [number, number] => Array.isArray(v) && v.length === 2 && v.every(isNum);
const isQuad = (v: unknown): v is Quad => Array.isArray(v) && v.length === 4 && v.every(isPair);
const isTriple = (v: unknown): v is [number, number, number] => Array.isArray(v) && v.length === 3 && v.every(isNum);
const isMat3 = (v: unknown): v is Mat3 => Array.isArray(v) && v.length === 3 && v.every(isTriple);

function parseSamples(raw: unknown): AlignmentSample[] {
  if (!Array.isArray(raw)) return [];
  const out: AlignmentSample[] = [];
  for (const s of raw as Record<string, unknown>[]) {
    if (!s || typeof s !== 'object' || !isNum(s.height_mm)) continue;
    const img = s.image_px;
    const bed = s.bed_mm;
    if (!Array.isArray(img) || !Array.isArray(bed) || img.length !== bed.length || img.length < 4) continue;
    if (!img.every(isPair) || !bed.every(isPair)) continue;
    out.push({
      height_mm: s.height_mm,
      image_px: img as [number, number][],
      bed_mm: bed as [number, number][],
      loo_rms_mm: isNum(s.loo_rms_mm) ? s.loo_rms_mm : null,
      loo_max_mm: isNum(s.loo_max_mm) ? s.loo_max_mm : null,
    });
  }
  return out;
}

function parseHeight(raw: unknown): HeightModel | null {
  const h = raw as Record<string, unknown> | null | undefined;
  if (!h || typeof h !== 'object') return null;
  if (!isNum(h.ref_height_mm) || !isMat3(h.bed_to_image) || !isTriple(h.gradient)) return null;
  if (!isNum(h.min_height_mm) || !isNum(h.max_height_mm)) return null;
  if (h.source !== 'measured' && h.source !== 'modelled') return null;
  return {
    ref_height_mm: h.ref_height_mm,
    bed_to_image: h.bed_to_image,
    gradient: h.gradient,
    source: h.source,
    min_height_mm: h.min_height_mm,
    max_height_mm: h.max_height_mm,
  };
}

/**
 * Suma a la alineación base (4 puntos) lo que añaden las marcas y la altura. Con marcas grabadas
 * la homografía viene del motor (ajustada con todas las marcas), no de los 4 puntos de pantalla.
 * Una alineación manual sin altura queda exactamente como la base.
 */
function withExtras(base: CameraAlignment, a: Record<string, unknown>): CameraAlignment {
  const samples = parseSamples(a.samples);
  const height = parseHeight(a.height);
  const ref = isNum(a.reference_height_mm) ? a.reference_height_mm : 0;
  const marks = samples.length > 0;
  if (!marks && !height && ref === 0) return base;
  return {
    ...base,
    ...(marks && isMat3(a.to_bed) ? { to_bed: a.to_bed, rms_mm: isNum(a.rms_mm) ? a.rms_mm : base.rms_mm } : {}),
    method: marks ? 'marks' : 'manual',
    reference_height_mm: ref,
    samples,
    height,
  };
}

/** Recalcula la homografía y el error de una alineación. `null` si los 4 puntos no sirven. */
export function fitAlignment(
  imageSize: [number, number],
  imagePx: Quad,
  bedMm: Quad,
): CameraAlignment | null {
  if (!isConvexQuad(imagePx, 1) || !isConvexQuad(bedMm, 1e-3)) return null;
  const h = homographyFrom4(imagePx, bedMm);
  if (!h) return null;
  const rms = Math.sqrt(
    imagePx.reduce((s, p, i) => {
      const q = applyH(h, p);
      return s + (q[0] - bedMm[i]![0]) ** 2 + (q[1] - bedMm[i]![1]) ** 2;
    }, 0) / 4,
  );
  return { image_size: imageSize, image_px: imagePx, bed_mm: bedMm, to_bed: h, rms_mm: rms };
}

/** Lee una configuración de cualquier origen; descarta lo que no tiene la forma esperada. */
export function parseSetup(raw: unknown): CameraSetup {
  const out = emptyCameraSetup();
  if (typeof raw !== 'object' || raw === null) return out;
  const r = raw as Record<string, unknown>;
  const c = r.calibration as Record<string, unknown> | null | undefined;
  const fields = ['width', 'height', 'fx', 'fy', 'cx', 'cy', 'k1', 'k2', 'p1', 'p2', 'k3', 'rms_error_px', 'views_used'];
  if (c && typeof c === 'object' && fields.every((f) => isNum(c[f])) && (c.width as number) > 0 && (c.fx as number) > 0) {
    out.calibration = c as unknown as CameraSetup['calibration'];
  }
  const a = r.alignment as Record<string, unknown> | null | undefined;
  if (a && typeof a === 'object' && isPair(a.image_size) && isQuad(a.image_px) && isQuad(a.bed_mm)) {
    const base = fitAlignment(a.image_size, a.image_px, a.bed_mm);
    out.alignment = base && withExtras(base, a);
  }
  const p = (r.prefs ?? {}) as Partial<Record<keyof CameraPrefs, unknown>>;
  const str = (v: unknown) => (typeof v === 'string' && v ? v : null);
  out.prefs = {
    device_id: str(p.device_id),
    device_label: str(p.device_label),
    width: isNum(p.width) ? p.width : null,
    height: isNum(p.height) ? p.height : null,
    opacity: isNum(p.opacity) ? Math.min(1, Math.max(0, p.opacity)) : out.prefs.opacity,
    visible: p.visible === true,
    ...(isNum(p.material_height_mm) && p.material_height_mm > 0 ? { material_height_mm: Math.min(300, p.material_height_mm) } : {}),
  };
  return out;
}

/** Respaldo real: `LaserApi` (en Tauri, `camera.json`; en modo mock, memoria y `localStorage`). */
export function createApiBackend(api: () => Pick<LaserApi, 'loadCameraSetup' | 'saveCameraSetup'> = getApi): SetupBackend {
  return {
    load: async () => parseSetup(await api().loadCameraSetup()),
    save: async (setup) => parseSetup(await api().saveCameraSetup({ setup })),
  };
}

let backend: SetupBackend | null = null;

export function getSetupBackend(): SetupBackend {
  backend ??= createApiBackend();
  return backend;
}

/** Para pruebas: sustituye el respaldo activo. */
export function setSetupBackend(b: SetupBackend | null): void {
  backend = b;
}
