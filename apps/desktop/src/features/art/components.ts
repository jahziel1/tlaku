import type { DesignObject, Document, ImportResult, LayerSettings, Point } from '@/api/types';
import { objectsToSvg } from '@/features/canvas/svgClip';
import { multiplyAffine, translation, unionBounds } from '@/lib/geometry';
import { groupIdsOf, regroupCopies } from '@/lib/groups';
import { objectBoundsWithCopies } from '@/lib/virtualArray';
import type { ComponentData, UserComponent } from './types';

/**
 * Componentes guardados (issue #22): lógica pura. Un componente es una selección con sus capas y
 * ajustes de corte, en el formato propio de Tlaku (los mismos `DesignObject` y `LayerSettings` que
 * el portapapeles interno y el proyecto), más una miniatura y la caja original.
 */

export const COMPONENT_FORMAT = 'tlaku-component';

/** Largo máximo de la miniatura; una hoja con miles de trayectos pasa a cajas para no duplicar el archivo. */
const MAX_THUMB_CHARS = 120_000;

/** Largo máximo del nombre (el del archivo). */
export const MAX_COMPONENT_NAME = 80;

/** Dónde cae el componente al insertarlo. */
export type Placement = 'auto' | 'original' | 'center';

export interface ComponentSize {
  width: number;
  height: number;
}

type Bounds = [Point, Point];

const boundsOf = (objects: readonly DesignObject[]): Bounds | null => unionBounds(objects.map(objectBoundsWithCopies));

const num = (n: number): string => String(Math.round(n * 100) / 100 + 0);

/** Miniatura simple: la caja de cada objeto. Para componentes demasiado pesados para dibujarlos enteros. */
function boxesSvg(objects: readonly DesignObject[], layers: readonly LayerSettings[], box: Bounds): string {
  const w = box[1][0] - box[0][0];
  const h = box[1][1] - box[0][1];
  const rects = objects
    .filter((o) => !o.hidden)
    .flatMap((o) => {
      const b = objectBoundsWithCopies(o);
      if (!b) return [];
      const color = layers.find((l) => l.color.toLowerCase() === o.layer.toLowerCase())?.color ?? '#000000';
      return [`<rect x="${num(b[0][0] - box[0][0])}" y="${num(box[1][1] - b[1][1])}" width="${num(b[1][0] - b[0][0])}" height="${num(b[1][1] - b[0][1])}" fill="none" stroke="${color}" stroke-width="0.1"/>`];
    });
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${num(w)}mm" height="${num(h)}mm" viewBox="0 0 ${num(w)} ${num(h)}">${rects.join('')}</svg>`;
}

/**
 * El componente de `objects` con las capas que usan (cada capa con sus ajustes). `null` si no hay
 * objetos con área.
 */
export function buildComponent(objects: readonly DesignObject[], docLayers: readonly LayerSettings[], now: Date = new Date()): ComponentData | null {
  const bounds = boundsOf(objects);
  if (!bounds || objects.length === 0) return null;
  const used = new Set(objects.map((o) => o.layer.toLowerCase()));
  const layers = docLayers.filter((l) => used.has(l.color.toLowerCase()));
  let thumb = objectsToSvg(objects, layers) ?? '';
  if (thumb.length > MAX_THUMB_CHARS) thumb = boxesSvg(objects, layers, bounds);
  return {
    format: COMPONENT_FORMAT,
    version: 1,
    created: now.toISOString(),
    // Copia: el documento sigue su vida y el componente no debe cambiar con él.
    objects: structuredClone(objects) as DesignObject[],
    layers: structuredClone(layers) as LayerSettings[],
    bounds,
    thumb,
  };
}

/** Ancho y alto del componente, en mm. */
export function componentSize(data: ComponentData): ComponentSize {
  const [[x0, y0], [x1, y1]] = data.bounds;
  return { width: x1 - x0, height: y1 - y0 };
}

/** Texto corto del tamaño: `600 × 600 mm`. */
export function sizeLabel(data: ComponentData): string {
  const { width, height } = componentSize(data);
  const f = (n: number) => String(Math.round(n * 10) / 10);
  return `${f(width)} × ${f(height)} mm`;
}

/** ¿La caja original cabe entera en la cama (de `[0, 0]` a `bed`)? */
export function fitsBed(bounds: Bounds, bed: readonly [number, number]): boolean {
  const eps = 1e-6;
  return bounds[0][0] >= -eps && bounds[0][1] >= -eps && bounds[1][0] <= bed[0] + eps && bounds[1][1] <= bed[1] + eps;
}

