import type { CameraSetup, DeviceProfile, MaterialLibrary, MaterialPreset, UserBundle } from '@/api';
import { PREF_KEYS } from '@/store/prefs';
import { sanitizeStored, type CostStored } from '@/features/costs/costProfiles';
import type { RemnantLibrary } from '@/features/nest/remnants';
import { getLanguage } from '@/i18n';
import { uniqueDeviceId } from '@/lib/device';
import type { UserArt } from '@/features/art/types';
import { applyBundle, buildBundle, type BundleChange } from '@/features/settings/bundle';
import { mergeLibraries, presetExtras, sameValue } from '@/features/materials/library';
import { t } from '@/i18n';
import { emptyCameraSetup } from '@/store/camera';
import { buildArtSection, planArt, type ArtPlan } from './art';
import { buildCamerasSection, camerasChanges, planCameras, type CamerasPlan } from './cameras';
import {
  buildCostsSection,
  buildHistorySection,
  buildTemplatesSection,
  costsChanges,
  defaultPolicies,
  planCosts,
  planHistory,
  planRemnants,
  planTemplates,
  remnantsChanges,
  templatesChanges,
  touched,
  uniqueLabel,
  type ConflictPolicy,
  type ConflictSection,
  type CostsPlan,
  type Diff,
  type HistoryPlan,
  type PolicyChoice,
  type RemnantsPlan,
  type TemplateItem,
  type TemplatesPlan,
} from './extras';
import { normalizeDevice, validateDevice } from '@/lib/device';
import { normalizeLayer } from '@/lib/layers';
import type { ShortcutOverrides } from '@/commands/shortcuts';
import type { Prefs } from '@/store/prefs';

/**
 * Paquetes de usuario `.tlakubundle`: máquinas (con sus macros y posiciones), biblioteca de
 * materiales, cámaras (calibración, alineación y configuraciones), biblioteca de arte propia,
 * preferencias y atajos, y la máquina activa, en un solo archivo. Esta parte es lógica
 * pura (sin API ni store): armar el paquete, leerlo con cuidado y calcular qué se aplicaría.
 * Aplicarlo de verdad es de `apply.ts`.
 */

export const USER_BUNDLE_FORMAT = 'tlaku-bundle' as const;
export const USER_BUNDLE_EXTENSION = 'tlakubundle';
export const DEFAULT_USER_BUNDLE_NAME = `tlaku.${USER_BUNDLE_EXTENSION}`;

/** Secciones que se pueden exportar y aplicar por separado. */
export type BundleSection = 'devices' | 'materials' | 'costs' | 'remnants' | 'templates' | 'cameras' | 'art' | 'prefs' | 'history';
export const BUNDLE_SECTIONS: readonly BundleSection[] = ['devices', 'materials', 'costs', 'remnants', 'templates', 'cameras', 'art', 'prefs', 'history'];
/** Secciones que solo existen desde la versión 2 del paquete. */
export const V2_SECTIONS: readonly BundleSection[] = ['costs', 'remnants', 'templates', 'history'];
/** Versión más nueva del paquete que esta aplicación entiende. */
export const USER_BUNDLE_VERSION = 2;

export type SectionChoice = Record<BundleSection, boolean>;
export const allSections = (value = true): SectionChoice => ({ devices: value, materials: value, costs: value, remnants: value, templates: value, cameras: value, art: value, prefs: value, history: value });
/** Lo que se marca por omisión: todo menos el historial de trabajos (puede ser grande y es de esa computadora). */
export const defaultSections = (): SectionChoice => ({ ...allSections(), history: false });

/** Lo que `buildUserBundle` necesita saber del estado de la aplicación. */
export interface BundleSource {
  devices: readonly DeviceProfile[];
  activeDeviceId: string | null;
  library: MaterialLibrary;
  prefs: Prefs;
  shortcuts: ShortcutOverrides;
  /** Configuración de cámara (sin ella, una vacía). */
  cameras?: CameraSetup;
  /** Biblioteca de arte propia (sin ella, vacía). */
  userArt?: readonly UserArt[];
  /** Perfiles de costos (versión 2). */
  costs?: CostStored;
  /** Inventario de retazos (versión 2). */
  remnants?: RemnantLibrary;
  /** Plantillas de proyecto (versión 2). */
  templates?: readonly TemplateItem[];
  /** Entradas del historial de trabajos (versión 2). */
  history?: readonly Record<string, unknown>[];
}

/**
 * Arma el paquete con las secciones elegidas; las no elegidas quedan en `null` (máquinas: ausentes).
 * Una sección que no se nombra en `sections` cuenta como no elegida.
 */
