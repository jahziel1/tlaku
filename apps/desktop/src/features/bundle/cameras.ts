import type { CameraAlignment, CameraPreset, CameraSetup, HeadCamera, LensCalibration } from '@/api/types';
import type { CamerasBundle } from '@/api';
import { parsePresets, parseSetup } from '@/features/camera/setup';
import { sameValue } from '@/features/materials/library';
import { hasCredentials, stripCredentials } from '@/features/camera/networkUrl';
import { MAX_PRESETS } from '@/features/camera/presets';

/**
 * Sección `cameras` del paquete `.tlakubundle` (M4): calibración de lente, alineación con la cama,
 * cámara del cabezal y configuraciones con nombre. Lógica pura: armar la sección, leerla con
 * cuidado y calcular qué cambiaría (la vista previa). Aplicarla es de `apply.ts`.
 *
 * Reglas de seguridad y de coherencia:
 * - Las URL de las cámaras de red salen SIN usuario ni clave (`stripCredentials`): las credenciales
 *   viven solo en el perfil local (`camera.json`). Al importar, si un archivo ajeno las trae, también
 *   se quitan.
 * - Los ids de dispositivo no viajan (son de cada equipo); sí la etiqueta.
 * - La calibración, la alineación y la del cabezal se reemplazan (solo hay una de cada); las
 *   configuraciones con nombre se mezclan y nunca pisan las locales.
 * - La alineación y la del cabezal dependen de la lente: si entra una lente distinta y el paquete no
 *   trae las suyas, las locales se quitan en lugar de quedar desalineadas.
 */

/** Cómo quedaría una pieza única respecto de la local; `null` = el paquete no la trae. */
export type PartState = 'new' | 'replace' | 'same';
export type CameraPart = 'calibration' | 'alignment' | 'head';

export interface PresetsPlan {
  /** Configuraciones que entran (incluidas las renombradas). */
  added: CameraPreset[];
  /** De las `added`, las que chocaban de nombre con otra distinta y entran como copia. */
  renamed: number;
  /** Idénticas a una que ya estaba. */
  skipped: number;
  /** No cupieron por el máximo de configuraciones. */
  dropped: number;
  /** No eran válidas. */
  invalid: number;
  /** Configuraciones cuya URL traía usuario y clave, que se quitaron. */
  strippedUrls: number;
}

export interface CamerasPlan {
  calibration: PartState | null;
  alignment: PartState | null;
  head: PartState | null;
  presets: PresetsPlan | null;
  /** Piezas presentes en el paquete pero con datos no válidos (se descartan). */
  invalid: CameraPart[];
  /** Piezas locales que se quitan por depender de la lente que entra. */
  cleared: ('alignment' | 'head')[];
  /** La configuración completa que quedaría. */
  setup: CameraSetup;
}

const isObject = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);

const withoutDevice = (p: CameraPreset): CameraPreset => ({
  ...p,
  device_id: null,
  network_url: stripCredentials(p.network_url),
});

/** La sección para exportar: sin credenciales en las URL ni ids de dispositivo. */
export function buildCamerasSection(setup: CameraSetup): CamerasBundle {
  const head = setup.head;
  return {
    calibration: setup.calibration ? structuredClone(setup.calibration) : null,
    alignment: setup.alignment ? structuredClone(setup.alignment) : null,
    head: head && (head.calibration || head.enabled) ? structuredClone(head) : null,
    presets: (setup.presets ?? []).map((p) => withoutDevice(structuredClone(p))),
  };
}

/** Lo que la sección lleva, para mostrarlo al exportar. */
export function camerasSummary(setup: CameraSetup): { lens: boolean; alignment: boolean; head: boolean; presets: number } {
  return {
    lens: setup.calibration !== null,
    alignment: setup.alignment !== null,
    head: setup.head?.calibration != null,
    presets: setup.presets?.length ?? 0,
  };
}

/** Campos de la alineación que definen su significado (lo demás lo recalcula el motor al guardar). */
const alignmentKey = (a: CameraAlignment | null) =>
  a && { size: a.image_size, px: a.image_px, mm: a.bed_mm, ref: a.reference_height_mm ?? 0, samples: a.samples ?? [] };

function partState<T>(incoming: T | null, local: T | null, same: (a: T, b: T) => boolean = sameValue): PartState | null {
  if (incoming === null) return null;
  if (local === null) return 'new';
  return same(incoming, local) ? 'same' : 'replace';
}

