import type { DesignObject, Point } from '@/api/types';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { worldToScreen, type ViewTransform } from '@/lib/viewport';
import type { AppState } from '@/store';
import { clearGesture, patchInteraction, useInteraction, type NodePreview } from './interaction';
import { insertNodeAt, nodeTargets, replacePaths, type NodeTarget } from './nodeActions';
import {
  applyPoint, applyVector, buildPaths, moveHandle, moveNodes, nearestSegment, nodeAt, nodeType, positionKey, positionsOf,
  rekeyCorners, type SegmentHit,
} from './nodeedit';
import { buildNodeOverlay, isNodeEditable, modelOf, type HandleItem, type NodeItem, type NodeOverlay } from './nodeview';
import { normalizeBox } from './hit';
import { constrainAngle, constrainDelta } from './shapes';
import { NO_SNAP, snapPoint, type SnapOptions, type SnapSet } from './snap';

/** Radio de captura de un nodo y de un manejador, en px de pantalla. */
export const NODE_HIT_PX = 8;
export const HANDLE_HIT_PX = 7;
/** Distancia en px antes de que un clic se convierta en arrastre. */
const DRAG_PX = 3;
/** Tolerancia de clic sobre un segmento, en px. */
const SEGMENT_HIT_PX = 6;

export interface Mods {
  shift: boolean;
  alt: boolean;
  ctrl: boolean;
}

export interface NodeToolDeps {
  el: HTMLElement;
  st: () => AppState;
  viewOf: () => ViewTransform;
  world: (sx: number, sy: number) => Point;
  capture: (e: PointerEvent) => void;
  release: (id: number) => void;
  snapOpts: (mods: Mods, suspendWithAlt?: boolean) => SnapOptions | null;
  targetsFor: (skip: ReadonlySet<string>) => SnapSet;
  setTip: (sx: number, sy: number, text: string) => void;
  /** Objeto bajo el punto (mm) o null: para pasar a editar los nodos de otro. */
  pick: (p: Point, tolMM: number) => DesignObject | null;
}

type What =
  | { t: 'node'; id: string; k: number; wasSelected: boolean; shift: boolean }
  | { t: 'handle'; id: string; k: number; which: 'in' | 'out' }
  | { t: 'segment'; id: string; hit: SegmentHit; shift: boolean }
  | { t: 'object'; obj: DesignObject }
  | { t: 'empty'; shift: boolean };

interface Gesture {
  pointerId: number;
  startScreen: Point;
  startWorld: Point;
  active: boolean;
  what: What;
  /** Objetos que se arrastran (nodos): se fijan al activarse el arrastre. */
  entries: NodeTarget[];
  /** Posición de partida (documento) de lo que se agarró: nodo o manejador. */
  grabbed: Point;
  targets: SnapSet;
  /** Resultado vivo; se guarda al soltar. */
  preview: Record<string, NodePreview> | null;
  /** Se rompió la simetría de un nodo con Alt. */
  broke: boolean;
}

const sameSel = (a: readonly number[], b: readonly number[]) => a.length === b.length && a.every((v, i) => v === b[i]);

/**
 * Herramienta de nodos del lienzo: elegir, arrastrar y borrar nodos, mover manejadores, caja de
 * selección y doble clic para agregar un nodo. Como las demás herramientas, un gesto edita una copia
 * viva (`nodePreview`) y escribe UNA vez en el documento al soltar; Esc lo cancela sin rastro.
 */
