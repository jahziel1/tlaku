import { describe, expect, it } from 'vitest';
import type { Affine, BezPath, Point } from '@/api/types';
import { cubicAt, ellipseBez, flattenBez, polylineBez, splitCubic } from '@/lib/bezpath';
import {
  breakAtNodes, buildPaths, canBreak, canJoin, closeSubs, deleteNodes, inferNodeType, insertMidpoints, insertNode, invertAffine,
  joinEndpoints, moveHandle, moveNodes, nearestSegment, nodeAt, nodeCount, nodeType, openSubsOf, parseModel, positionKey,
  rekeyCorners, selectedSegments, setNodeType, setSegmentsKind, applyPoint,
} from './nodeedit';

const close = (a: Point, b: Point, eps = 1e-9) => Math.hypot(a[0] - b[0], a[1] - b[1]) < eps;
const polyline = (pts: Point[], closed = false) => parseModel([polylineBez(pts, closed)]);
const SQUARE: Point[] = [[0, 0], [10, 0], [10, 10], [0, 10]];

/** Curva muestreada: sirve para comprobar que una operación no cambió la forma. */
function sample(paths: BezPath[], n = 40): Point[] {
  return paths.flatMap((p) => flattenBez(p, 0.0005).flatMap((f) => f.points)).filter((_, i) => i % Math.max(1, n) === 0);
}

/** Distancia máxima de los puntos de `a` a la polilínea de `b` (a los segmentos, no solo a los vértices). */
function maxDistance(a: Point[], b: BezPath[]): number {
  const segs: [Point, Point][] = [];
  for (const p of b) for (const f of flattenBez(p, 0.0005)) for (let i = 0; i + 1 < f.points.length; i++) segs.push([f.points[i], f.points[i + 1]]);
  const dist = (p: Point, [u, v]: [Point, Point]) => {
    const dx = v[0] - u[0];
    const dy = v[1] - u[1];
    const l2 = dx * dx + dy * dy;
    const t = l2 === 0 ? 0 : Math.min(1, Math.max(0, ((p[0] - u[0]) * dx + (p[1] - u[1]) * dy) / l2));
    return Math.hypot(u[0] + t * dx - p[0], u[1] + t * dy - p[1]);
  };
  let worst = 0;
  for (const p of a) worst = Math.max(worst, Math.min(...segs.map((sg) => dist(p, sg))));
  return worst;
}

const CURVE: BezPath = [
  { type: 'move_to', to: [0, 0] },
  { type: 'cubic_to', ctrl1: [0, 10], ctrl2: [10, 10], to: [10, 0] },
];

describe('división de Bézier en t', () => {
  it('las dos mitades suman la misma curva y se tocan en el punto de t', () => {
    const p0: Point = [0, 0];
    const c1: Point = [0, 10];
    const c2: Point = [10, 10];
    const p1: Point = [10, 0];
    for (const t of [0.1, 0.5, 0.83]) {
      const [l, r] = splitCubic(p0, c1, c2, p1, t);
      expect(close(l[3], cubicAt(p0, c1, c2, p1, t))).toBe(true);
      expect(close(l[3], r[0])).toBe(true);
      for (const u of [0, 0.25, 0.6, 1]) {
        expect(close(cubicAt(...l, u), cubicAt(p0, c1, c2, p1, u * t), 1e-9)).toBe(true);
        expect(close(cubicAt(...r, u), cubicAt(p0, c1, c2, p1, t + u * (1 - t)), 1e-9)).toBe(true);
      }
    }
  });
});

