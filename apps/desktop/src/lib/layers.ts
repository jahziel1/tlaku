import type { DesignObject, Document, ImageSettings, LayerSettings, TabSettings } from '@/api/types';
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
};

/** Mismos valores que `TabSettings::default()` del motor. */
export const DEFAULT_TAB_SETTINGS: TabSettings = {
  enabled: false,
  width: 0.5,
  count: 4,
  spacing: 0,
  skip_inner: true,
};

export function makeLayer(color: string, name: string, patch: Partial<LayerSettings> = {}): LayerSettings {
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
    ...patch,
  };
}

/**
 * Completa una capa que llega sin los campos nuevos (documentos viejos, perfiles guardados antes
 * de la Fase 4) con los valores por defecto. Devuelve el mismo objeto si ya estaba completa.
 */
export function normalizeLayer(layer: Partial<LayerSettings> & { color: string }): LayerSettings {
  const base = makeLayer(layer.color, layer.name ?? '');
  const image = { ...base.image, ...layer.image };
  const tabs = { ...base.tabs, ...layer.tabs };
  const merged: LayerSettings = { ...base, ...layer, image, tabs };
  const complete =
    (Object.keys(base) as (keyof LayerSettings)[]).every((k) => k in layer) &&
    Object.keys(image).every((k) => k in (layer.image ?? {})) &&
    Object.keys(tabs).every((k) => k in (layer.tabs ?? {}));
  return complete ? (layer as LayerSettings) : merged;
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
export function countByLayer(doc: Document): Map<string, LayerCount> {
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
