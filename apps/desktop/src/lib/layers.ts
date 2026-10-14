import type { DesignObject, Document, ImageSettings, LayerMode, LayerSettings, TabSettings } from '@/api/types';
import { colorName, isGuideColor } from './palette';

/** Mismos valores por defecto que `ImageSettings::default()` del motor (umbral 0-1, no 0-255). */
export const DEFAULT_IMAGE_SETTINGS: ImageSettings = {
  mode: 'stucki',
  threshold: 0.5,
  brightness: 0,
  contrast: 0,
  gamma: 1,
  invert: false,
  dot_width: 0,
  halftone_cell: 0.5,
  halftone_angle: 45,
  pass_through: false,
  enhance_radius: 0,
  enhance_amount: 0,
  denoise: 0,
};

/** Mismos valores que `TabSettings::default()` del motor. */
export const DEFAULT_TAB_SETTINGS: TabSettings = {
  enabled: false,
  width: 0.5,
  count: 4,
  spacing: 0,
  skip_inner: true,
  power: 0,
};

/** Máximo de operaciones adicionales (sub-capas) de una capa; el mismo que `MAX_SUB_LAYERS` del motor (`place.rs`). */
export const MAX_SUB_LAYERS = 11;

/** Grupos de ajustes anidados de una capa (cada uno es un objeto con sus propios campos). */
export const NESTED_KEYS = ['image', 'tabs', 'perforation', 'lead_in', 'lead_out', 'cut_through', 'dot_mode', 'ramp', 'galvo'] as const;
export type NestedKey = (typeof NESTED_KEYS)[number];

/**
 * Capa con los valores de fábrica (los mismos que `LayerSettings::for_color` del motor). No mira los
 * valores por defecto del usuario: es la base con que se completa una capa de un documento viejo.
 */
export function factoryLayer(color: string, name: string, patch: Partial<LayerSettings> = {}): LayerSettings {
  return {
    color,
    name,
    mode: 'line',
    output: true,
    visible: true,
    speed: 1000,
    power: 50,
    min_power: 0,
    passes: 1,
    air: false,
    interval: 0.1,
    angle: 0,
    crosshatch: false,
    bidirectional: true,
    overscan: 'auto',
    kerf: 0,
    image: { ...DEFAULT_IMAGE_SETTINGS },
    order: null,
    fill_style: 'scan',
    tabs: { ...DEFAULT_TAB_SETTINGS },
    z_offset: 0,
    z_step: 0,
    // Ola 2: mismos valores por defecto que `LayerSettings::for_color` del motor (no cambian nada).
    perforation: { enabled: false, cut_mm: 1, skip_mm: 1 },
    lead_in: { enabled: false, kind: 'straight', length: 1, angle: 45 },
    lead_out: { enabled: false, kind: 'straight', length: 1, angle: 45 },
    overcut: 0,
    start_delay_ms: 0,
    end_delay_ms: 0,
    cut_through: { enabled: false, power: 100, ms: 100 },
    dot_mode: { enabled: false, dwell_ms: 1, spacing_mm: 0.5 },
    ramp: { enabled: false, length_mm: 1, start_pct: 0, outer_only: false },
    fill_grouping: 'all',
    angle_increment: 0,
    sub_layers: [],
    laser2: false,
    // Ola 5: mismos valores que `GalvoLayer::default()` (solo los usan las máquinas galvo).
    galvo: {
      frequency_khz: 30,
      pulse_width_ns: null,
      override_timings: false,
      timing: { laser_on_us: 100, laser_off_us: 100, end_us: 300, polygon_us: 100 },
      jump: { speed: 4000, delay_min_us: 200, delay_max_us: 400, distance_limit: 10 },
      wobble: { enabled: false, size: 0.2, step: 0.05 },
      repeat: 1,
      pause_before_ms: 0,
      pause_after_ms: 0,
      ppi: 0,
      slice3d: { enabled: false, slices: 16, angle_step_deg: 0 },
    },
    ...patch,
  };
}

// ---- valores por defecto del usuario (C35) ------------------------------------------------------

/**
 * Los ajustes de una capa que se copian, se pegan y se guardan como valores por defecto: todo menos
 * lo que identifica a la capa (color, nombre, posición fijada) y su estado (visible, con salida).
 */
export type LayerTemplate = Omit<LayerSettings, 'color' | 'name' | 'order' | 'visible' | 'output'>;

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);

/** Copia independiente de una capa (los objetos anidados no se comparten con la original). */
function cloneLayer<T extends LayerSettings | LayerTemplate>(l: T): T {
  const out = { ...l } as Record<string, unknown>;
  for (const k of NESTED_KEYS) if (isObject(out[k])) out[k] = { ...(out[k] as object) };
  out.sub_layers = (l.sub_layers ?? []).map((s) => cloneLayer(s));
  return out as T;
}