export function createNodeTool(d: NodeToolDeps) {
  let g: Gesture | null = null;

  const overlay = (): NodeOverlay => {
    const s = d.st();
    const ix = useInteraction.getState();
    return buildNodeOverlay({
      objects: s.doc.objects, selectedIds: s.selectedIds, nodeSel: s.nodeSel, nodeCorners: s.nodeCorners,
      preview: ix.nodePreview, hover: ix.nodeHover,
    });
  };

  const screenOf = (x: number, y: number): Point => worldToScreen(d.viewOf(), [x, y]);

  function hitHandle(sx: number, sy: number): HandleItem | null {
    let best: HandleItem | null = null;
    let bestD = HANDLE_HIT_PX;
    for (const h of overlay().handles) {
      const [px, py] = screenOf(h.hx, h.hy);
      const dist = Math.hypot(px - sx, py - sy);
      if (dist <= bestD) {
        bestD = dist;
        best = h;
      }
    }
    return best;
  }

  function hitNode(sx: number, sy: number): NodeItem | null {
    let best: NodeItem | null = null;
    let bestD = NODE_HIT_PX;
    for (const n of overlay().nodes) {
      const [px, py] = screenOf(n.x, n.y);
      const dist = Math.hypot(px - sx, py - sy);
      // A igual distancia gana el seleccionado (el de arriba al pintar).
      if (dist < bestD || (dist === bestD && n.selected)) {
        bestD = dist;
        best = n;
      }
    }
    return best;
  }

  function hitSegment(w: Point): { id: string; hit: SegmentHit } | null {
    const s = d.st();
    const tol = SEGMENT_HIT_PX / s.zoom;
    let best: { id: string; hit: SegmentHit } | null = null;
    for (const id of s.selectedIds) {
      const obj = s.doc.objects.find((o) => o.id === id);
      if (!obj || !isNodeEditable(obj)) continue;
      const model = modelOf(obj);
      const hit = model && nearestSegment(model, obj.transform, w, tol);
      if (hit && (!best || hit.dist < best.hit.dist)) best = { id, hit };
    }
    return best;
  }

  const setSel = (sel: Record<string, number[]>) => d.st().setNodeSel(sel);

  // ---- abrir ---------------------------------------------------------------------------------------------

  function down(e: PointerEvent, sx: number, sy: number, mods: Mods): void {
    const s = d.st();
    const startWorld = d.world(sx, sy);
    let what: What;
    const handle = hitHandle(sx, sy);
    const node = handle ? null : hitNode(sx, sy);
    if (handle) {
      what = { t: 'handle', id: handle.id, k: handle.k, which: handle.which };
    } else if (node) {
      const was = (s.nodeSel[node.id] ?? []).includes(node.k);
      what = { t: 'node', id: node.id, k: node.k, wasSelected: was, shift: mods.shift };
      if (!was) {
        // Un nodo nuevo se elige al apretar (así se puede arrastrar de una vez); Mayús lo suma.
        const cur = mods.shift ? s.nodeSel : {};
        setSel({ ...cur, [node.id]: [...(cur[node.id] ?? []), node.k].sort((a, b) => a - b) });
      }
    } else {
      const seg = hitSegment(startWorld);
      if (seg) {
        what = { t: 'segment', id: seg.id, hit: seg.hit, shift: mods.shift };
      } else {
        const picked = d.pick(startWorld, SEGMENT_HIT_PX / s.zoom);
        what = picked && !s.selectedIds.includes(picked.id) ? { t: 'object', obj: picked } : { t: 'empty', shift: mods.shift };
      }
    }
    g = {
      pointerId: e.pointerId, startScreen: [sx, sy], startWorld, active: false, what, entries: [], grabbed: startWorld,
      targets: { xs: [], ys: [] }, preview: null, broke: false,
    };
    patchInteraction({ nodeHover: null });
    d.capture(e);
  }

  // ---- actualizar ----------------------------------------------------------------------------------------------

  function activate(gs: Gesture): void {
    gs.active = true;
    const w = gs.what;
    if (w.t === 'node' || w.t === 'handle') {
      const s = d.st();
      gs.entries = nodeTargets(s);
      const entry = gs.entries.find((t) => t.obj.id === w.id);
      const n = entry && nodeAt(entry.model, w.k);
      if (!entry || !n) {
        g = null;
        return;
      }
      const at = w.t === 'handle' ? (w.which === 'in' ? n.inH : n.outH) : n.p;
      gs.grabbed = applyPoint(entry.obj.transform, at ?? n.p);
      gs.targets = d.targetsFor(new Set(gs.entries.map((t) => t.obj.id)));
      patchInteraction({ gesture: 'nodes' });
    } else if (w.t === 'segment') {
      // Arrastrar desde un segmento no edita nada: se ignora hasta soltar.
      gs.entries = [];
    } else {
      patchInteraction({ gesture: 'nodes' });
    }
  }

  function update(gs: Gesture, sx: number, sy: number, mods: Mods): void {
    const w = d.world(sx, sy);
    const what = gs.what;
    if (what.t === 'node' || what.t === 'handle') {
      let dx = w[0] - gs.startWorld[0];
      let dy = w[1] - gs.startWorld[1];
      let guides = NO_SNAP.guides;
      const entry = gs.entries.find((t) => t.obj.id === what.id);
      if (!entry) return;
      if (what.t === 'handle') {
        const node = nodeAt(entry.model, what.k)!;
        let target: Point = [gs.grabbed[0] + dx, gs.grabbed[1] + dy];
        if (mods.shift) target = constrainAngle(applyPoint(entry.obj.transform, node.p), target);
        const opts = d.snapOpts(mods, false);
        if (opts) {
          const r = snapPoint(target, gs.targets, opts);
          target = [target[0] + r.dx, target[1] + r.dy];
          guides = r.guides;
        }
        const corners = new Set(d.st().nodeCorners[what.id] ?? []);
        const type = nodeType(node, corners);
        const model = moveHandle(entry.model, what.k, what.which, applyPoint(entry.inv, target), type, mods.alt);
        gs.broke = mods.alt && type !== 'corner';
        gs.preview = { [what.id]: { model, paths: buildPaths(model) } };
        patchInteraction({ nodePreview: gs.preview, guides });
        d.setTip(sx, sy, t('nodes.tip.point', { x: fmt(target[0], 2), y: fmt(target[1], 2) }));
        return;
      }
      if (mods.shift) [dx, dy] = constrainDelta(dx, dy);
      const opts = d.snapOpts(mods);
      if (opts) {
        const r = snapPoint([gs.grabbed[0] + dx, gs.grabbed[1] + dy], gs.targets, opts, mods.shift ? { x: dy === 0, y: dx === 0 } : { x: true, y: true });
        dx += r.dx;
        dy += r.dy;
        guides = r.guides;
      }
      const preview: Record<string, NodePreview> = {};
      for (const e of gs.entries) {
        const model = moveNodes(e.model, e.sel, applyVector(e.inv, [dx, dy]));
        preview[e.obj.id] = { model, paths: buildPaths(model) };
      }
      gs.preview = preview;
      patchInteraction({ nodePreview: preview, guides });
      d.setTip(sx, sy, t('nodes.tip.point', { x: fmt(gs.grabbed[0] + dx, 2), y: fmt(gs.grabbed[1] + dy, 2) }));
      return;
    }
    if (what.t === 'segment') return;
    patchInteraction({ marquee: { a: gs.startWorld, b: w, mode: 'window' } });
  }

  // ---- soltar ---------------------------------------------------------------------------------------------------

  function commitPreview(gs: Gesture): void {
    const s = d.st();
    const pv = gs.preview;
    if (!pv) return;
    const changes = new Map<string, ReturnType<typeof buildPaths>>();
    const rekeys: [string, string[]][] = [];
    for (const e of gs.entries) {
      const p = pv[e.obj.id];
      if (!p) continue;
      changes.set(e.obj.id, p.paths);
      const keys = s.nodeCorners[e.obj.id] ?? [];
      if (gs.what.t === 'node' && keys.length) rekeys.push([e.obj.id, rekeyCorners(keys, positionsOf(e.model, e.sel), positionsOf(p.model, e.sel))]);
      if (gs.what.t === 'handle' && gs.broke && e.obj.id === gs.what.id) {
        const n = nodeAt(p.model, gs.what.k);
        if (n) rekeys.push([e.obj.id, [...new Set([...keys, positionKey(n.p)])]]);
      }
    }
    s.applyDocChange(gs.what.t === 'handle' ? 'nodes.history.handle' : 'nodes.history.move', (doc) => replacePaths(doc, changes));
    for (const [id, keys] of rekeys) s.setNodeCorners(id, keys);
  }

  function up(e: PointerEvent, sx: number, sy: number, mods: Mods): void {
    const gs = g;
    if (!gs || e.pointerId !== gs.pointerId) return;
    d.release(gs.pointerId);
    if (gs.active) update(gs, sx, sy, mods);
    g = null;
    const what = gs.what;
    if (gs.active && (what.t === 'node' || what.t === 'handle')) {
      commitPreview(gs);
    } else if (gs.active && what.t === 'empty') {
      selectBox(normalizeBox(gs.startWorld, d.world(sx, sy)), what.shift || mods.shift);
    } else if (gs.active && what.t === 'object') {
      selectBox(normalizeBox(gs.startWorld, d.world(sx, sy)), false);
    } else if (!gs.active) {
      click(what, mods);
    }
    clearGesture();
    patchInteraction({ nodeHover: null });
  }

  /** Clic sin arrastre. */
  function click(what: What, mods: Mods): void {
    const s = d.st();
    switch (what.t) {
      case 'node': {
        if (!what.wasSelected) return; // ya se eligió al apretar
        const mine = s.nodeSel[what.id] ?? [];
        if (what.shift || mods.shift) {
          const rest = mine.filter((k) => k !== what.k);
          setSel({ ...s.nodeSel, [what.id]: rest });
        } else if (!sameSel(mine, [what.k]) || Object.keys(s.nodeSel).length > 1) {
          setSel({ [what.id]: [what.k] });
        }
        return;
      }
      case 'segment': {
        // Un clic en un segmento elige sus dos nodos (así se le puede dar recta, curva o un nodo en medio).
        const obj = s.doc.objects.find((o) => o.id === what.id);
        const model = obj && modelOf(obj);
        if (!model) return;
        const sb = model.subs[what.hit.s];
        let base = 0;
        for (let j = 0; j < what.hit.s; j++) base += model.subs[j].nodes.length;
        const a = base + what.hit.i;
        const b = base + ((what.hit.i + 1) % sb.nodes.length);
        const cur = what.shift ? (s.nodeSel[what.id] ?? []) : [];
        setSel({ ...(what.shift ? s.nodeSel : {}), [what.id]: [...new Set([...cur, a, b])].sort((x, y) => x - y) });
        return;
      }
      case 'object':
        // Otro objeto: se pasa a editar sus nodos.
        s.select([what.obj.id]);
        s.clearNodeSel();
        return;
      case 'empty':
        if (!what.shift) s.clearNodeSel();
        return;
      case 'handle':
        return;
    }
  }

  /** Nodos de los objetos seleccionados que caen dentro de la caja. */
  function selectBox(box: [number, number, number, number], additive: boolean): void {
    const s = d.st();
    const found: Record<string, number[]> = additive ? Object.fromEntries(Object.entries(s.nodeSel).map(([k, v]) => [k, [...v]])) : {};
    for (const n of overlay().nodes) {
      if (n.x < box[0] || n.x > box[2] || n.y < box[1] || n.y > box[3]) continue;
      const list = (found[n.id] ??= []);
      if (!list.includes(n.k)) list.push(n.k);
    }
    for (const v of Object.values(found)) v.sort((a, b) => a - b);
    setSel(found);
  }

  // ---- hover y doble clic -------------------------------------------------------------------------------------------

  /** Cursor y resalte según lo que hay bajo el puntero. */
  function hover(sx: number, sy: number): void {
    const handle = hitHandle(sx, sy);
    const node = handle ? null : hitNode(sx, sy);
    const cur = useInteraction.getState().nodeHover;
    const next = handle ? { id: handle.id, k: handle.k, handle: handle.which } : node ? { id: node.id, k: node.k, handle: null } : null;
    const same = (cur === null && next === null) || (cur && next && cur.id === next.id && cur.k === next.k && cur.handle === next.handle);
    const onSeg = !next && hitSegment(d.world(sx, sy)) !== null;
    patchInteraction({ cursor: next ? 'move' : onSeg ? 'pointer' : 'default', ...(same ? {} : { nodeHover: next }) });
  }

  /** Doble clic sobre un segmento: agrega un nodo ahí sin cambiar la forma. Devuelve `true` si lo hizo. */
  function doubleClick(sx: number, sy: number): boolean {
    if (hitHandle(sx, sy) || hitNode(sx, sy)) return false;
    const seg = hitSegment(d.world(sx, sy));
    if (!seg) return false;
    return insertNodeAt(d.st(), seg.id, seg.hit);
  }

  return {
    down,
    /** `true` si el movimiento era de un gesto de nodos. */
    move(e: PointerEvent, sx: number, sy: number, mods: Mods): boolean {
      const gs = g;
      if (!gs || e.pointerId !== gs.pointerId) return false;
      if (!gs.active) {
        if (Math.hypot(sx - gs.startScreen[0], sy - gs.startScreen[1]) < DRAG_PX) return true;
        activate(gs);
      }
      if (g) update(g, sx, sy, mods);
      return true;
    },
    up(e: PointerEvent, sx: number, sy: number, mods: Mods): boolean {
      if (!g || e.pointerId !== g.pointerId) return false;
      up(e, sx, sy, mods);
      return true;
    },
    /** Mayús, Alt o Ctrl cambiaron a mitad de un arrastre. */
    refresh(sx: number, sy: number, mods: Mods): void {
      if (g?.active) update(g, sx, sy, mods);
    },
    active: () => g !== null,
    cancel(): void {
      if (g) d.release(g.pointerId);
      g = null;
    },
    hover,
    doubleClick,
  };
}

export type NodeTool = ReturnType<typeof createNodeTool>;