describe('lectura y reconstrucción', () => {
  it('un polígono cerrado vuelve igual', () => {
    const path = polylineBez(SQUARE, true);
    const m = parseModel([path]);
    expect(m.subs).toHaveLength(1);
    expect(m.subs[0].nodes).toHaveLength(4);
    expect(m.subs[0].closed).toBe(true);
    expect(buildPaths(m)).toEqual([path]);
  });

  it('una elipse son 4 nodos con manejadores y un cierre sin nodo repetido', () => {
    const m = parseModel([ellipseBez(0, 0, 10, 5)]);
    expect(nodeCount(m)).toBe(4);
    expect(m.subs[0].closed).toBe(true);
    for (const n of m.subs[0].nodes) expect(inferNodeType(n)).toBe('symmetric');
  });

  it('las cuadráticas se elevan a cúbicas sin cambiar la forma', () => {
    const quad: BezPath = [{ type: 'move_to', to: [0, 0] }, { type: 'quad_to', ctrl: [5, 10], to: [10, 0] }];
    const out = buildPaths(parseModel([quad]));
    expect(out[0][1].type).toBe('cubic_to');
    expect(maxDistance(sample([quad]), out)).toBeLessThan(0.01);
  });

  it('varios subtrayectos y trayectorias conservan su agrupación', () => {
    const a = polylineBez(SQUARE, true);
    const b: BezPath = [...polylineBez([[20, 0], [30, 0]], false), ...polylineBez([[20, 5], [30, 5]], false)];
    const out = buildPaths(parseModel([a, b]));
    expect(out).toHaveLength(2);
    expect(out[1].filter((s) => s.type === 'move_to')).toHaveLength(2);
  });
});

describe('insertar nodos', () => {
  it('en una recta cae en t sin manejadores', () => {
    const m = polyline([[0, 0], [10, 0]]);
    const r = insertNode(m, 0, 0, 0.25)!;
    expect(r.model.subs[0].nodes.map((n) => n.p)).toEqual([[0, 0], [2.5, 0], [10, 0]]);
    expect(r.model.subs[0].nodes[1].inH).toBeNull();
    expect(r.sel).toEqual([1]);
  });

  it('en una curva la forma no cambia y el nodo nuevo queda suave', () => {
    const m = parseModel([CURVE]);
    for (const t of [0.2, 0.5, 0.9]) {
      const r = insertNode(m, 0, 0, t)!;
      expect(nodeCount(r.model)).toBe(3);
      expect(maxDistance(sample([CURVE]), buildPaths(r.model))).toBeLessThan(1e-3);
      expect(close(r.model.subs[0].nodes[1].p, cubicAt([0, 0], [0, 10], [10, 10], [10, 0], t))).toBe(true);
      expect(inferNodeType(r.model.subs[0].nodes[1])).not.toBe('corner'); // alineados: suave (o simétrico si son iguales)
    }
  });

  it('en el segmento de cierre el nodo va al final', () => {
    const m = polyline(SQUARE, true);
    const r = insertNode(m, 0, 3, 0.5)!;
    expect(r.model.subs[0].nodes[4].p).toEqual([0, 5]);
    expect(r.sel).toEqual([4]);
  });

  it('el punto medio de los segmentos entre nodos elegidos', () => {
    const m = polyline(SQUARE, true);
    const r = insertMidpoints(m, [0, 1, 2])!;
    expect(nodeCount(r.model)).toBe(6);
    expect(r.sel).toHaveLength(2);
    expect(insertMidpoints(m, [0])).toBeNull();
  });

  it('rechaza un segmento que no existe', () => {
    expect(insertNode(polyline([[0, 0], [1, 1]]), 0, 1, 0.5)).toBeNull();
  });
});

describe('borrar nodos', () => {
  it('un nodo del medio reconecta a sus vecinos', () => {
    const r = deleteNodes(polyline([[0, 0], [5, 0], [10, 0], [10, 10]]), [1])!;
    expect(r.model.subs[0].nodes.map((n) => n.p)).toEqual([[0, 0], [10, 0], [10, 10]]);
  });

  it('un extremo acorta el trayecto abierto', () => {
    const r = deleteNodes(polyline([[0, 0], [5, 0], [10, 0]]), [2])!;
    expect(r.model.subs[0].nodes).toHaveLength(2);
    expect(r.model.subs[0].closed).toBe(false);
  });

  it('un cerrado conserva el cierre y los manejadores que quedan', () => {
    const m = parseModel([ellipseBez(0, 0, 10, 10)]);
    const r = deleteNodes(m, [0])!;
    expect(r.model.subs[0].closed).toBe(true);
    expect(r.model.subs[0].nodes).toHaveLength(3);
  });

  it('si no quedan dos nodos el trayecto desaparece (sin huecos raros)', () => {
    const r = deleteNodes(polyline([[0, 0], [5, 0], [10, 0]]), [0, 1])!;
    expect(r.model.subs).toHaveLength(0);
    expect(buildPaths(r.model)).toEqual([]);
    // un cerrado de rectas con 2 nodos no encierra nada
    expect(deleteNodes(polyline([[0, 0], [10, 0], [10, 10]], true), [2])!.model.subs).toHaveLength(0);
  });

  it('solo borra los subtrayectos que tocan los nodos', () => {
    const m = parseModel([[...polylineBez(SQUARE, true), ...polylineBez([[20, 0], [30, 0], [40, 0]], false)]]);
    const r = deleteNodes(m, [4])!;
    expect(r.model.subs).toHaveLength(2);
    expect(r.model.subs[0].nodes).toHaveLength(4);
    expect(r.model.subs[1].nodes).toHaveLength(2);
  });
});