/** Los ajustes copiables de una capa. */
export function layerTemplate(layer: LayerSettings): LayerTemplate {
  const { color: _c, name: _n, order: _o, visible: _v, output: _out, ...rest } = cloneLayer(normalizeLayer(layer));
  return rest;
}

/** La capa con los ajustes de `template` encima; conserva su color, nombre, posición, visibilidad y salida. */
export function applyTemplate(layer: LayerSettings, template: LayerTemplate): LayerSettings {
  const copy = cloneLayer(template);
  return { ...layer, ...copy, sub_layers: copy.sub_layers.map((s) => ({ ...s, color: layer.color })) };
}

/** Valores por defecto del usuario; `null` = los de fábrica. Los fija `features/layers/defaults.ts` desde las preferencias. */
let userDefaults: LayerTemplate | null = null;

export function setLayerDefaults(template: LayerTemplate | null): void {
  userDefaults = template;
}

export function getLayerDefaults(): LayerTemplate | null {
  return userDefaults;
}

/** Capa nueva: valores de fábrica, encima los valores por defecto del usuario y, al final, `patch`. */
export function makeLayer(color: string, name: string, patch: Partial<LayerSettings> = {}): LayerSettings {
  const base = factoryLayer(color, name);
  return { ...(userDefaults ? applyTemplate(base, userDefaults) : base), ...patch };
}

/** Valores válidos de los campos de texto con opciones, por ruta. */
const ENUMS: Record<string, readonly string[]> = {
  mode: ['line', 'fill', 'image'],
  fill_style: ['scan', 'offset', 'flood'],
  fill_grouping: ['all', 'by_shape', 'by_group'],
  'image.mode': ['threshold', 'floyd', 'jarvis', 'stucki', 'atkinson', 'bayer4', 'grayscale', 'halftone', 'newsprint', 'sketch'],
  'lead_in.kind': ['straight', 'arc'],
  'lead_out.kind': ['straight', 'arc'],
};

/** Mezcla `raw` sobre `base` conservando el tipo de cada campo de `base`; lo que no encaja se descarta. */
function mergeTyped(base: Record<string, unknown>, raw: Record<string, unknown>, path = ''): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [key, def] of Object.entries(base)) {
    const at = path ? `${path}.${key}` : key;
    const v = raw[key];
    if (key === 'sub_layers') {
      const list = Array.isArray(v) ? v : [];
      out[key] = list
        .slice(0, MAX_SUB_LAYERS)
        .filter(isObject)
        .map((s) => ({ ...mergeTyped(factoryLayer('#000000', '') as unknown as Record<string, unknown>, s), sub_layers: [] }));
    } else if (key === 'overscan') {
      out[key] = v === 'auto' || (typeof v === 'number' && Number.isFinite(v) && v >= 0) ? v : def;
    } else if (key === 'order') {
      out[key] = null;
    } else if (isObject(def)) {
      out[key] = mergeTyped(def, isObject(v) ? v : {}, at);
    } else if (typeof def === 'number') {
      out[key] = typeof v === 'number' && Number.isFinite(v) ? v : def;
    } else if (typeof def === 'boolean') {
      out[key] = typeof v === 'boolean' ? v : def;
    } else if (typeof def === 'string') {
      const allowed = ENUMS[at];
      out[key] = typeof v === 'string' && (!allowed || allowed.includes(v)) ? v : def;
    } else {
      out[key] = def;
    }
  }
  return out;
}

/**
 * Valida unos valores por defecto leídos del disco o de un paquete: lo que falta o no es del tipo
 * esperado toma el valor de fábrica. `null` si no es un objeto.
 */
export function sanitizeLayerTemplate(raw: unknown): LayerTemplate | null {
  if (!isObject(raw)) return null;
  const merged = mergeTyped(factoryLayer('#000000', '') as unknown as Record<string, unknown>, raw) as unknown as LayerSettings;
  return layerTemplate(merged);
}

// ---- sub-capas (operaciones adicionales) ---------------------------------------------------------

/** Operación adicional nueva: valores de fábrica en `mode`, con la velocidad, potencia e intervalo de la capa. */
export function newSubLayer(parent: LayerSettings, mode: LayerMode): LayerSettings {
  return factoryLayer(parent.color, '', { mode, speed: parent.speed, power: parent.power, interval: parent.interval, air: parent.air });
}

/**
 * Completa una capa que llega sin los campos nuevos (documentos viejos, perfiles guardados antes
 * de la Fase 4) con los valores por defecto. Devuelve el mismo objeto si ya estaba completa.
 */
export function normalizeLayer(layer: Partial<LayerSettings> & { color: string }): LayerSettings {
  const base = factoryLayer(layer.color, layer.name ?? '');
  const merged = { ...base, ...layer } as Record<string, unknown>;
  let complete = (Object.keys(base) as (keyof LayerSettings)[]).every((k) => k in layer);
  for (const k of NESTED_KEYS) {
    const given = (layer as Record<string, unknown>)[k];
    const def = base[k] as unknown as Record<string, unknown>;
    merged[k] = { ...def, ...(isObject(given) ? given : {}) };
    if (!isObject(given) || !Object.keys(def).every((f) => f in given)) complete = false;
  }
  const subs = layer.sub_layers ?? [];
  const normalSubs = subs.map((s) => normalizeLayer(s));
  if (normalSubs.some((s, i) => s !== subs[i])) {
    complete = false;
    merged.sub_layers = normalSubs;
  }
  return complete ? (layer as LayerSettings) : (merged as unknown as LayerSettings);
}

