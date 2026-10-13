/**
 * Lógica pura de la biblioteca de materiales (sin interfaz ni API): agrupar, filtrar, aplicar un
 * ajuste a una capa, crear ajustes desde una capa y mezclar bibliotecas al importar.
 */
import type { LayerSettings, MaterialLibrary, MaterialPreset } from '@/api/types';
import { normalizeLayer } from '@/lib/layers';
import { locale } from '@/i18n';

/** Versión del archivo de biblioteca (`MATERIAL_LIBRARY_VERSION` del motor). */
export const LIBRARY_VERSION = 1;

export function emptyLibrary(): MaterialLibrary {
  return { version: LIBRARY_VERSION, presets: [] };
}

// ---------------------------------------------------------------------------
// Aplicar a una capa
// ---------------------------------------------------------------------------

/** Campos de la capa que un ajuste NO toca: identidad y estado de la capa en el documento. */
export const PRESET_SKIPPED_FIELDS = ['color', 'name', 'output', 'visible', 'order'] as const;
type Skipped = (typeof PRESET_SKIPPED_FIELDS)[number];

export type PresetPatch = Omit<LayerSettings, Skipped>;

/** Todo lo que se copia a la capa al aplicar el ajuste (los campos viejos se completan con los de fábrica). */
export function presetPatch(preset: Pick<MaterialPreset, 'settings'>): PresetPatch {
  const full: Partial<LayerSettings> = { ...normalizeLayer(preset.settings) };
  for (const k of PRESET_SKIPPED_FIELDS) delete full[k];
  return full as PresetPatch;
}

/** Comparación profunda de valores de JSON (los ajustes solo tienen números, textos, booleanos y objetos). */
export function sameValue(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true;
  if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false;
  const ka = Object.keys(a);
  const kb = Object.keys(b);
  if (ka.length !== kb.length) return false;
  return ka.every((k) => sameValue((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]));
}

/** `true` si la capa ya tiene exactamente los ajustes del material. */
export function layerMatchesPreset(layer: LayerSettings, preset: MaterialPreset): boolean {
  return sameValue(presetPatch({ settings: layer }), presetPatch(preset));
}

/** Capas del documento cuyos ajustes coinciden con los del material. */
export function matchingLayers(layers: readonly LayerSettings[], preset: MaterialPreset): LayerSettings[] {
  return layers.filter((l) => layerMatchesPreset(l, preset));
}

/** Capa con el ajuste aplicado: la misma referencia si no cambia nada. */
export function applyPresetToLayer(layer: LayerSettings, preset: MaterialPreset): LayerSettings {
  const next = { ...normalizeLayer(layer), ...presetPatch(preset) };
  return sameValue(next, layer) ? layer : next;
}

/** Cambio de documento (para `applyDocChange`) que aplica el ajuste a la capa `color`. */
export function applyPresetChange<D extends { layers: LayerSettings[] }>(doc: D, color: string, preset: MaterialPreset): D {
  let changed = false;
  const layers = doc.layers.map((l) => {
    if (l.color.toLowerCase() !== color.toLowerCase()) return l;
    const next = applyPresetToLayer(l, preset);
    if (next !== l) changed = true;
    return next;
  });
  return changed ? { ...doc, layers } : doc;
}

// ---------------------------------------------------------------------------
// Crear y editar
// ---------------------------------------------------------------------------

/** Datos que el usuario escribe al guardar o editar un ajuste. */
export interface PresetForm {
  material: string;
  /** `null` = no aplica (grabados). */
  thickness: number | null;
  title: string;
  notes: string;
  device_id: string | null;
}

/** Ajuste nuevo a partir de una capa; los ajustes se copian completos (también color y nombre, que al aplicar se ignoran). */
export function presetFromLayer(layer: LayerSettings, form: PresetForm, id: string): MaterialPreset {
  return {
    id,
    material: form.material.trim(),
    thickness: form.thickness,
    title: form.title.trim(),
    notes: form.notes.trim(),
    device_id: form.device_id,
    settings: structuredClone(normalizeLayer(layer)),
  };
}

/** Motivo por el que el formulario no se puede guardar, o `null`. Devuelve claves de i18n. */
export function formProblem(form: PresetForm): 'materials.form.needMaterial' | 'materials.form.needTitle' | 'materials.form.badThickness' | null {
  if (form.material.trim() === '') return 'materials.form.needMaterial';
  if (form.title.trim() === '') return 'materials.form.needTitle';
  if (form.thickness !== null && (!Number.isFinite(form.thickness) || form.thickness <= 0)) return 'materials.form.badThickness';
  return null;
}

/** Un id que no está en `taken`. `rand` se inyecta en las pruebas. */
export function uniqueId(taken: ReadonlySet<string>, rand: () => number = Math.random): string {
  for (;;) {
    const id = `mat-${Date.now().toString(36)}-${Math.floor(rand() * 36 ** 4).toString(36).padStart(4, '0')}`;
    if (!taken.has(id)) return id;
  }
}

export const idsOf = (presets: readonly MaterialPreset[]): Set<string> => new Set(presets.map((p) => p.id));

/** Copia de un ajuste con id nuevo; el título lleva el sufijo `suffix` para distinguirla. */
export function duplicatePreset(preset: MaterialPreset, id: string, suffix: string): MaterialPreset {
  return { ...structuredClone(preset), id, title: `${preset.title} ${suffix}`.trim() };
}

// ---------------------------------------------------------------------------
// Mezclar al importar
// ---------------------------------------------------------------------------