/** Forma básica de un componente leído del disco o de un paquete. */
export function isComponentData(raw: unknown): raw is ComponentData {
  const d = raw as Partial<ComponentData> | null;
  return (
    !!d &&
    typeof d === 'object' &&
    d.format === COMPONENT_FORMAT &&
    Array.isArray(d.objects) &&
    d.objects.length > 0 &&
    Array.isArray(d.layers) &&
    Array.isArray(d.bounds) &&
    d.bounds.length === 2 &&
    d.bounds.every((p) => Array.isArray(p) && p.length === 2 && p.every((n) => typeof n === 'number' && Number.isFinite(n)))
  );
}

/** JSON con las claves ordenadas, para comparar ajustes sin depender del orden. */
function stable(v: unknown): string {
  if (Array.isArray(v)) return `[${v.map(stable).join(',')}]`;
  if (v && typeof v === 'object') {
    const o = v as Record<string, unknown>;
    return `{${Object.keys(o).sort().map((k) => `${JSON.stringify(k)}:${stable(o[k])}`).join(',')}}`;
  }
  return JSON.stringify(v) ?? 'null';
}

/** Lo de una capa que cambia cómo se corta o graba; el nombre y la visibilidad no cuentan. */
const cutSettings = ({ name: _name, visible: _visible, ...rest }: LayerSettings): string => stable(rest);

/**
 * Colores de capa del componente que el documento ya tiene con ajustes de corte distintos. Esas
 * capas conservan los del documento (como al pegar entre pestañas): el componente se adapta al
 * proyecto, no al revés.
 */
export function layerConflicts(componentLayers: readonly LayerSettings[], docLayers: readonly LayerSettings[]): string[] {
  const mine = new Map(docLayers.map((l) => [l.color.toLowerCase(), l]));
  return componentLayers.filter((l) => {
    const existing = mine.get(l.color.toLowerCase());
    return existing !== undefined && cutSettings(existing) !== cutSettings(l);
  }).map((l) => l.color);
}

export interface InsertPlan {
  /** Lo que se importa (una sola operación de deshacer). */
  result: ImportResult;
  /** Colores cuyas capas ya existían con otros ajustes (se conservan los del documento). */
  conflicts: string[];
  /** Dónde cayó realmente: `original` o `center`. */
  placedAt: 'original' | 'center';
  /** Con `original`: la caja se sale de la cama. */
  outsideBed: boolean;
}

/**
 * Prepara la inserción. `auto` usa la posición original si cabe en la cama y, si no, el centro de
 * la vista; `original` la respeta siempre (avisa si se sale); `center` centra en `center`.
 */
export function planInsert(data: ComponentData, doc: Document, opts: { placement: Placement; center: Point; bed: readonly [number, number] }): InsertPlan {
  const fits = fitsBed(data.bounds, opts.bed);
  const placedAt = opts.placement === 'center' || (opts.placement === 'auto' && !fits) ? 'center' : 'original';
  const mid: Point = [(data.bounds[0][0] + data.bounds[1][0]) / 2, (data.bounds[0][1] + data.bounds[1][1]) / 2];
  const shift = placedAt === 'center' ? translation(opts.center[0] - mid[0], opts.center[1] - mid[1]) : null;
  const moved = data.objects.map((o): DesignObject => ({
    ...structuredClone(o),
    locked: false,
    transform: shift ? multiplyAffine(shift, o.transform) : o.transform,
  }));
  // Los grupos del componente pasan a grupos nuevos, para no unirse a los del documento.
  const objects = regroupCopies(moved, groupIdsOf(doc.objects));
  const docColors = new Set(doc.layers.map((l) => l.color.toLowerCase()));
  return {
    result: { objects, new_layers: structuredClone(data.layers.filter((l) => !docColors.has(l.color.toLowerCase()))) as LayerSettings[], warnings: [] },
    conflicts: layerConflicts(data.layers, doc.layers),
    placedAt,
    outsideBed: placedAt === 'original' && !fits,
  };
}

/** Los componentes de un paquete que son válidos (se descartan los demás). */
export function validComponents(raw: unknown): { items: UserComponent[]; invalid: number } {
  const list = Array.isArray(raw) ? (raw as unknown[]) : [];
  const items: UserComponent[] = [];
  let invalid = 0;
  for (const it of list) {
    const c = it as Partial<UserComponent> | null;
    if (c && typeof c === 'object' && typeof c.name === 'string' && c.name.trim() !== '' && isComponentData(c.data)) items.push({ name: c.name.trim(), data: c.data });
    else invalid++;
  }
  return { items, invalid };
}

/** El nombre sin espacios sobrantes y con el largo máximo. */
export function cleanName(raw: string): string {
  return raw.trim().slice(0, MAX_COMPONENT_NAME);
}