/** Documento con todas sus capas completas; el mismo objeto si no hacía falta tocar ninguna. */
export function normalizeLayers(doc: Document): Document {
  const layers = doc.layers.map((l) => normalizeLayer(l));
  return layers.every((l, i) => l === doc.layers[i]) ? doc : { ...doc, layers };
}

/** La capa trae ajustes de Z distintos de 0 (que la máquina sin eje Z ignora). */
export function usesZ(layer: Pick<LayerSettings, 'z_offset' | 'z_step'>): boolean {
  return (layer.z_offset ?? 0) !== 0 || (layer.z_step ?? 0) !== 0;
}

/** Rasgos de una capa que conviene ver de un vistazo en la lista. */
export type LayerFeature = 'tabs' | 'offset' | 'z';

/** Puentes activos (solo en line), relleno por contornos (solo en fill) y Z (si la máquina lo tiene). */
export function layerFeatures(layer: LayerSettings, machineHasZ: boolean): LayerFeature[] {
  const out: LayerFeature[] = [];
  if (layer.mode === 'line' && layer.tabs?.enabled) out.push('tabs');
  if (layer.mode === 'fill' && layer.fill_style === 'offset') out.push('offset');
  if (machineHasZ && usesZ(layer)) out.push('z');
  return out;
}

/** Capa nueva para un color: con el nombre de la paleta y, si es la de guías, sin salida. */
export function newLayerFor(color: string): LayerSettings {
  return makeLayer(color, colorName(color), isGuideColor(color) ? { output: false } : {});
}

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** Devuelve el documento con la capa de ese color; el mismo objeto si ya existía. */
export function ensureLayerIn(doc: Document, color: string): Document {
  if (doc.layers.some((l) => same(l.color, color))) return doc;
  return { ...doc, layers: [...doc.layers, newLayerFor(color)] };
}

/**
 * Pasa los objetos (no bloqueados) a la capa de `color`, creándola si hace falta.
 * Devuelve el mismo documento si no cambia nada.
 */
export function assignObjectsToLayer(doc: Document, ids: readonly string[], color: string): Document {
  const set = new Set(ids);
  const moves = (o: DesignObject) => set.has(o.id) && !o.locked && o.layer !== color;
  if (!doc.objects.some(moves)) return doc;
  const withLayer = ensureLayerIn(doc, color);
  return { ...withLayer, objects: withLayer.objects.map((o) => (moves(o) ? { ...o, layer: color } : o)) };
}

/** Cuántos objetos hay en una capa y cuántos de ellos salen a la máquina si la capa tiene salida. */
export interface LayerCount {
  total: number;
  /** No ocultos y de un tipo que el modo de la capa admite (vector en line/fill, imagen en image). */
  live: number;
}

/** Conteos por color de capa (en minúsculas). */
const countMemo = new WeakMap<object, { layers: Document['layers']; out: Map<string, LayerCount> }>();

/** Se guarda por lista de objetos y de capas (los paneles lo piden en cada actualización del store). No hay que modificar el resultado. */
export function countByLayer(doc: Document): Map<string, LayerCount> {
  const hit = countMemo.get(doc.objects);
  if (hit && hit.layers === doc.layers) return hit.out;
  const out = countByLayerRaw(doc);
  countMemo.set(doc.objects, { layers: doc.layers, out });
  return out;
}

function countByLayerRaw(doc: Document): Map<string, LayerCount> {
  const mode = new Map(doc.layers.map((l) => [l.color.toLowerCase(), l.mode]));
  const out = new Map<string, LayerCount>();
  for (const o of doc.objects) {
    const key = o.layer.toLowerCase();
    const c = out.get(key) ?? { total: 0, live: 0 };
    c.total += 1;
    const m = mode.get(key);
    const compatible = m === undefined ? true : (m === 'image') === (o.kind.type === 'image');
    if (!o.hidden && compatible) c.live += 1;
    out.set(key, c);
  }
  return out;
}

// ---- intervalo <-> DPI --------------------------------------------------------------------------

const MM_PER_INCH = 25.4;

/** Líneas por pulgada que equivalen a un intervalo en mm. */
export function intervalToDpi(intervalMm: number): number {
  return MM_PER_INCH / intervalMm;
}

/** Intervalo en mm (redondeado a 6 decimales) que equivale a un DPI. */
export function dpiToInterval(dpi: number): number {
  return Math.round((MM_PER_INCH / dpi) * 1e6) / 1e6;
}
