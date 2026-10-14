/**
 * Persistencia de la calibración y la alineación para que no se pierdan al reiniciar.
 *
 * Todo pasa por `LaserApi` (`loadCameraSetup` / `saveCameraSetup`): en Tauri es `camera.json` en la
 * carpeta de configuración (src-tauri/src/edit/camera.rs, que valida y recalcula la homografía) y
 * en modo mock, memoria y `localStorage` (api/mock/camera.ts). Aquí solo se valida lo recibido.
 */
import { getApi, type LaserApi } from '@/api';
import type { AlignmentSample, CameraPreset, HeadCamera, HeadCalibration, HeightModel, LensCalibration } from '@/api/types';
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

const isMat2 = (v: unknown): v is [[number, number], [number, number]] => Array.isArray(v) && v.length === 2 && v.every(isPair);

/** Calibración guardada: los campos del ojo de pez son opcionales (las de antes no los traen). */
function parseLens(c: Record<string, unknown>): LensCalibration | null {
  const fields = ['width', 'height', 'fx', 'fy', 'cx', 'cy', 'k1', 'k2', 'p1', 'p2', 'k3', 'rms_error_px', 'views_used'];
  if (!fields.every((f) => isNum(c[f])) || (c.width as number) <= 0 || (c.fx as number) <= 0) return null;
  const lens = { ...c } as unknown as LensCalibration;
  delete lens.model;
  delete lens.k4;
  delete lens.view_zoom;
  if (c.model === 'fisheye' || c.model === 'brown_conrady') lens.model = c.model;
  if (isNum(c.k4)) lens.k4 = c.k4;
  if (isNum(c.view_zoom) && c.view_zoom > 0) lens.view_zoom = c.view_zoom;
  return lens;
}

function parseHeadCalibration(raw: unknown): HeadCalibration | null {
  const c = raw as Record<string, unknown> | null | undefined;
  if (!c || typeof c !== 'object') return null;
  if (!isPair(c.image_size) || !isPair(c.ref_px) || !isMat2(c.px_to_mm) || !isPair(c.offset_mm)) return null;
  return {
    image_size: c.image_size,
    ref_px: c.ref_px,
    px_to_mm: c.px_to_mm,
    offset_mm: c.offset_mm,
    rms_mm: isNum(c.rms_mm) ? c.rms_mm : 0,
    observations: isNum(c.observations) ? c.observations : 0,
  };
}

/** Cámara del cabezal; sin ella (configuraciones de antes de la ola 4) está apagada y sin calibrar. */
export function parseHead(raw: unknown): HeadCamera {
  const h = raw as Record<string, unknown> | null | undefined;
  if (!h || typeof h !== 'object') return { enabled: false, calibration: null };
  return { enabled: h.enabled === true, calibration: parseHeadCalibration(h.calibration) };
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

/** Configuraciones de cámara con nombre guardadas; descarta las que no tienen la forma esperada. */
export function parsePresets(raw: unknown): CameraPreset[] {
  if (!Array.isArray(raw)) return [];
  const out: CameraPreset[] = [];
  const str = (v: unknown) => (typeof v === 'string' && v !== '' ? v : null);
  const size = (v: unknown) => (isNum(v) && v > 0 ? Math.round(v) : null);
  for (const r of raw as Record<string, unknown>[]) {
    if (!r || typeof r !== 'object' || typeof r.name !== 'string' || r.name.trim() === '') continue;
    const name = r.name.trim();
    const preset: CameraPreset = {
      name,
      source: r.source === 'network' ? 'network' : 'local',
      device_id: str(r.device_id),
      device_label: str(r.device_label),
      width: size(r.width),
      height: size(r.height),
      network_url: str(r.network_url),
      dpm: isNum(r.dpm) && r.dpm > 0 ? r.dpm : null,
      offset_x_mm: isNum(r.offset_x_mm) ? r.offset_x_mm : 0,
      offset_y_mm: isNum(r.offset_y_mm) ? r.offset_y_mm : 0,
      grayscale: r.grayscale === true,
    };
    const at = out.findIndex((p) => p.name.toLowerCase() === name.toLowerCase());
    if (at >= 0) out[at] = preset;
    else out.push(preset);
  }
  return out;
}

/** Lee una configuración de cualquier origen; descarta lo que no tiene la forma esperada. */
export function parseSetup(raw: unknown): CameraSetup {
  const out = emptyCameraSetup();
  if (typeof raw !== 'object' || raw === null) return out;
  const r = raw as Record<string, unknown>;
  const c = r.calibration as Record<string, unknown> | null | undefined;
  if (c && typeof c === 'object') out.calibration = parseLens(c);
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
    ...(p.grayscale === true ? { grayscale: true } : {}),
    ...(p.update_on_job_finish === true ? { update_on_job_finish: true } : {}),
    ...(p.source === 'network' ? { source: 'network' as const } : {}),
    ...(str(p.network_url) ? { network_url: str(p.network_url) } : {}),
    ...(isNum(p.dpm) && p.dpm > 0 ? { dpm: p.dpm } : {}),
    ...(isNum(p.offset_x_mm) && p.offset_x_mm !== 0 ? { offset_x_mm: p.offset_x_mm } : {}),
    ...(isNum(p.offset_y_mm) && p.offset_y_mm !== 0 ? { offset_y_mm: p.offset_y_mm } : {}),
  };
  out.head = parseHead(r.head);
  const presets = parsePresets(r.presets);
  if (presets.length > 0) out.presets = presets;
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