const sameLens = (a: LensCalibration, b: LensCalibration) => sameValue(a, b);
const sameHead = (a: HeadCamera, b: HeadCamera) => sameValue(a, b);

/** Nombre libre entre `taken` (sin distinguir mayúsculas): `nombre (2)`, `nombre (3)`... */
function freeName(base: string, taken: ReadonlySet<string>): string {
  for (let k = 2; ; k++) {
    const name = `${base} (${k})`;
    if (!taken.has(name.toLowerCase())) return name;
  }
}

function planPresets(raw: unknown, local: readonly CameraPreset[]): PresetsPlan | null {
  if (raw === undefined || raw === null) return null;
  const list = Array.isArray(raw) ? raw : [];
  const parsed = parsePresets(list);
  const strippedUrls = list.filter((p) => isObject(p) && typeof p.network_url === 'string' && hasCredentials(p.network_url)).length;
  const taken = new Set(local.map((p) => p.name.toLowerCase()));
  const byName = new Map(local.map((p) => [p.name.toLowerCase(), p]));
  const added: CameraPreset[] = [];
  let renamed = 0;
  let skipped = 0;
  let dropped = 0;
  for (const incoming of parsed.map(withoutDevice)) {
    const existing = byName.get(incoming.name.toLowerCase());
    if (existing && sameValue({ ...withoutDevice(existing), name: '' }, { ...incoming, name: '' })) {
      skipped++;
      continue;
    }
    if (local.length + added.length >= MAX_PRESETS) {
      dropped++;
      continue;
    }
    let preset = incoming;
    if (existing) {
      preset = { ...incoming, name: freeName(incoming.name, taken) };
      renamed++;
    }
    taken.add(preset.name.toLowerCase());
    added.push(preset);
  }
  return { added, renamed, skipped, dropped, invalid: Math.max(0, list.length - parsed.length), strippedUrls };
}

/** Qué cambiaría aplicar la sección sobre la configuración de cámara actual; `null` si no es una sección. */
export function planCameras(raw: unknown, current: CameraSetup): CamerasPlan | null {
  if (!isObject(raw)) return null;
  const parsed = parseSetup({ calibration: raw.calibration, alignment: raw.alignment, head: raw.head, prefs: {} });
  const incomingHead = parsed.head?.calibration ? parsed.head : null;
  const invalid: CameraPart[] = [];
  if (isObject(raw.calibration) && parsed.calibration === null) invalid.push('calibration');
  if (isObject(raw.alignment) && parsed.alignment === null) invalid.push('alignment');
  if (isObject(raw.head) && isObject(raw.head.calibration) && incomingHead === null) invalid.push('head');

  const localHead = current.head?.calibration ? current.head : null;
  const calibration = partState(parsed.calibration, current.calibration, sameLens);
  const alignment = partState(parsed.alignment, current.alignment, (a, b) => sameValue(alignmentKey(a), alignmentKey(b)));
  const head = partState(incomingHead, localHead, sameHead);
  const presets = planPresets(raw.presets, current.presets ?? []);

  // Una lente distinta deja sin sentido la alineación y la del cabezal que no vienen con ella.
  const lensChanged = calibration === 'new' || calibration === 'replace';
  const cleared: CamerasPlan['cleared'] = [];
  let nextAlignment = parsed.alignment ?? current.alignment;
  let nextHead = incomingHead ?? current.head ?? { enabled: false, calibration: null };
  if (lensChanged && parsed.alignment === null && current.alignment) {
    nextAlignment = null;
    cleared.push('alignment');
  }
  if (lensChanged && incomingHead === null && current.head?.calibration) {
    nextHead = { enabled: current.head.enabled, calibration: null };
    cleared.push('head');
  }
  const setup: CameraSetup = {
    ...current,
    calibration: parsed.calibration ?? current.calibration,
    alignment: nextAlignment,
    head: nextHead,
    presets: [...(current.presets ?? []), ...(presets?.added ?? [])],
  };
  return { calibration, alignment, head, presets, invalid, cleared, setup };
}

/** Cuántos cambios reales traería: piezas nuevas o que reemplazan, más las configuraciones nuevas. */
export function camerasChanges(plan: CamerasPlan): number {
  const part = (s: PartState | null) => (s === 'new' || s === 'replace' ? 1 : 0);
  return part(plan.calibration) + part(plan.alignment) + part(plan.head) + (plan.presets?.added.length ?? 0) + plan.cleared.length;
}