describe('recta y curva', () => {
  it('a recta quita los manejadores del segmento', () => {
    const m = parseModel([CURVE]);
    const r = setSegmentsKind(m, [{ s: 0, i: 0 }], 'line')!;
    expect(buildPaths(r.model)[0][1].type).toBe('line_to');
  });

  it('a curva usa tercios: la forma sigue siendo la recta', () => {
    const m = polyline([[0, 0], [9, 0]]);
    const r = setSegmentsKind(m, [{ s: 0, i: 0 }], 'curve')!;
    const seg = buildPaths(r.model)[0][1];
    expect(seg).toEqual({ type: 'cubic_to', ctrl1: [3, 0], ctrl2: [6, 0], to: [9, 0] });
  });

  it('una curva a media (un solo manejador) se completa sin mover el existente', () => {
    const m = parseModel([[{ type: 'move_to', to: [0, 0] }, { type: 'cubic_to', ctrl1: [0, 4], ctrl2: [10, 0], to: [10, 0] }]]);
    const r = setSegmentsKind(m, [{ s: 0, i: 0 }], 'curve')!;
    expect(r.model.subs[0].nodes[0].outH).toEqual([0, 4]);
    expect(r.model.subs[0].nodes[1].inH).not.toBeNull();
  });

  it('los segmentos elegidos son los que tienen sus dos extremos elegidos; con un nodo, los dos vecinos', () => {
    const m = polyline(SQUARE, true);
    expect(selectedSegments(m, [0, 1])).toEqual([{ s: 0, i: 0 }]);
    expect(selectedSegments(m, [3, 0])).toEqual([{ s: 0, i: 3 }]);
    expect(selectedSegments(m, [2])).toEqual([]);
    expect(selectedSegments(m, [2], true)).toEqual([{ s: 0, i: 1 }, { s: 0, i: 2 }]);
    expect(selectedSegments(polyline([[0, 0], [1, 0], [2, 0]]), [0], true)).toEqual([{ s: 0, i: 0 }]);
  });
});

