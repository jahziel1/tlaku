import type { DesignObject, LayerSettings } from '@/api/types';
import { getGeom } from '@/features/canvas/geom';
import { isSelectable, layerLookup, pointInFill } from '@/features/canvas/hit';
import { objectBounds, objectPolylines } from '@/lib/geometry';

/**
 * Selecciones especiales (menú Editar). Todas devuelven ids y solo consideran objetos que se
 * pueden seleccionar (visibles, sin bloquear y de una capa visible). Aquí no hay estado: el comando
 * decide qué hacer con el resultado.
 */

/** Los objetos que se pueden seleccionar con el área. */
export function selectableObjects(objects: readonly DesignObject[], layers: readonly LayerSettings[]): DesignObject[] {
  const lookup = layerLookup(layers);
  return objects.filter((o) => isSelectable(o, lookup(o.layer)));
}

/** ¿Tiene algún contorno abierto? Los textos y las imágenes nunca son "abiertos". */
export function isOpenShape(obj: DesignObject): boolean {
  if (obj.kind.type !== 'vector') return false;
  return objectPolylines(obj).some((p) => !p.closed);
}

export function selectOpen(objects: readonly DesignObject[]): string[] {
  return objects.filter(isOpenShape).map((o) => o.id);
}

/** Abiertas en capas de relleno: no se pueden rellenar, así que no saldrían en el trabajo. */
export function selectOpenInFill(objects: readonly DesignObject[], layers: readonly LayerSettings[]): string[] {
  const lookup = layerLookup(layers);
  return objects.filter((o) => isOpenShape(o) && lookup(o.layer)?.mode === 'fill').map((o) => o.id);
}

export function selectInLayer(objects: readonly DesignObject[], color: string): string[] {
  const c = color.toLowerCase();
  return objects.filter((o) => o.layer.toLowerCase() === c).map((o) => o.id);
}

/** Área de la caja envolvente (mm²): la medida con la que se comparan tamaños. */
export function boxArea(obj: DesignObject): number {
  const b = objectBounds(obj);
  return b ? (b[1][0] - b[0][0]) * (b[1][1] - b[0][1]) : 0;
}

/**
 * Objetos sueltos (sin grupo) que quedan por completo dentro de alguna de las formas `containers`.
 * Las contenedoras siguen en el resultado. Necesita contornos cerrados en la contenedora.
 */
export function selectContained(objects: readonly DesignObject[], containers: readonly DesignObject[]): string[] {
  const ids = new Set(containers.map((c) => c.id));
  for (const box of containers) {
    const outer = objectBounds(box);
    if (!outer) continue;
    const geom = getGeom(box);
    for (const o of objects) {
      if (o.id === box.id || o.group) continue;
      const b = objectBounds(o);
      if (!b || b[0][0] < outer[0][0] || b[0][1] < outer[0][1] || b[1][0] > outer[1][0] || b[1][1] > outer[1][1]) continue;
      const first = objectPolylines(o)[0]?.points[0];
      if (first && pointInFill(geom, first[0], first[1])) ids.add(o.id);
    }
  }
  return [...ids];
}

/** Objetos fuera de `exclude` cuya caja envolvente tiene menos área que `limit` (mm²). */
export function selectSmaller(objects: readonly DesignObject[], limit: number, exclude: ReadonlySet<string> = new Set()): string[] {
  return objects.filter((o) => !exclude.has(o.id) && boxArea(o) < limit - 1e-9).map((o) => o.id);
}

/**
 * Diámetro (mm) si el objeto es un círculo: un solo contorno cerrado cuyos puntos están a la misma
 * distancia del centro (hasta 0,5 %). `null` si no.
 */
export function circleDiameter(obj: DesignObject): number | null {
  if (obj.kind.type !== 'vector') return null;
  const polys = objectPolylines(obj);
  if (polys.length !== 1 || !polys[0]!.closed || polys[0]!.points.length < 16) return null;
  const b = objectBounds(obj);
  if (!b) return null;
  const w = b[1][0] - b[0][0];
  const h = b[1][1] - b[0][1];
  if (w < 1e-9 || Math.abs(w - h) > 0.01 * Math.max(w, h)) return null;
  const cx = (b[0][0] + b[1][0]) / 2;
  const cy = (b[0][1] + b[1][1]) / 2;
  const radii = polys[0]!.points.map(([x, y]) => Math.hypot(x - cx, y - cy));
  const mean = radii.reduce((a, r) => a + r, 0) / radii.length;
  const spread = Math.max(...radii) - Math.min(...radii);
  return spread <= Math.max(0.01 * mean, 0.03) ? w : null;
}

/** Círculos cuyo diámetro está entre `min` y `max` (mm, inclusive, con 0,001 de holgura). */
export function selectCircles(objects: readonly DesignObject[], min: number, max: number): string[] {
  const lo = Math.min(min, max) - 1e-3;
  const hi = Math.max(min, max) + 1e-3;
  return objects
    .filter((o) => {
      const d = circleDiameter(o);
      return d !== null && d >= lo && d <= hi;
    })
    .map((o) => o.id);
}
