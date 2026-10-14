import type { ImageSettings } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { readPref, writePref } from '@/lib/platform';

/**
 * Presets de «Ajustar imagen» (C51): combinaciones de brillo, contraste, gamma, realce de bordes y
 * quitar motas que se eligen de una lista. No incluyen el tramado ni el intervalo: esos dependen de
 * la máquina y del material, no de la foto. Los valores caben en los límites del motor
 * (`ImageAdjust::validate`: brillo y contraste de -1 a 1, gamma mayor que 0, quitar motas hasta 16).
 */

export const PRESET_KEYS = ['brightness', 'contrast', 'gamma', 'enhance_radius', 'enhance_amount', 'denoise'] as const;
export type PresetKey = (typeof PRESET_KEYS)[number];
export type PresetValues = Pick<ImageSettings, PresetKey>;

/** Sin ajustes: lo que tiene una capa de imagen recién creada. */
export const NEUTRAL: PresetValues = { brightness: 0, contrast: 0, gamma: 1, enhance_radius: 0, enhance_amount: 0, denoise: 0 };

export interface FactoryPreset {
  id: string;
  name: I18nKey;
  hint: I18nKey;
  values: PresetValues;
}

/** Los presets de fábrica. Gamma menor que 1 aclara los medios tonos (menos potencia en las sombras suaves). */
export const FACTORY_PRESETS: readonly FactoryPreset[] = [
  { id: 'neutral', name: 'layers.preset.neutral', hint: 'layers.preset.neutral.hint', values: NEUTRAL },
  {
    id: 'photo-wood',
    name: 'layers.preset.photoWood',
    hint: 'layers.preset.photoWood.hint',
    values: { brightness: 0.05, contrast: 0.25, gamma: 0.85, enhance_radius: 1.5, enhance_amount: 0.5, denoise: 0 },
  },
  {
    id: 'logo-contrast',
    name: 'layers.preset.logo',
    hint: 'layers.preset.logo.hint',
    values: { brightness: 0, contrast: 0.8, gamma: 1, enhance_radius: 1, enhance_amount: 0.3, denoise: 2 },
  },
  {
    id: 'portrait-soft',
    name: 'layers.preset.portrait',
    hint: 'layers.preset.portrait.hint',
    values: { brightness: 0.1, contrast: 0.1, gamma: 0.9, enhance_radius: 1, enhance_amount: 0.3, denoise: 1 },
  },
  {
    id: 'dark-photo',
    name: 'layers.preset.dark',
    hint: 'layers.preset.dark.hint',
    values: { brightness: 0.25, contrast: 0.15, gamma: 0.7, enhance_radius: 0, enhance_amount: 0, denoise: 0 },
  },
  {
    id: 'dirty-scan',
    name: 'layers.preset.scan',
    hint: 'layers.preset.scan.hint',
    values: { brightness: 0.05, contrast: 0.4, gamma: 1, enhance_radius: 0, enhance_amount: 0, denoise: 3 },
  },
  {
    id: 'fine-detail',
    name: 'layers.preset.detail',
    hint: 'layers.preset.detail.hint',
    values: { brightness: 0, contrast: 0.15, gamma: 1, enhance_radius: 2, enhance_amount: 0.8, denoise: 0 },
  },
];

export interface UserPreset {
  id: string;
  name: string;
  values: PresetValues;
}

/** Largo máximo del nombre de un preset propio. */
export const MAX_NAME_LENGTH = 40;
/** Máximo de presets propios (la lista es un menú, no una biblioteca). */
export const MAX_USER_PRESETS = 50;
const STORAGE_KEY = 'imagePresets';
const MAX_DENOISE = 16;

/** Las seis cifras de una capa. */
export function valuesOf(image: ImageSettings): PresetValues {
  return {
    brightness: image.brightness,
    contrast: image.contrast,
    gamma: image.gamma,
    enhance_radius: image.enhance_radius,
    enhance_amount: image.enhance_amount,
    denoise: image.denoise,
  };
}