export function buildUserBundle(
  source: BundleSource,
  opts: { sections?: Partial<SectionChoice>; app?: string; now?: Date } = {},
): UserBundle {
  const sections: Partial<SectionChoice> = opts.sections ?? allSections();
  const v2 = V2_SECTIONS.some((k) => sections[k]);
  return {
    format: USER_BUNDLE_FORMAT,
    // Un paquete sin secciones nuevas sigue siendo de la versión 1: lo abre cualquier versión de Tlaku.
    version: v2 ? 2 : 1,
    created: (opts.now ?? new Date()).toISOString(),
    app: opts.app ?? '',
    ...(sections.devices ? { devices: structuredClone([...source.devices]), active_device: source.activeDeviceId } : {}),
    materials: sections.materials ? structuredClone(source.library) : null,
    cameras: sections.cameras ? buildCamerasSection(source.cameras ?? emptyCameraSetup()) : null,
    art: sections.art ? buildArtSection(source.userArt ?? []) : null,
    prefs: sections.prefs ? buildBundle(source.prefs, source.shortcuts, 'all') : null,
    ...(sections.costs ? { costs: buildCostsSection(source.costs ?? sanitizeStored({}, getLanguage())) } : {}),
    ...(sections.remnants ? { remnants: structuredClone(source.remnants ?? { version: 1, remnants: [] }) } : {}),
    ...(sections.templates ? { templates: buildTemplatesSection(source.templates ?? []) } : {}),
    ...(sections.history ? { history: buildHistorySection(source.history ?? []) } : {}),
  };
}

/** Comprueba la forma básica de lo que devolvió el motor. */
export function isUserBundle(raw: unknown): raw is UserBundle {
  const b = raw as { format?: unknown; version?: unknown } | null;
  return !!b && typeof b === 'object' && b.format === USER_BUNDLE_FORMAT && (b.version === 1 || b.version === 2);
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
  /** Biblioteca resultante con «conservar ambos» (la actual más lo que entra, sin pisar nada). */
  library: MaterialLibrary;
  /** Lo que entra, separado en nuevos, en conflicto (mismo id y contenido distinto) e iguales. */
  diff: Diff<MaterialPreset>;
  /** La biblioteca actual, de la que parte `resolveMaterials`. */
  base: MaterialLibrary;
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
  /** Preferencias del paquete cuyo valor difiere del actual. */
  changedPrefs: number;
  /** Los atajos del paquete difieren de los actuales. */
  shortcutsChanged: boolean;
}

export interface ImportPlan {
  created: string;
  app: string;
  /** `null` = el paquete no trae esa sección. */
  devices: DevicesPlan | null;
  materials: MaterialsPlan | null;
  cameras: CamerasPlan | null;
  art: ArtPlan | null;
  prefs: PrefsPlan | null;
  costs: CostsPlan | null;
  remnants: RemnantsPlan | null;
  /** `localNames`: nombres de todas las plantillas locales (para renombrar sin chocar). */
  templates: (TemplatesPlan & { localNames: string[] }) | null;
  history: HistoryPlan | null;
}

/** Estado actual contra el que se compara el paquete. */
export interface CurrentState extends BundleSource {
  /** Ids de comandos que esta versión conoce (para avisar de atajos de comandos desconocidos). */
  knownCommands: ReadonlySet<string>;
  /** Nombres de todas las plantillas locales (en `templates` solo van las que el paquete también trae). */
  templateNames?: readonly string[];
  /** Ids del historial de trabajos local. */
  historyIds?: ReadonlySet<string>;
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
      ...presetExtras(p),
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
  const withoutId = (p: MaterialPreset) => ({ ...p, id: '' });
  const byId = new Map(current.library.presets.map((p) => [p.id, p]));
  const diff: Diff<MaterialPreset> = { added: [], conflicts: [], unchanged: [] };
  for (const p of presets) {
    if (current.library.presets.some((q) => sameValue(withoutId(q), withoutId(p)))) diff.unchanged.push(p);
    else if (byId.has(p.id)) diff.conflicts.push(p);
    else diff.added.push(p);
  }
  return { library: merged.library, diff, base: current.library, added: merged.added, renamed: merged.renamed, skipped: merged.skipped, invalid };
}

/** Biblioteca de materiales que quedaría según la política con los que chocan de id. */
export function resolveMaterials(plan: MaterialsPlan, policy: ConflictPolicy): MaterialLibrary {
  if (policy === 'keepBoth') return plan.library;
  const added = mergeLibraries(plan.base, { version: 1, presets: plan.diff.added }).library;
  if (policy === 'skip') return added;
  const replacement = new Map(plan.diff.conflicts.map((p) => [p.id, p]));
  return { ...added, presets: added.presets.map((p) => replacement.get(p.id) ?? p) };
}