export interface MergeResult {
  library: MaterialLibrary;
  /** Ajustes nuevos que entraron (con su id o con uno nuevo). */
  added: number;
  /** De esos, cuántos chocaban de id con uno distinto y se copiaron con id nuevo. */
  renamed: number;
  /** Ajustes idénticos a uno que ya estaba (mismo contenido): no se vuelven a agregar. */
  skipped: number;
}

/**
 * Agrega a `current` los ajustes de `incoming`. Un id que ya existe con contenido distinto no
 * pisa al existente: el ajuste entra como copia con un id nuevo. Si es idéntico a uno que ya está
 * (aunque tenga otro id), se omite, así importar dos veces el mismo archivo no duplica nada.
 */
export function mergeLibraries(current: MaterialLibrary, incoming: MaterialLibrary, rand: () => number = Math.random): MergeResult {
  const byId = new Map(current.presets.map((p) => [p.id, p]));
  const taken = idsOf(current.presets);
  const presets = [...current.presets];
  let added = 0;
  let renamed = 0;
  let skipped = 0;
  const withoutId = (p: MaterialPreset) => ({ ...p, id: '' });
  for (const p of incoming.presets) {
    const existing = byId.get(p.id);
    // Idéntico (salvo el id) a uno que ya está: es el mismo ajuste, no se vuelve a agregar.
    if (presets.some((q) => sameValue(withoutId(q), withoutId(p)))) {
      skipped++;
    } else if (!existing) {
      presets.push(p);
      taken.add(p.id);
      byId.set(p.id, p);
      added++;
    } else {
      const id = uniqueId(taken, rand);
      taken.add(id);
      presets.push({ ...p, id });
      added++;
      renamed++;
    }
  }
  return { library: { version: LIBRARY_VERSION, presets }, added, renamed, skipped };
}

// ---------------------------------------------------------------------------
// Agrupar y filtrar
// ---------------------------------------------------------------------------

export interface ThicknessGroup {
  /** `null` = sin grosor. */
  thickness: number | null;
  presets: MaterialPreset[];
}

export interface MaterialGroup {
  /** Nombre tal como lo escribió el usuario (el del primer ajuste del grupo). */
  material: string;
  thicknesses: ThicknessGroup[];
}

/** Minúsculas y sin acentos, para comparar y buscar. */
export function fold(text: string): string {
  return text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase().trim();
}

const collators = new Map<string, Intl.Collator>();
/** Comparador alfabético en el idioma de la interfaz (se crea una vez por idioma). */
function collator(): Intl.Collator {
  const loc = locale();
  let c = collators.get(loc);
  if (!c) collators.set(loc, (c = new Intl.Collator(loc, { sensitivity: 'base', numeric: true })));
  return c;
}

/** Grosor en el texto de la interfaz: "3 mm", "0,5 mm" o "". */
export function thicknessLabel(thickness: number | null, unit = 'mm'): string {
  if (thickness === null) return '';
  return `${new Intl.NumberFormat(locale(), { maximumFractionDigits: 3 }).format(thickness)} ${unit}`;
}

/**
 * Agrupa por material (sin distinguir mayúsculas ni acentos) y dentro de cada uno por grosor.
 * Materiales por orden alfabético; grosores de menor a mayor, con "sin grosor" al final; ajustes
 * por título.
 */
export function groupPresets(presets: readonly MaterialPreset[]): MaterialGroup[] {
  const groups = new Map<string, { material: string; byThickness: Map<number | null, MaterialPreset[]> }>();
  for (const p of presets) {
    const key = fold(p.material);
    let g = groups.get(key);
    if (!g) {
      g = { material: p.material.trim(), byThickness: new Map() };
      groups.set(key, g);
    }
    const list = g.byThickness.get(p.thickness) ?? [];
    list.push(p);
    g.byThickness.set(p.thickness, list);
  }
  return [...groups.values()]
    .sort((a, b) => collator().compare(a.material, b.material))
    .map((g) => ({
      material: g.material,
      thicknesses: [...g.byThickness.entries()]
        .sort(([a], [b]) => (a === null ? 1 : b === null ? -1 : a - b))
        .map(([thickness, list]) => ({ thickness, presets: [...list].sort((a, b) => collator().compare(a.title, b.title)) })),
    }));
}

export interface PresetFilter {
  search: string;
  /** Solo los ajustes de esta máquina y los que valen para cualquiera. `null` = sin filtro. */
  deviceId: string | null;
}

/** Ajustes que cumplen la búsqueda (material, título, notas o grosor) y el filtro de máquina. */
export function filterPresets(presets: readonly MaterialPreset[], filter: PresetFilter): MaterialPreset[] {
  const words = fold(filter.search).split(/\s+/).filter(Boolean);
  return presets.filter((p) => {
    if (filter.deviceId !== null && p.device_id !== null && p.device_id !== filter.deviceId) return false;
    if (words.length === 0) return true;
    const hay = fold(`${p.material} ${p.title} ${p.notes} ${p.thickness ?? ''} ${thicknessLabel(p.thickness)}`);
    return words.every((w) => hay.includes(w));
  });
}

/** Resumen corto de los ajustes para la fila: "1000 mm/min · 60 % · ×2 · 0,1 mm". */
export function settingsSummary(s: LayerSettings): string {
  const parts = [`${s.speed} mm/min`, `${s.power} %`];
  if (s.passes > 1) parts.push(`×${s.passes}`);
  if (s.mode !== 'line') parts.push(`${s.interval} mm`);
  return parts.join(' · ');
}
