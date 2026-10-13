import type { DesignObject } from '@/api/types';
import { applyPoint, nodeType, parseModel, segmentCount, type EditModel, type NodeType } from './nodeedit';
import type { NodeHover, NodePreview } from './interaction';

/** Un nodo tal como se dibuja y se acierta con el puntero (coordenadas del documento). */
export interface NodeItem {
  id: string;
  k: number;
  x: number;
  y: number;
  type: NodeType;
  selected: boolean;
}

/** Manejador visible: línea del nodo `(nx, ny)` al punto `(hx, hy)`. */
export interface HandleItem {
  id: string;
  k: number;
  which: 'in' | 'out';
  nx: number;
  ny: number;
  hx: number;
  hy: number;
}

export interface NodeOverlay {
  nodes: NodeItem[];
  handles: HandleItem[];
  hover: NodeHover;
  /** Hay un arrastre en curso: los objetos se dibujan con la vista previa. */
  editing: boolean;
}

const modelCache = new WeakMap<object, EditModel>();

/** Modelo de edición de un objeto vectorial (se calcula una vez por versión del objeto). */
export function modelOf(obj: DesignObject): EditModel | null {
  if (obj.kind.type !== 'vector') return null;
  let m = modelCache.get(obj.kind);
  if (!m) {
    m = parseModel(obj.kind.paths);
    modelCache.set(obj.kind, m);
  }
  return m;
}

/** Objetos cuyos nodos se pueden editar: vectores visibles y sin bloquear. */
export function isNodeEditable(obj: DesignObject): boolean {
  return obj.kind.type === 'vector' && !obj.locked && !obj.hidden;
}

const previewObjects = new WeakMap<NodePreview, DesignObject>();

/** El objeto con la vista previa del arrastre aplicada (mismo objeto mientras la vista previa no cambie). */
export function withPreview(obj: DesignObject, preview: NodePreview): DesignObject {
  let o = previewObjects.get(preview);
  if (!o && obj.kind.type === 'vector') {
    o = { ...obj, kind: { ...obj.kind, paths: preview.paths } };
    previewObjects.set(preview, o);
  }
  return o ?? obj;
}

/** Modelo vigente de un objeto: el del arrastre en curso si lo hay. */
export function liveModel(obj: DesignObject, preview: Record<string, NodePreview> | null): EditModel | null {
  return preview?.[obj.id]?.model ?? modelOf(obj);
}

export interface OverlayInput {
  objects: readonly DesignObject[];
  selectedIds: readonly string[];
  nodeSel: Record<string, number[]>;
  nodeCorners: Record<string, string[]>;
  preview: Record<string, NodePreview> | null;
  hover: NodeHover;
}

let memo: { input: OverlayInput; out: NodeOverlay } | null = null;

/**
 * Nodos y manejadores a dibujar: todos los nodos de los vectores seleccionados y los manejadores
 * de los nodos seleccionados (más el de cada vecino que mira hacia ellos). Se recalcula solo si cambia algún insumo.
 */
export function buildNodeOverlay(input: OverlayInput): NodeOverlay {
  const m = memo?.input;
  if (
    m && m.objects === input.objects && m.selectedIds === input.selectedIds && m.nodeSel === input.nodeSel &&
    m.nodeCorners === input.nodeCorners && m.preview === input.preview && m.hover === input.hover
  ) return memo!.out;
  const nodes: NodeItem[] = [];
  const handles: HandleItem[] = [];
  const picked = new Set(input.selectedIds);
  for (const obj of input.objects) {
    if (!picked.has(obj.id) || !isNodeEditable(obj)) continue;
    const model = liveModel(obj, input.preview);
    if (!model) continue;
    const sel = new Set(input.nodeSel[obj.id] ?? []);
    const corners = new Set(input.nodeCorners[obj.id] ?? []);
    const t = obj.transform;
    let k = 0;
    for (const sb of model.subs) {
      const base = k;
      const n = sb.nodes.length;
      const shown = new Set<string>();
      sb.nodes.forEach((node, i) => {
        const [x, y] = applyPoint(t, node.p);
        const selected = sel.has(base + i);
        nodes.push({ id: obj.id, k: base + i, x, y, type: nodeType(node, corners), selected });
        if (!selected) return;
        if (node.inH) shown.add(`${i}:in`);
        if (node.outH) shown.add(`${i}:out`);
        const next = i + 1 < n ? i + 1 : sb.closed ? 0 : -1;
        if (next >= 0 && i < segmentCount(sb) && sb.nodes[next].inH) shown.add(`${next}:in`);
        const prev = i > 0 ? i - 1 : sb.closed ? n - 1 : -1;
        if (prev >= 0 && sb.nodes[prev].outH) shown.add(`${prev}:out`);
      });
      for (const key of shown) {
        const [is, which] = key.split(':') as [string, 'in' | 'out'];
        const i = Number(is);
        const node = sb.nodes[i];
        const h = which === 'in' ? node.inH : node.outH;
        if (!h) continue;
        const [nx, ny] = applyPoint(t, node.p);
        const [hx, hy] = applyPoint(t, h);
        handles.push({ id: obj.id, k: base + i, which, nx, ny, hx, hy });
      }
      k += n;
    }
  }
  const out: NodeOverlay = { nodes, handles, hover: input.hover, editing: input.preview !== null };
  memo = { input, out };
  return out;
}