describe('tipos de nodo', () => {
  const smoothPts = () =>
    parseModel([[
      { type: 'move_to', to: [0, 0] },
      { type: 'cubic_to', ctrl1: [0, 5], ctrl2: [3, 8], to: [10, 10] },
      { type: 'cubic_to', ctrl1: [15, 10], ctrl2: [20, 5], to: [20, 0] },
    ]]);

  it('suave alinea los manejadores y conserva sus longitudes', () => {
    const m = smoothPts();
    const before = m.subs[0].nodes[1];
    const lenIn = Math.hypot(before.inH![0] - before.p[0], before.inH![1] - before.p[1]);
    const lenOut = Math.hypot(before.outH![0] - before.p[0], before.outH![1] - before.p[1]);
    expect(inferNodeType(before)).toBe('corner');
    const n = setNodeType(m, [1], 'smooth')!.model.subs[0].nodes[1];
    expect(inferNodeType(n)).toBe('smooth');
    expect(Math.hypot(n.inH![0] - n.p[0], n.inH![1] - n.p[1])).toBeCloseTo(lenIn, 9);
    expect(Math.hypot(n.outH![0] - n.p[0], n.outH![1] - n.p[1])).toBeCloseTo(lenOut, 9);
  });

  it('simétrico iguala las longitudes', () => {
    const n = setNodeType(smoothPts(), [1], 'symmetric')!.model.subs[0].nodes[1];
    expect(inferNodeType(n)).toBe('symmetric');
    expect(Math.hypot(n.inH![0] - n.p[0], n.inH![1] - n.p[1])).toBeCloseTo(Math.hypot(n.outH![0] - n.p[0], n.outH![1] - n.p[1]), 9);
  });

  it('suave en un vértice de rectas crea los manejadores con un tercio del segmento', () => {
    const m = polyline([[0, 0], [9, 0], [9, 18]]);
    const n = setNodeType(m, [1], 'smooth')!.model.subs[0].nodes[1];
    expect(n.inH).not.toBeNull();
    expect(n.outH).not.toBeNull();
    expect(inferNodeType(n)).toBe('smooth');
  });

  it('un extremo abierto no tiene qué alinear', () => {
    const m = polyline([[0, 0], [9, 0]]);
    const r = setNodeType(m, [0], 'smooth')!;
    expect(r.model.subs[0].nodes[0].outH).toBeNull();
  });

  it('esquina se recuerda por posición aunque los manejadores sigan alineados', () => {
    const m = parseModel([ellipseBez(0, 0, 10, 10)]);
    const node = m.subs[0].nodes[0];
    expect(nodeType(node)).toBe('symmetric');
    const corners = new Set([positionKey(node.p)]);
    expect(nodeType(node, corners)).toBe('corner');
    expect(rekeyCorners([positionKey(node.p)], [node.p], [[1, 2]])).toEqual([positionKey([1, 2])]);
  });

  it('mover un manejador de un nodo suave arrastra al otro; simétrico, también su longitud; Alt, no', () => {
    const m = smoothPts();
    const sm = setNodeType(m, [1], 'smooth')!.model;
    const n0 = sm.subs[0].nodes[1];
    const lenOut = Math.hypot(n0.outH![0] - n0.p[0], n0.outH![1] - n0.p[1]);
    const moved = moveHandle(sm, 1, 'in', [10, 14], 'smooth').subs[0].nodes[1];
    expect(inferNodeType(moved)).toBe('smooth');
    expect(Math.hypot(moved.outH![0] - moved.p[0], moved.outH![1] - moved.p[1])).toBeCloseTo(lenOut, 9);
    expect(moved.outH![1]).toBeLessThan(10); // el otro lado apunta al lado opuesto
    const sym = moveHandle(sm, 1, 'in', [10, 14], 'symmetric').subs[0].nodes[1];
    expect(Math.hypot(sym.outH![0] - sym.p[0], sym.outH![1] - sym.p[1])).toBeCloseTo(4, 9);
    const alt = moveHandle(sm, 1, 'in', [10, 14], 'smooth', true).subs[0].nodes[1];
    expect(alt.outH).toEqual(n0.outH);
    expect(inferNodeType(alt)).toBe('corner');
    const corner = moveHandle(sm, 1, 'in', [10, 14], 'corner').subs[0].nodes[1];
    expect(corner.outH).toEqual(n0.outH);
  });
});

