import type { Affine, BezPath, DesignObject, Document, Point } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { getPref, type AppState } from '@/store';
import {
  applyPoint, applyVector, breakAtNodes, buildPaths, canBreak, canJoin, closeSubs, deleteNodes, insertMidpoints, insertNode,
  invertAffine, joinEndpoints, moveNodes, nodeAt, openSubsOf, positionKey, positionsOf, rekeyCorners, selectedSegments,
  setNodeType, setSegmentsKind, type EditModel, type EditResult, type NodeType,
} from './nodeedit';
import type { NodePreview } from './interaction';
import { isNodeEditable, modelOf } from './nodeview';

/**
 * Operaciones de la herramienta de nodos sobre el store: leen la selección de objetos y de nodos,
 * calculan el modelo nuevo con `nodeedit` y lo escriben UNA vez con `applyDocChange` (un paso de
 * deshacer). Nada aquí depende del DOM, así que el registro de comandos y las pruebas las usan igual.
 */

export type NodeOp = 'insert' | 'delete' | 'toLine' | 'toCurve' | 'corner' | 'smooth' | 'symmetric' | 'break' | 'join' | 'close';

export const NODE_OPS: readonly NodeOp[] = ['insert', 'delete', 'toLine', 'toCurve', 'corner', 'smooth', 'symmetric', 'break', 'join', 'close'];

/** Pasos de las flechas en mm (los mismos que al mover objetos: preferencias de edición). */
export const nodeNudge = (): { normal: number; big: number; fine: number } => ({
  normal: getPref('nudgeStep'),
  big: getPref('nudgeStepBig'),
  fine: getPref('nudgeStepFine'),
});

type NodeState = Pick<AppState, 'doc' | 'selectedIds' | 'nodeSel'>;

export interface NodeTarget {
  obj: DesignObject;
  model: EditModel;
  /** Índices planos de los nodos seleccionados. */
  sel: number[];
  /** Inversa del `transform` del objeto: del documento a sus coordenadas locales. */
  inv: Affine;
}

/** Objetos seleccionados con nodos seleccionados (vectores sin bloquear y con `transform` invertible). */
export function nodeTargets(s: NodeState, preview: Record<string, NodePreview> | null = null): NodeTarget[] {
  const out: NodeTarget[] = [];
  for (const id of s.selectedIds) {
    const sel = s.nodeSel[id];
    if (!sel || sel.length === 0) continue;
    const obj = s.doc.objects.find((o) => o.id === id);
    if (!obj || !isNodeEditable(obj)) continue;
    const model = preview?.[id]?.model ?? modelOf(obj);
    const inv = invertAffine(obj.transform);
    if (model && inv) out.push({ obj, model, sel, inv });
  }
  return out;
}

export function selectedNodeCount(s: NodeState): number {
  return nodeTargets(s).reduce((n, t) => n + t.sel.length, 0);
}

/** Posiciones en coordenadas del documento de los nodos seleccionados. */
export function selectedNodePoints(s: NodeState, preview: Record<string, NodePreview> | null = null): Point[] {
  const out: Point[] = [];
  for (const t of nodeTargets(s, preview)) {
    for (const k of t.sel) {
      const n = nodeAt(t.model, k);
      if (n) out.push(applyPoint(t.obj.transform, n.p));
    }
  }
  return out;
}

/** Centro de la caja de los nodos seleccionados (con uno solo, el propio nodo), en mm de documento. */
export function selectedNodeCenter(s: NodeState, preview: Record<string, NodePreview> | null = null): Point | null {
  const pts = selectedNodePoints(s, preview);
  if (pts.length === 0) return null;
  const xs = pts.map((p) => p[0]);
  const ys = pts.map((p) => p[1]);
  return [(Math.min(...xs) + Math.max(...xs)) / 2, (Math.min(...ys) + Math.max(...ys)) / 2];
}

/** Escribe los trayectos nuevos de los objetos indicados; un objeto sin trayectos desaparece. */
export function replacePaths(doc: Document, changes: ReadonlyMap<string, BezPath[]>): Document {
  if (changes.size === 0) return doc;
  const objects: DesignObject[] = [];
  for (const o of doc.objects) {
    const paths = changes.get(o.id);
    if (!paths || o.kind.type !== 'vector') {
      objects.push(o);
      continue;
    }
    if (paths.length === 0) continue;
    objects.push({ ...o, kind: { ...o.kind, paths } });
  }
  return { ...doc, objects };
}

type Check = (t: NodeTarget) => boolean;

function anyTarget(s: AppState, check: Check): boolean {
  return nodeTargets(s).some(check);
}

/** `true` si la operación se puede hacer ahora o la clave del motivo por el que no. */
export function nodeOpAvailability(s: AppState, op: NodeOp): true | I18nKey {
  if (s.activeTool !== 'nodes') return 'nodes.reason.notTool';
  const targets = nodeTargets(s);
  if (targets.length === 0) return 'nodes.reason.noNodes';
  switch (op) {
    case 'insert':
      return anyTarget(s, (t) => selectedSegments(t.model, t.sel).length > 0) ? true : 'nodes.reason.needSegment';
    case 'toLine':
    case 'toCurve':
      return anyTarget(s, (t) => selectedSegments(t.model, t.sel, true).length > 0) ? true : 'nodes.reason.needSegment';
    case 'break':
      return anyTarget(s, (t) => canBreak(t.model, t.sel)) ? true : 'nodes.reason.cantBreak';
    case 'join':
      return targets.length === 1 && canJoin(targets[0].model, targets[0].sel) ? true : 'nodes.reason.needEnds';
    case 'close':
      return anyTarget(s, (t) => openSubsOf(t.model, t.sel).length > 0) ? true : 'nodes.reason.noOpen';
    default:
      return true;
  }
}