/** Perfiles que se guardan al aplicar la sección de máquinas, y el id nuevo de cada uno que se conservó junto al local. */
export function resolveDevices(plan: DevicesPlan, current: readonly DeviceProfile[], policy: ConflictPolicy): { save: DeviceProfile[]; idMap: Map<string, string> } {
  const save = [...plan.added];
  const idMap = new Map<string, string>();
  if (policy === 'replace') save.push(...plan.replaced);
  if (policy === 'keepBoth') {
    const names = new Set([...current, ...plan.added].map((d) => d.name));
    const ids = new Set([...current, ...plan.added].map((d) => d.id));
    for (const d of plan.replaced) {
      const name = uniqueLabel(d.name, names);
      const id = uniqueDeviceId(name, ids);
      names.add(name);
      ids.add(id);
      idMap.set(d.id, id);
      save.push({ ...structuredClone(d), id, name, is_default: false });
    }
  }
  return { save, idMap };
}

function planPrefs(bundle: UserBundle, current: CurrentState): PrefsPlan | null {
  if (!isObject(bundle.prefs)) return null;
  const change = applyBundle(bundle.prefs, { prefs: current.prefs, shortcuts: current.shortcuts }, current.knownCommands);
  const changedPrefs = PREF_KEYS.filter((k) => !sameValue(change.prefs[k], current.prefs[k])).length;
  return { change, changedPrefs, shortcutsChanged: !sameValue(change.shortcuts, current.shortcuts) };
}

function planTemplatesSection(bundle: UserBundle, current: CurrentState): ImportPlan['templates'] {
  const plan = planTemplates(bundle.templates, current.templates ?? []);
  return plan ? { ...plan, localNames: [...(current.templateNames ?? (current.templates ?? []).map((x) => x.name))] } : null;
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
    cameras: planCameras(bundle.cameras, current.cameras ?? emptyCameraSetup()),
    art: planArt(bundle.art, current.userArt ?? []),
    prefs: planPrefs(bundle, current),
    costs: planCosts(bundle.costs, current.costs ?? sanitizeStored({}, getLanguage()), getLanguage()),
    remnants: planRemnants(bundle.remnants, current.remnants ?? { version: 1, remnants: [] }),
    templates: planTemplatesSection(bundle, current),
    history: planHistory(bundle.history, current.historyIds ?? new Set()),
  };
}

/** Cuántos cambios reales produciría cada sección con la política elegida para los conflictos. */
export function sectionChanges(plan: ImportPlan, policies: PolicyChoice = defaultPolicies()): Record<BundleSection, number> {
  const d = plan.devices;
  const m = plan.materials;
  const p = plan.prefs?.change;
  return {
    devices: d ? d.added.length + (policies.devices === 'skip' ? 0 : d.replaced.length) + (d.activeDevice ? 1 : 0) : 0,
    materials: m ? (policies.materials === 'keepBoth' ? m.added : touched(m.diff, policies.materials)) : 0,
    costs: plan.costs ? costsChanges(plan.costs, policies.costs) : 0,
    remnants: plan.remnants ? remnantsChanges(plan.remnants, policies.remnants) : 0,
    templates: plan.templates ? templatesChanges(plan.templates, policies.templates) : 0,
    cameras: plan.cameras ? camerasChanges(plan.cameras) : 0,
    art: plan.art ? plan.art.items.length : 0,
    prefs: p ? p.appliedPrefs + p.appliedShortcuts : 0,
    history: plan.history ? plan.history.added.length : 0,
  };
}

/** Cuántos elementos chocan por id (mismo id, contenido distinto) en cada sección que admite política. */
export function conflictCounts(plan: ImportPlan): Record<ConflictSection, number> {
  const c = plan.costs;
  return {
    devices: plan.devices?.replaced.length ?? 0,
    materials: plan.materials?.diff.conflicts.length ?? 0,
    costs: c ? c.machines.conflicts.length + c.materials.conflicts.length + (c.sales === 'different' ? 1 : 0) : 0,
    remnants: plan.remnants?.diff.conflicts.length ?? 0,
    templates: plan.templates?.diff.conflicts.length ?? 0,
  };
}

/** Secciones que trae el paquete (aunque no cambien nada). */
export function presentSections(plan: ImportPlan): BundleSection[] {
  return BUNDLE_SECTIONS.filter((s) => plan[s] !== null);
}
