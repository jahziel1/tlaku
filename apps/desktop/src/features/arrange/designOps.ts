import { getApi } from '@/api';
import type { DesignObject, Document, TangentCircle, VirtualArray } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { ellipseBez, flattenBez } from '@/lib/bezpath';
import { errorMessage } from '@/lib/errors';
import { multiplyAffine, objectCurves } from '@/lib/geometry';
import { groupIdsOf, regroupCopies } from '@/lib/groups';
import { virtualArrayIssue, virtualCopies } from '@/lib/virtualArray';
import { selectSelectedObjects, type AppState } from '@/store';
import { uniqueId } from '@/store/document';
import { analyzeSelection, operandOf } from './geomOps';
import { useDesignUi } from './designUi';
import { envelopeOf, isNoWarp, type WarpParams } from './warpPresets';
import { blockBounds, selectionUnits, type Unit } from './units';

/** Mismo operando de siempre: un grupo viaja como un solo objeto con todas sus curvas. */
export const operandsOf = (units: readonly Unit[]): DesignObject[] => units.map(operandOf);

/** Los objetos de las unidades, sueltos y con su transform (cada uno se deforma por separado). */
const flatObjects = (units: readonly Unit[]): DesignObject[] => units.flatMap((u) => u.objects);

// ---- motivos por los que un comando no está disponible ------------------------------------------

/** Qué impide trabajar con la selección, o `true`. `min` = unidades mínimas; `max` = máximas. */
export function designReason(s: Pick<AppState, 'doc' | 'selectedIds'>, min = 1, max = Infinity, allowOpen = true): true | I18nKey {
  if (s.selectedIds.length === 0) return 'arrange.reason.noSelection';
  const a = analyzeSelection(s);
  if (a.units === 0) return 'arrange.reason.noSelection';
  if (a.problem === 'locked') return 'arrange.reason.locked';
  if (a.problem === 'image') return 'arrange.reason.image';
  if (a.problem === 'noContour') return 'arrange.reason.noContour';
  if (a.problem === 'open' && !allowOpen) return 'arrange.reason.open';
  if (a.units < min) return min === 2 ? 'design.reason.needTwo' : 'design.reason.needMore';
  if (a.units > max) return 'design.reason.tooMany';
  return true;
}

// ---- deformar --------------------------------------------------------------------------------------

/** Pide al motor los objetos deformados con los parámetros; sin deformación devuelve los mismos. */
export async function requestWarp(units: readonly Unit[], params: WarpParams): Promise<DesignObject[]> {
  const bounds = blockBounds(units);
  if (!bounds) throw new Error(t('arrange.reason.noSelection'));
  return getApi().warpObjects({ objects: flatObjects(units), bounds, envelope: envelopeOf(params, bounds) });
}

/** Sustituye cada objeto por su versión deformada (mismo id y lugar en el orden de dibujo). */
export function replaceObjects(doc: Document, results: readonly DesignObject[]): Document {
  const by = new Map(results.map((r) => [r.id, r]));
  return { ...doc, objects: doc.objects.map((o) => by.get(o.id) ?? o) };
}

export async function runWarp(s: AppState, params: WarpParams): Promise<boolean> {
  if (isNoWarp(params)) return false;
  const units = selectionUnits(s.doc, s.selectedIds);
  const ui = useDesignUi.getState();
  ui.setBusy(true);
  let results: DesignObject[];
  try {
    results = await requestWarp(units, params);
  } catch (e) {
    s.notify('error', errorMessage(e));
    return false;
  } finally {
    useDesignUi.getState().setBusy(false);
  }
  s.applyDocChange('design.history.warp', (doc) => replaceObjects(doc, results));
  return true;
}

// ---- contorno envolvente ---------------------------------------------------------------------------

export async function requestHull(units: readonly Unit[], maxEdge: number | null, layer: string | null): Promise<DesignObject[]> {
  return getApi().hullOutline({ objects: flatObjects(units), max_edge: maxEdge, layer });
}

/** Agrega el contorno (y quita los originales si no se conservan). Devuelve el id nuevo o `null`. */
export async function runHull(s: AppState, concave: boolean, maxEdge: number, keepOriginal: boolean): Promise<string | null> {
  const units = selectionUnits(s.doc, s.selectedIds);
  const ui = useDesignUi.getState();
  ui.setBusy(true);
  let results: DesignObject[];
  try {
    results = await requestHull(units, concave ? maxEdge : null, s.activeLayer);
  } catch (e) {
    s.notify('error', errorMessage(e));
    return null;
  } finally {
    useDesignUi.getState().setBusy(false);
  }
  const removed = keepOriginal ? new Set<string>() : new Set(units.flatMap((u) => u.ids));
  let id: string | null = null;
  s.applyDocChange('design.history.hull', (doc) => {
    const taken = new Set(doc.objects.map((o) => o.id));
    const fresh = results.map((r) => {
      const unique = uniqueId('contorno', taken);
      taken.add(unique);
      id = unique;
      return { ...r, id: unique, group: null, locked: false, hidden: false };
    });
    return { ...doc, objects: [...doc.objects.filter((o) => !removed.has(o.id)), ...fresh] };
  });
  if (id) s.select([id]);
  return id;
}