const LABEL: Record<NodeOp, I18nKey> = {
  insert: 'nodes.history.insert',
  delete: 'nodes.history.delete',
  toLine: 'nodes.history.toLine',
  toCurve: 'nodes.history.toCurve',
  corner: 'nodes.history.type',
  smooth: 'nodes.history.type',
  symmetric: 'nodes.history.type',
  break: 'nodes.history.break',
  join: 'nodes.history.join',
  close: 'nodes.history.close',
};

function compute(op: NodeOp, t: NodeTarget): EditResult | null {
  switch (op) {
    case 'insert': return insertMidpoints(t.model, t.sel);
    case 'delete': return deleteNodes(t.model, t.sel);
    case 'toLine': return setSegmentsKind(t.model, selectedSegments(t.model, t.sel, true), 'line', t.sel);
    case 'toCurve': return setSegmentsKind(t.model, selectedSegments(t.model, t.sel, true), 'curve', t.sel);
    case 'corner':
    case 'smooth':
    case 'symmetric': return setNodeType(t.model, t.sel, op as NodeType);
    case 'break': return breakAtNodes(t.model, t.sel);
    case 'join': return joinEndpoints(t.model, t.sel);
    case 'close': return closeSubs(t.model, openSubsOf(t.model, t.sel), t.sel);
  }
}

/** Aplica una operación a los nodos seleccionados. Devuelve `false` si no hizo nada. */
export function runNodeOp(s: AppState, op: NodeOp): boolean {
  if (nodeOpAvailability(s, op) !== true) return false;
  const changes = new Map<string, BezPath[]>();
  const nextSel: Record<string, number[]> = {};
  const corners: [string, string[]][] = [];
  for (const t of nodeTargets(s)) {
    const r = compute(op, t);
    if (!r) continue;
    changes.set(t.obj.id, buildPaths(r.model));
    nextSel[t.obj.id] = r.sel;
    if (op === 'corner' || op === 'smooth' || op === 'symmetric') {
      // "Esquina" se recuerda por posición (sus manejadores pueden seguir alineados); los otros tipos lo olvidan.
      const keys = new Set(s.nodeCorners[t.obj.id] ?? []);
      for (const p of positionsOf(t.model, t.sel)) {
        if (op === 'corner') keys.add(positionKey(p));
        else keys.delete(positionKey(p));
      }
      corners.push([t.obj.id, [...keys]]);
    }
  }
  if (changes.size === 0) return false;
  s.applyDocChange(LABEL[op], (doc) => replacePaths(doc, changes));
  s.pruneSelection(); // borrar todos los nodos de un objeto lo elimina
  for (const [id, keys] of corners) s.setNodeCorners(id, keys);
  s.setNodeSel(nextSel);
  return true;
}

/** Mueve los nodos seleccionados `d` mm de documento. `key` funde ráfagas en un solo paso de deshacer. */
export function nudgeNodes(s: AppState, d: Point, key: string, label: I18nKey = 'nodes.history.nudge'): boolean {
  const changes = new Map<string, BezPath[]>();
  const rekeys: [string, string[]][] = [];
  for (const t of nodeTargets(s)) {
    const local = applyVector(t.inv, d);
    const moved = moveNodes(t.model, t.sel, local);
    changes.set(t.obj.id, buildPaths(moved));
    const before = s.nodeCorners[t.obj.id];
    if (before?.length) rekeys.push([t.obj.id, rekeyCorners(before, positionsOf(t.model, t.sel), positionsOf(moved, t.sel))]);
  }
  if (changes.size === 0 || (d[0] === 0 && d[1] === 0)) return false;
  s.applyDocChange(label, (doc) => replacePaths(doc, changes), key);
  for (const [id, keys] of rekeys) s.setNodeCorners(id, keys);
  return true;
}

/** Pone una coordenada exacta (X o Y, en mm de documento) al centro de la selección de nodos. */
export function setNodeCoord(s: AppState, axis: 'x' | 'y', value: number): boolean {
  const c = selectedNodeCenter(s);
  if (!c || !Number.isFinite(value)) return false;
  const d: Point = axis === 'x' ? [value - c[0], 0] : [0, value - c[1]];
  const ids = nodeTargets(s).map((t) => t.obj.id).join(',');
  return nudgeNodes(s, d, `nodecoord:${axis}:${ids}`, 'nodes.history.coord');
}

/** Agrega un nodo en el segmento `(sub, i)` del objeto, en el parámetro `t`, y lo deja seleccionado. */
export function insertNodeAt(s: AppState, id: string, seg: { s: number; i: number; t: number }): boolean {
  const obj = s.doc.objects.find((o) => o.id === id);
  const model = obj && isNodeEditable(obj) ? modelOf(obj) : null;
  if (!obj || !model) return false;
  const r = insertNode(model, seg.s, seg.i, seg.t);
  if (!r) return false;
  const paths = buildPaths(r.model);
  s.applyDocChange('nodes.history.insert', (doc) => replacePaths(doc, new Map([[id, paths]])));
  s.setNodeSel({ [id]: r.sel });
  return true;
}
