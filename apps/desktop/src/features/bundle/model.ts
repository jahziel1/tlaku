import type { DeviceProfile, MaterialLibrary, MaterialPreset, UserBundle } from '@/api';
import { applyBundle, buildBundle, type BundleChange } from '@/features/settings/bundle';
import { mergeLibraries, sameValue } from '@/features/materials/library';
import { t } from '@/i18n';
import { normalizeDevice, validateDevice } from '@/lib/device';
import { normalizeLayer } from '@/lib/layers';
import type { ShortcutOverrides } from '@/commands/shortcuts';
import type { Prefs } from '@/store/prefs';

/**
 * Paquetes de usuario `.tlakubundle`: máquinas (con sus macros y posiciones), biblioteca de
 * materiales, preferencias y atajos, y la máquina activa, en un solo archivo. Esta parte es lógica
 * pura (sin API ni store): armar el paquete, leerlo con cuidado y calcular qué se aplicaría.
 * Aplicarlo de verdad es de `apply.ts`.
 */

export const USER_BUNDLE_FORMAT = 'tlaku-bundle' as const;
export const USER_BUNDLE_EXTENSION = 'tlakubundle';
export const DEFAULT_USER_BUNDLE_NAME = `tlaku.${USER_BUNDLE_EXTENSION}`;

/** Secciones que se pueden exportar y aplicar por separado. */
export type BundleSection = 'devices' | 'materials' | 'prefs';
export const BUNDLE_SECTIONS: readonly BundleSection[] = ['devices', 'materials', 'prefs'];

export type SectionChoice = Record<BundleSection, boolean>;
export const allSections = (value = true): SectionChoice => ({ devices: value, materials: value, prefs: value });

/** Lo que `buildUserBundle` necesita saber del estado de la aplicación. */
export interface BundleSource {
  devices: readonly DeviceProfile[];
  activeDeviceId: string | null;
  library: MaterialLibrary;
  prefs: Prefs;
  shortcuts: ShortcutOverrides;
}

/** Arma el paquete con las secciones elegidas; las no elegidas quedan en `null` (máquinas: ausentes). */
export function buildUserBundle(
  source: BundleSource,
  opts: { sections?: SectionChoice; app?: string; now?: Date } = {},
): UserBundle {
  const sections = opts.sections ?? allSections();
  return {
    format: USER_BUNDLE_FORMAT,
    version: 1,
    created: (opts.now ?? new Date()).toISOString(),
    app: opts.app ?? '',
    ...(sections.devices ? { devices: structuredClone([...source.devices]), active_device: source.activeDeviceId } : {}),
    materials: sections.materials ? structuredClone(source.library) : null,
    prefs: sections.prefs ? buildBundle(source.prefs, source.shortcuts, 'all') : null,
  };
}

/** Comprueba la forma básica de lo que devolvió el motor. */
export function isUserBundle(raw: unknown): raw is UserBundle {
  const b = raw as { format?: unknown; version?: unknown } | null;
  return !!b && typeof b === 'object' && b.format === USER_BUNDLE_FORMAT && b.version === 1;
}

// ---------------------------------------------------------------------------
// Qué se aplicaría
// ---------------------------------------------------------------------------

export type InvalidReason = 'notObject' | 'noId' | 'noName' | 'invalid' | 'duplicate';

/** Un elemento del paquete que se descarta, para avisar al usuario. */
export interface InvalidItem {
  /** Id (o posición «#n» si no tiene). */
  id: string;
  reason: InvalidReason;
  /** Campos con problema (solo con `invalid`). */
  fields?: string[];
}

export interface DevicesPlan {
  /** Perfiles con un id que no existe todavía. */
  added: DeviceProfile[];
  /** Perfiles con el id de uno existente y contenido distinto: lo reemplazan. */
  replaced: DeviceProfile[];
  /** Perfiles idénticos a los que ya hay. */
  unchanged: DeviceProfile[];
  invalid: InvalidItem[];
  /** Máquina activa que el paquete pide y que existiría después de aplicar; `null` = no cambia. */
  activeDevice: string | null;
}

export interface MaterialsPlan {
  /** Biblioteca resultante (la actual más lo que entra). */
  library: MaterialLibrary;
  added: number;
  /** De los `added`, los que chocaban de id con otro distinto y entran con id nuevo. */
  renamed: number;
  /** Idénticos a uno que ya estaba. */
  skipped: number;
  /** Ajustes del paquete que no eran válidos. */
  invalid: number;
}

export interface PrefsPlan {
  change: BundleChange;
}

export interface ImportPlan {
  created: string;
  app: string;
  /** `null` = el paquete no trae esa sección. */
  devices: DevicesPlan | null;
  materials: MaterialsPlan | null;
  prefs: PrefsPlan | null;
}

/** Estado actual contra el que se compara el paquete. */
export interface CurrentState extends BundleSource {
  /** Ids de comandos que esta versión conoce (para avisar de atajos de comandos desconocidos). */
  knownCommands: ReadonlySet<string>;
}

const isObject = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);