// ---- círculo tangente ------------------------------------------------------------------------------

/** Objeto círculo (con sus parámetros de elipse) centrado en `circle.center`. */
export function circleObject(id: string, layer: string, circle: TangentCircle): DesignObject {
  const [cx, cy] = circle.center;
  const r = circle.radius;
  return {
    id,
    layer,
    transform: [1, 0, 0, 1, cx, cy],
    kind: { type: 'vector', paths: [ellipseBez(0, 0, r, r)] },
    locked: false,
    hidden: false,
    group: null,
    shape: { type: 'ellipse', rx: r, ry: r },
  };
}

export function addCircles(s: AppState, circles: readonly TangentCircle[], layer: string): string[] {
  const created: string[] = [];
  s.applyDocChange('design.history.tangent', (doc) => {
    created.length = 0;
    const taken = new Set(doc.objects.map((o) => o.id));
    const fresh = circles.map((c) => {
      const id = uniqueId('circulo', taken);
      taken.add(id);
      created.push(id);
      return circleObject(id, layer, c);
    });
    return { ...doc, objects: [...doc.objects, ...fresh] };
  });
  if (created.length) s.select(created);
  return created;
}

// ---- arreglo virtual -------------------------------------------------------------------------------

/** El objeto único al que se puede asignar un arreglo virtual, o el motivo por el que no hay. */
export function virtualTarget(s: Pick<AppState, 'doc' | 'selectedIds'>): { object: DesignObject } | { reason: I18nKey } {
  const picked = selectSelectedObjects(s as AppState);
  if (picked.length === 0) return { reason: 'arrange.reason.noSelection' };
  if (picked.length > 1) return { reason: 'design.reason.virtualOne' };
  const object = picked[0]!;
  if (object.locked) return { reason: 'arrange.reason.locked' };
  if (object.group) return { reason: 'design.reason.virtualGrouped' };
  return { object };
}

/** Asigna (o quita, con `null`) el arreglo virtual de un objeto. */
export function setVirtualArray(s: AppState, id: string, array: VirtualArray | null): void {
  if (array && virtualArrayIssue(array)) return;
  s.applyDocChange(array ? 'design.history.virtual' : 'design.history.virtualRemove', (doc) => ({
    ...doc,
    objects: doc.objects.map((o) => (o.id === id ? { ...o, array } : o)),
  }));
}

/**
 * Convierte el arreglo virtual de los objetos con `ids` en objetos de verdad: cada copia pasa a ser
 * un objeto nuevo (al final del orden de dibujo, con id propio) y el original deja de tener arreglo.
 * Devuelve el documento nuevo y los ids de las copias; el mismo documento si no había arreglos.
 */
export function flattenArrays(doc: Document, ids: ReadonlySet<string>): { doc: Document; created: string[] } {
  const taken = new Set(doc.objects.map((o) => o.id));
  const groups = groupIdsOf(doc.objects);
  const created: string[] = [];
  const copies: DesignObject[] = [];
  let changed = false;
  const objects = doc.objects.map((o) => {
    if (!ids.has(o.id) || !o.array) return o;
    changed = true;
    const made = virtualCopies(o).map((m) => {
      const id = uniqueId(o.id.replace(/-\d+$/, ''), taken);
      taken.add(id);
      created.push(id);
      return { ...o, id, locked: false, array: null, transform: multiplyAffine(m, o.transform) };
    });
    copies.push(...regroupCopies(made, groups));
    return { ...o, array: null };
  });
  return changed ? { doc: { ...doc, objects: [...objects, ...copies] }, created } : { doc, created };
}

// ---- vistas previas en miniatura -------------------------------------------------------------------

/** Polilíneas (en mm, Y hacia arriba) de una lista de objetos, para dibujar una vista previa. */
export function outlinesOf(objects: readonly DesignObject[]): { points: [number, number][]; closed: boolean }[] {
  return objects.flatMap((o) => objectCurves(o)).flatMap((path) => flattenBez(path, 0.05));
}