describe('romper, unir y cerrar', () => {
  it('romper un abierto en un nodo del medio da dos trayectos que se tocan', () => {
    const m = polyline([[0, 0], [5, 0], [10, 0], [10, 10]]);
    expect(canBreak(m, [1])).toBe(true);
    expect(canBreak(m, [0])).toBe(false);
    const r = breakAtNodes(m, [1])!;
    expect(r.model.subs.map((s) => s.nodes.length)).toEqual([2, 3]);
    expect(r.model.subs[0].nodes[1].p).toEqual([5, 0]);
    expect(r.model.subs[1].nodes[0].p).toEqual([5, 0]);
    expect(r.sel).toEqual([2]);
  });

  it('romper conserva la mitad de cada curva', () => {
    const m = insertNode(parseModel([CURVE]), 0, 0, 0.4)!.model;
    const r = breakAtNodes(m, [1])!;
    const [a, b] = r.model.subs;
    expect(a.nodes[1].outH).toBeNull();
    expect(b.nodes[0].inH).toBeNull();
    expect(a.nodes[1].inH).not.toBeNull();
    expect(b.nodes[0].outH).not.toBeNull();
    expect(maxDistance(sample([CURVE]), buildPaths(r.model))).toBeLessThan(1e-3);
  });

  it('romper un cerrado lo abre en ese nodo', () => {
    const m = polyline(SQUARE, true);
    const r = breakAtNodes(m, [2])!;
    expect(r.model.subs).toHaveLength(1);
    const s = r.model.subs[0];
    expect(s.closed).toBe(false);
    expect(s.nodes).toHaveLength(5);
    expect(s.nodes[0].p).toEqual([10, 10]);
    expect(s.nodes[4].p).toEqual([10, 10]);
  });

  it('romper un cerrado en dos nodos da dos trayectos', () => {
    const r = breakAtNodes(polyline(SQUARE, true), [0, 2])!;
    expect(r.model.subs).toHaveLength(2);
    expect(r.model.subs.every((s) => !s.closed)).toBe(true);
  });

  it('un extremo no se rompe', () => {
    expect(breakAtNodes(polyline([[0, 0], [5, 0], [10, 0]]), [0])).toBeNull();
  });

  it('unir dos extremos que coinciden los funde en un nodo', () => {
    const broken = breakAtNodes(polyline([[0, 0], [5, 0], [10, 0]]), [1])!.model;
    expect(canJoin(broken, [1, 2])).toBe(true);
    const r = joinEndpoints(broken, [1, 2])!;
    expect(r.model.subs).toHaveLength(1);
    expect(r.model.subs[0].nodes.map((n) => n.p)).toEqual([[0, 0], [5, 0], [10, 0]]);
    expect(r.sel).toEqual([1]);
  });

  it('unir extremos lejanos los conecta con una recta sin mover nada', () => {
    const m = parseModel([[...polylineBez([[0, 0], [10, 0]], false), ...polylineBez([[20, 0], [30, 0]], false)]]);
    const r = joinEndpoints(m, [1, 2])!;
    expect(r.model.subs[0].nodes.map((n) => n.p)).toEqual([[0, 0], [10, 0], [20, 0], [30, 0]]);
    expect(r.sel).toEqual([1, 2]);
  });

  it('unir invierte el trayecto que haga falta (inicio con inicio)', () => {
    const m = parseModel([[...polylineBez([[0, 0], [10, 0]], false), ...polylineBez([[0, 5], [10, 5]], false)]]);
    const r = joinEndpoints(m, [0, 2])!;
    expect(r.model.subs[0].nodes.map((n) => n.p)).toEqual([[10, 0], [0, 0], [0, 5], [10, 5]]);
  });

  it('unir los dos extremos de un mismo trayecto lo cierra', () => {
    const r = joinEndpoints(polyline([[0, 0], [10, 0], [10, 10]]), [0, 2])!;
    expect(r.model.subs[0].closed).toBe(true);
  });

  it('no une nodos que no son extremos ni más de dos', () => {
    const m = polyline([[0, 0], [5, 0], [10, 0], [15, 0]]);
    expect(canJoin(m, [1, 2])).toBe(false);
    expect(joinEndpoints(m, [0, 1, 3])).toBeNull();
    expect(canJoin(polyline(SQUARE, true), [0, 1])).toBe(false);
  });

  it('cerrar un abierto añade el segmento de cierre; si los extremos coinciden, los funde', () => {
    const open = polyline([[0, 0], [10, 0], [10, 10]]);
    expect(openSubsOf(open, [1])).toEqual([0]);
    const closed = closeSubs(open, [0])!;
    expect(closed.model.subs[0].closed).toBe(true);
    expect(closed.model.subs[0].nodes).toHaveLength(3);
    const loop = parseModel([[...polylineBez([[0, 0], [10, 0], [10, 10], [0, 0]], false)]]);
    // sin `close` sigue abierto aunque termine donde empezó: "cerrar" funde los dos nodos
    expect(loop.subs[0].closed).toBe(false);
    expect(loop.subs[0].nodes).toHaveLength(4);
    expect(closeSubs(loop, [0])!.model.subs[0].nodes).toHaveLength(3);
    const bez = parseModel([[{ type: 'move_to', to: [0, 0] }, { type: 'line_to', to: [5, 0] }, { type: 'line_to', to: [5, 5] }, { type: 'line_to', to: [0, 0.0000001] }]]);
    // extremos que coinciden (dentro de la tolerancia) se funden al cerrar
    const fused = closeSubs(bez, [0])!;
    expect(fused.model.subs[0].closed).toBe(true);
    expect(fused.model.subs[0].nodes).toHaveLength(3);
    expect(closeSubs(closed.model, [0])).toBeNull();
  });

  it('cerrar con una curva de cierre la conserva al reconstruir', () => {
    const m = parseModel([[
      { type: 'move_to', to: [0, 0] },
      { type: 'line_to', to: [10, 0] },
      { type: 'cubic_to', ctrl1: [10, 10], ctrl2: [0, 10], to: [0, 0] },
      { type: 'close' },
    ]]);
    expect(m.subs[0].nodes).toHaveLength(2);
    const out = buildPaths(m)[0];
    expect(out.map((s) => s.type)).toEqual(['move_to', 'line_to', 'cubic_to', 'close']);
  });
});