/** Normaliza y valida los perfiles del paquete; devuelve los buenos y los descartados. */
export function sanitizeDevices(raw: unknown): { devices: DeviceProfile[]; invalid: InvalidItem[] } {
  const devices: DeviceProfile[] = [];
  const invalid: InvalidItem[] = [];
  const seen = new Set<string>();
  const list = Array.isArray(raw) ? raw : [];
  list.forEach((item, i) => {
    if (!isObject(item)) {
      invalid.push({ id: `#${i + 1}`, reason: 'notObject' });
      return;
    }
    const id = typeof item.id === 'string' ? item.id.trim() : '';
    if (id === '') {
      invalid.push({ id: `#${i + 1}`, reason: 'noId' });
      return;
    }
    if (typeof item.name !== 'string' || item.name.trim() === '') {
      invalid.push({ id, reason: 'noName' });
      return;
    }
    if (seen.has(id)) {
      invalid.push({ id, reason: 'duplicate' });
      return;
    }
    let device: DeviceProfile;
    let fields: string[];
    try {
      device = normalizeDevice({ ...(item as unknown as DeviceProfile), id, name: item.name });
      fields = Object.keys(validateDevice(device));
    } catch {
      invalid.push({ id, reason: 'invalid' });
      return;
    }
    if (fields.length > 0) {
      invalid.push({ id, reason: 'invalid', fields });
      return;
    }
    seen.add(id);
    devices.push(device);
  });
  return { devices, invalid };
}

/** Ajustes de material válidos del paquete y cuántos se descartaron. */
export function sanitizePresets(raw: unknown): { presets: MaterialPreset[]; invalid: number } {
  const list = isObject(raw) && Array.isArray(raw.presets) ? (raw.presets as unknown[]) : [];
  const presets: MaterialPreset[] = [];
  let invalid = 0;
  for (const p of list) {
    if (!isObject(p) || typeof p.id !== 'string' || p.id === '' || typeof p.material !== 'string' || p.material.trim() === '' || typeof p.title !== 'string' || !isObject(p.settings)) {
      invalid++;
      continue;
    }
    const thickness = typeof p.thickness === 'number' && Number.isFinite(p.thickness) && p.thickness > 0 ? p.thickness : null;
    const settings = p.settings as unknown as MaterialPreset['settings'];
    presets.push({
      id: p.id,
      material: p.material,
      thickness,
      title: p.title,
      notes: typeof p.notes === 'string' ? p.notes : '',
      device_id: typeof p.device_id === 'string' ? p.device_id : null,
      settings: normalizeLayer({ ...settings, color: typeof settings.color === 'string' ? settings.color : '#000000' }),
    });
  }
  return { presets, invalid };
}

function planDevices(bundle: UserBundle, current: CurrentState): DevicesPlan | null {
  if (!Array.isArray(bundle.devices)) return null;
  const { devices, invalid } = sanitizeDevices(bundle.devices);
  const byId = new Map(current.devices.map((d) => [d.id, d]));
  const added: DeviceProfile[] = [];
  const replaced: DeviceProfile[] = [];
  const unchanged: DeviceProfile[] = [];
  for (const d of devices) {
    const existing = byId.get(d.id);
    if (!existing) added.push(d);
    else if (sameValue(normalizeDevice(existing), d)) unchanged.push(d);
    else replaced.push(d);
  }
  const wanted = typeof bundle.active_device === 'string' ? bundle.active_device : null;
  const exists = wanted !== null && (byId.has(wanted) || devices.some((d) => d.id === wanted));
  const activeDevice = exists && wanted !== current.activeDeviceId ? wanted : null;
  return { added, replaced, unchanged, invalid, activeDevice };
}

function planMaterials(bundle: UserBundle, current: CurrentState): MaterialsPlan | null {
  if (!isObject(bundle.materials)) return null;
  const { presets, invalid } = sanitizePresets(bundle.materials);
  const merged = mergeLibraries(current.library, { version: 1, presets });
  return { library: merged.library, added: merged.added, renamed: merged.renamed, skipped: merged.skipped, invalid };
}

function planPrefs(bundle: UserBundle, current: CurrentState): PrefsPlan | null {
  if (!isObject(bundle.prefs)) return null;
  return { change: applyBundle(bundle.prefs, { prefs: current.prefs, shortcuts: current.shortcuts }, current.knownCommands) };
}

/**
 * Qué cambiaría aplicar `bundle` sobre el estado `current`, sección por sección, sin tocar nada.
 * Las secciones ausentes quedan en `null`. Lanza si no es un paquete que esta versión entienda.
 */
export function planImport(bundle: unknown, current: CurrentState): ImportPlan {
  if (!isUserBundle(bundle)) throw new Error(t('bundle.err.notBundle'));
  return {
    created: typeof bundle.created === 'string' ? bundle.created : '',
    app: typeof bundle.app === 'string' ? bundle.app : '',
    devices: planDevices(bundle, current),
    materials: planMaterials(bundle, current),
    prefs: planPrefs(bundle, current),
  };
}

/** Cuántos cambios reales produciría cada sección. */
export function sectionChanges(plan: ImportPlan): Record<BundleSection, number> {
  const d = plan.devices;
  const m = plan.materials;
  const p = plan.prefs?.change;
  return {
    devices: d ? d.added.length + d.replaced.length + (d.activeDevice ? 1 : 0) : 0,
    materials: m ? m.added : 0,
    prefs: p ? p.appliedPrefs + p.appliedShortcuts : 0,
  };
}

/** Secciones que trae el paquete (aunque no cambien nada). */
export function presentSections(plan: ImportPlan): BundleSection[] {
  return BUNDLE_SECTIONS.filter((s) => plan[s] !== null);
}
