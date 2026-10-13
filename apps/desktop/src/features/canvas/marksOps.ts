import type { DesignObject, Document, Point, StartSpec } from '@/api/types';

/**
 * Cambios del documento para puentes manuales (`manual_tabs`) y punto de inicio / sentido
 * (`start`). Son funciones puras sobre el documento: devuelven el mismo objeto si no hay nada que
 * cambiar, para que `applyDocChange` no deje pasos vacíos en el historial.
 */

const round3 = (n: number) => Math.round(n * 1000) / 1000;
const roundPoint = (p: Point): Point => [round3(p[0]), round3(p[1])];

const mapObject = (doc: Document, ids: ReadonlySet<string>, change: (o: DesignObject) => DesignObject): Document => {
  let touched = false;
  const objects = doc.objects.map((o) => {
    if (!ids.has(o.id) || o.locked) return o;
    const next = change(o);
    if (next !== o) touched = true;
    return next;
  });
  return touched ? { ...doc, objects } : doc;
};

/** Distancia a la que un clic cuenta como "sobre un puente que ya está". */
export function tabIndexNear(tabs: readonly Point[], p: Point, reach: number): number {
  let best = -1;
  let bestD = reach + 1e-9;
  tabs.forEach((t, i) => {
    const d = Math.hypot(t[0] - p[0], t[1] - p[1]);
    if (d <= bestD) {
      best = i;
      bestD = d;
    }
  });
  return best;
}

/**
 * Pone un puente en `point` (un punto del contorno) o, si ya hay uno a menos de `reach` mm, lo quita.
 * Devuelve cuál fue la acción para avisar y para el historial.
 */
export function toggleTab(doc: Document, id: string, point: Point, reach: number): { doc: Document; action: 'add' | 'remove' | 'none' } {
  const obj = doc.objects.find((o) => o.id === id);
  if (!obj || obj.locked) return { doc, action: 'none' };
  const tabs = obj.manual_tabs ?? [];
  const at = tabIndexNear(tabs, point, reach);
  const manual_tabs = at >= 0 ? tabs.filter((_, i) => i !== at) : [...tabs, roundPoint(point)];
  const next = mapObject(doc, new Set([id]), (o) => ({ ...o, manual_tabs }));
  return { doc: next, action: at >= 0 ? 'remove' : 'add' };
}

export function clearTabs(doc: Document, ids: readonly string[]): Document {
  return mapObject(doc, new Set(ids), (o) => ((o.manual_tabs?.length ?? 0) === 0 ? o : { ...o, manual_tabs: [] }));
}

/** `start` normalizado: sin punto ni inversión no queda nada guardado. */
const tidy = (start: StartSpec): StartSpec | null => (start.point === null && !start.reverse ? null : start);

/** Fija el punto de inicio de `id` (conserva el sentido que ya tuviera). */
export function setStartPoint(doc: Document, id: string, point: Point): Document {
  return mapObject(doc, new Set([id]), (o) => {
    const start = tidy({ point: roundPoint(point), reverse: o.start?.reverse ?? false });
    return { ...o, start };
  });
}

/**
 * Invierte el sentido de los objetos: si todos ya van invertidos los deja como estaban; si no, los
 * invierte todos. Sirve a imágenes y objetos sin trazo igual que a los demás (no hace nada con ellos).
 */
export function reverseDirection(doc: Document, ids: readonly string[]): Document {
  const set = new Set(ids);
  const targets = doc.objects.filter((o) => set.has(o.id) && !o.locked && o.kind.type !== 'image');
  if (targets.length === 0) return doc;
  const reverse = !targets.every((o) => o.start?.reverse === true);
  return mapObject(doc, new Set(targets.map((o) => o.id)), (o) => {
    const start = tidy({ point: o.start?.point ?? null, reverse });
    return { ...o, start };
  });
}

export function clearStart(doc: Document, ids: readonly string[]): Document {
  return mapObject(doc, new Set(ids), (o) => (o.start == null ? o : { ...o, start: null }));
}

/** ¿Alguno de los objetos tiene puentes manuales / inicio fijado? (para habilitar los comandos). */
export const anyTabs = (objs: readonly DesignObject[]): boolean => objs.some((o) => (o.manual_tabs?.length ?? 0) > 0);
export const anyStart = (objs: readonly DesignObject[]): boolean => objs.some((o) => o.start != null);