/** ¿Los dos juegos de valores son el mismo (con la tolerancia de un campo numérico)? */
export function sameValues(a: PresetValues, b: PresetValues): boolean {
  return PRESET_KEYS.every((k) => Math.abs(a[k] - b[k]) < 1e-6);
}

/** Los valores de un preset sobre los ajustes de la capa: el resto (tramado, umbral…) no se toca. */
export function applyValues(image: ImageSettings, values: PresetValues): ImageSettings {
  return { ...image, ...values };
}

/** ¿Los valores son aceptables para el motor? Mismos límites que `ImageAdjust::validate`. */
export function validValues(v: PresetValues): boolean {
  const finite = PRESET_KEYS.every((k) => Number.isFinite(v[k]));
  return (
    finite &&
    v.brightness >= -1 &&
    v.brightness <= 1 &&
    v.contrast >= -1 &&
    v.contrast <= 1 &&
    v.gamma > 0 &&
    v.enhance_radius >= 0 &&
    v.enhance_amount >= 0 &&
    Number.isInteger(v.denoise) &&
    v.denoise >= 0 &&
    v.denoise <= MAX_DENOISE
  );
}

/** El id del preset (de fábrica o propio) que coincide con `values`, o `null` si es una mezcla propia. */
export function matchPreset(values: PresetValues, user: readonly UserPreset[]): string | null {
  const f = FACTORY_PRESETS.find((p) => sameValues(p.values, values));
  if (f) return f.id;
  return user.find((p) => sameValues(p.values, values))?.id ?? null;
}

/** Nombre limpio para un preset propio, o `null` si no sirve (vacío). Se recorta a `MAX_NAME_LENGTH`. */
export function cleanName(raw: string): string | null {
  const name = raw.trim().replace(/\s+/g, ' ').slice(0, MAX_NAME_LENGTH).trim();
  return name === '' ? null : name;
}

/**
 * Agrega un preset propio. Si ya hay uno con ese nombre (sin distinguir mayúsculas) lo reemplaza,
 * conservando su id. Devuelve la lista nueva, o `null` si el nombre o los valores no sirven o la
 * lista está llena.
 */
export function withPreset(list: readonly UserPreset[], rawName: string, values: PresetValues, newId: () => string): UserPreset[] | null {
  const name = cleanName(rawName);
  if (!name || !validValues(values)) return null;
  const same = list.find((p) => p.name.toLowerCase() === name.toLowerCase());
  if (same) return list.map((p) => (p === same ? { ...p, name, values } : p));
  if (list.length >= MAX_USER_PRESETS) return null;
  return [...list, { id: newId(), name, values }];
}

export function withoutPreset(list: readonly UserPreset[], id: string): UserPreset[] {
  return list.filter((p) => p.id !== id);
}

/** Lee la lista guardada, descartando lo que no tenga forma de preset. */
export function parseUserPresets(raw: string | null): UserPreset[] {
  if (!raw) return [];
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch {
    return [];
  }
  if (!Array.isArray(data)) return [];
  const out: UserPreset[] = [];
  for (const item of data) {
    if (typeof item !== 'object' || item === null) continue;
    const { id, name, values } = item as Record<string, unknown>;
    if (typeof id !== 'string' || typeof name !== 'string' || typeof values !== 'object' || values === null) continue;
    const v = values as Record<string, unknown>;
    if (!PRESET_KEYS.every((k) => typeof v[k] === 'number')) continue;
    const clean = cleanName(name);
    const vals = valuesOf(v as unknown as ImageSettings);
    if (clean && validValues(vals) && !out.some((p) => p.id === id)) out.push({ id, name: clean, values: vals });
  }
  return out.slice(0, MAX_USER_PRESETS);
}

export function loadUserPresets(): UserPreset[] {
  return parseUserPresets(readPref(STORAGE_KEY));
}

export function saveUserPresets(list: readonly UserPreset[]): void {
  writePref(STORAGE_KEY, JSON.stringify(list));
}

/** Id para un preset propio nuevo. */
export function newPresetId(): string {
  return `user-${Date.now().toString(36)}-${Math.floor(Math.random() * 1e6).toString(36)}`;
}
