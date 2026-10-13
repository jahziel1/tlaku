import type { BezPath, PathSeg, Point } from '@/api/types';

/**
 * Dibujo de curvas con la herramienta Línea: un clic pone un nodo de esquina (segmento recto) y
 * arrastrar al poner el nodo saca sus manijas (curva). La manija de salida sigue al puntero y la
 * de entrada es su reflejo, así el nodo queda suave.
 */
export interface PenNode {
  p: Point;
  /** Manija que llega al nodo (absoluta); `null` si no hay. */
  hin: Point | null;
  /** Manija que sale del nodo (absoluta); `null` si no hay. */
  hout: Point | null;
}

/** Distancia mínima (mm) entre el nodo y el puntero para considerar que se arrastró una manija. */
const MIN_HANDLE_MM = 1e-6;

export const cornerNode = (p: Point): PenNode => ({ p, hin: null, hout: null });

/** Nodo con las manijas definidas por el puntero: salida en `pointer`, entrada reflejada. */
export function smoothNode(p: Point, pointer: Point): PenNode {
  if (Math.hypot(pointer[0] - p[0], pointer[1] - p[1]) < MIN_HANDLE_MM) return cornerNode(p);
  return { p, hout: pointer, hin: [2 * p[0] - pointer[0], 2 * p[1] - pointer[1]] };
}

/** Quita nodos consecutivos repetidos (el doble clic suma uno igual). */
export function dedupeNodes(nodes: readonly PenNode[]): PenNode[] {
  const out: PenNode[] = [];
  for (const n of nodes) {
    const last = out[out.length - 1];
    if (!last || Math.hypot(n.p[0] - last.p[0], n.p[1] - last.p[1]) > 1e-9) out.push(n);
  }
  return out;
}

/** Segmento de `a` a `b`: cúbico si alguna de sus manijas existe, recto si no. */
function segment(a: PenNode, b: PenNode): PathSeg {
  if (!a.hout && !b.hin) return { type: 'line_to', to: b.p };
  return { type: 'cubic_to', ctrl1: a.hout ?? a.p, ctrl2: b.hin ?? b.p, to: b.p };
}

/**
 * Trayectoria de los nodos, o `null` si no hay al menos dos. Cerrada, el último nodo se une al
 * primero (con curva si hay manijas) y la forma termina en `close`.
 */
export function penPath(nodes: readonly PenNode[], close: boolean): BezPath | null {
  const ns = dedupeNodes(nodes);
  if (ns.length < 2) return null;
  const path: PathSeg[] = [{ type: 'move_to', to: ns[0]!.p }];
  for (let i = 1; i < ns.length; i++) path.push(segment(ns[i - 1]!, ns[i]!));
  const closed = close && ns.length >= 3;
  if (closed) {
    const last = ns[ns.length - 1]!;
    const first = ns[0]!;
    if (last.hout || first.hin) path.push(segment(last, first));
    path.push({ type: 'close' });
  }
  return path;
}

/** ¿Tiene alguna curva? (decide el nombre del paso de deshacer). */
export function hasCurves(path: BezPath): boolean {
  return path.some((s) => s.type === 'cubic_to' || s.type === 'quad_to');
}

/**
 * Vista previa mientras se dibuja: lo ya puesto más el tramo elástico hasta el cursor. Sin cursor
 * (o con un nodo solo) devuelve lo que haya.
 */
export function penDraft(nodes: readonly PenNode[], cursor: Point | null): BezPath {
  const ns = nodes.slice();
  if (ns.length === 0) return [];
  const path: PathSeg[] = [{ type: 'move_to', to: ns[0]!.p }];
  for (let i = 1; i < ns.length; i++) path.push(segment(ns[i - 1]!, ns[i]!));
  if (cursor) {
    const last = ns[ns.length - 1]!;
    path.push(segment(last, { p: cursor, hin: null, hout: null }));
  }
  return path;
}

/** Manijas del nodo para dibujarlas: pares (nodo, manija). */
export function handleLines(node: PenNode | undefined): [Point, Point][] {
  if (!node) return [];
  const out: [Point, Point][] = [];
  if (node.hin) out.push([node.p, node.hin]);
  if (node.hout) out.push([node.p, node.hout]);
  return out;
}