describe('edición en coordenadas locales con transform', () => {
  const rotatedScaled = (): Affine => {
    const a = (Math.PI / 6);
    // escala 2 en X, 3 en Y, giro de 30° y traslación
    return [2 * Math.cos(a), 2 * Math.sin(a), -3 * Math.sin(a), 3 * Math.cos(a), 40, 25];
  };

  it('la inversa deshace el transform', () => {
    const m = rotatedScaled();
    const inv = invertAffine(m)!;
    const p: Point = [7, -3];
    const back = applyPoint(inv, applyPoint(m, p));
    expect(close(back, p, 1e-9)).toBe(true);
    expect(invertAffine([0, 0, 0, 0, 1, 1])).toBeNull();
  });

  it('mover un nodo a coordenadas de documento exactas cae ahí tras aplicar el transform', () => {
    const t = rotatedScaled();
    const model = polyline(SQUARE, true);
    const inv = invertAffine(t)!;
    const target: Point = [55.5, 31.25];
    const local = applyPoint(inv, target);
    const k = 2;
    const delta: Point = [local[0] - model.subs[0].nodes[k].p[0], local[1] - model.subs[0].nodes[k].p[1]];
    const moved = moveNodes(model, [k], delta);
    expect(close(applyPoint(t, nodeAt(moved, k)!.p), target, 1e-9)).toBe(true);
    // los demás nodos no se movieron
    expect(nodeAt(moved, 0)!.p).toEqual([0, 0]);
  });

  it('el nodo se mueve con sus manejadores (rígido)', () => {
    const m = parseModel([ellipseBez(0, 0, 10, 10)]);
    const moved = moveNodes(m, [0], [1, 2]);
    const a = m.subs[0].nodes[0];
    const b = moved.subs[0].nodes[0];
    expect(b.p).toEqual([a.p[0] + 1, a.p[1] + 2]);
    expect(b.outH).toEqual([a.outH![0] + 1, a.outH![1] + 2]);
    expect(b.inH).toEqual([a.inH![0] + 1, a.inH![1] + 2]);
  });

  it('el segmento más cercano se encuentra en coordenadas de documento y su t sirve en el modelo local', () => {
    const t = rotatedScaled();
    const model = parseModel([CURVE]);
    // punto de la curva a t = 0.3, llevado al documento
    const pt = applyPoint(t, cubicAt([0, 0], [0, 10], [10, 10], [10, 0], 0.3));
    const hit = nearestSegment(model, t, [pt[0] + 0.01, pt[1] - 0.01], 0.5)!;
    expect(hit.s).toBe(0);
    expect(hit.i).toBe(0);
    expect(hit.t).toBeCloseTo(0.3, 2);
    expect(nearestSegment(model, t, [pt[0] + 50, pt[1]], 0.5)).toBeNull();
    // insertar con esa t no cambia la forma
    const r = insertNode(model, hit.s, hit.i, hit.t)!;
    expect(maxDistance(sample([CURVE]), buildPaths(r.model))).toBeLessThan(1e-3);
  });
});
