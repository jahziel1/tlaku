import type { DesignObject, LayerSettings } from '@/api/types';
import { nodeCount, parseModel } from '@/features/canvas/nodeedit';
import { readPref, writePref } from '@/lib/platform';
import { objectIndex } from './objectIndex';
import type { SliceCreator } from './types';

/** Un objeto oculto, bloqueado o de una capa invisible no se puede seleccionar. */
function selectable(o: DesignObject, layers: Map<string, LayerSettings>): boolean {
  return !o.hidden && !o.locked && (layers.get(o.layer.toLowerCase())?.visible ?? true);
}

export type ToolId = 'select' | 'nodes' | 'line' | 'rect' | 'ellipse' | 'polygon' | 'star' | 'text' | 'pan' | 'measure' | 'tabs' | 'start' | 'trim' | 'warp' | 'fillet';

export const TOOL_IDS: readonly ToolId[] = ['select', 'nodes', 'line', 'rect', 'ellipse', 'polygon', 'star', 'text', 'pan', 'measure', 'tabs', 'start', 'trim', 'warp', 'fillet'];

/** Interruptores globales de transformación (para evitar accidentes): cada tipo se puede apagar. */
export interface TransformAllow {
  move: boolean;
  scale: boolean;
  rotate: boolean;
}

export const MIN_POLYGON_SIDES = 3;
export const MAX_POLYGON_SIDES = 100;

export interface SelectionSlice {
  selectedIds: string[];
  activeTool: ToolId;
  /** Color de capa para lo que se dibuje a continuación. */
  activeLayer: string | null;
  /** Punto de referencia de la barra numérica (0..8, fila por fila desde arriba a la izquierda). */
  anchor: number;
  /** Candado de proporción de la barra numérica. */
  lockRatio: boolean;
  /** Radio de esquinas (mm) con el que se dibujan los rectángulos nuevos. */
  cornerRadius: number;
  /** Lados con los que se dibujan los polígonos nuevos. */
  polygonSides: number;
  /** Puntas de las estrellas nuevas. */
  starPoints: number;
  /** Radio interno de la estrella como fracción del externo (0.05 a 0.95). */
  starRatio: number;
  /** Redondeo (mm) de las esquinas de los polígonos y estrellas nuevos. */
  shapeRounding: number;
  /** Interruptores de mover, escalar y girar (apagados, esa transformación no se aplica). */
  transformAllow: TransformAllow;
  /** Escalar siempre desde el centro (como mantener Alt al arrastrar una manija). */
  scaleFromCenter: boolean;
  /** Ventana de propiedades de forma visible (aparece cuando la selección son formas paramétricas). */
  shapePropsOpen: boolean;
  /** Muestra sobre el lienzo la prioridad de corte de los objetos que la tienen (Ver). */
  priorityBadges: boolean;
  /**
   * Nodos seleccionados con la herramienta de nodos: por id de objeto, índices planos de nodo
   * (ver `features/canvas/nodeedit`). Solo cuenta lo de objetos que siguen seleccionados.
   */
  nodeSel: Record<string, number[]>;
  /** Nodos que se pidieron "esquina" aunque sus manejadores estén alineados: claves de posición local por objeto. */
  nodeCorners: Record<string, string[]>;

  select: (ids: string[]) => void;
  toggleSelected: (id: string) => void;
  clearSelection: () => void;
  selectAll: () => void;
  /** Quita de la selección lo que ya no existe en el documento. */
  pruneSelection: () => void;
  selectLayerObjects: (color: string, additive?: boolean) => void;
  setTool: (tool: ToolId) => void;
  setActiveLayer: (color: string | null) => void;
  setAnchor: (anchor: number) => void;
  setLockRatio: (lock: boolean) => void;
  setCornerRadius: (mm: number) => void;
  setPolygonSides: (n: number) => void;
  setStarPoints: (n: number) => void;
  setStarRatio: (ratio: number) => void;
  setShapeRounding: (mm: number) => void;
  setTransformAllow: (patch: Partial<TransformAllow>) => void;
  setScaleFromCenter: (on: boolean) => void;
  setShapePropsOpen: (open: boolean) => void;
  setPriorityBadges: (on: boolean) => void;
  setNodeSel: (sel: Record<string, number[]>) => void;
  clearNodeSel: () => void;
  setNodeCorners: (id: string, keys: string[]) => void;
}

/** Deja solo la selección de nodos de los objetos indicados. */
function keepNodeSel(sel: Record<string, number[]>, ids: readonly string[]): Record<string, number[]> {
  const keys = Object.keys(sel);
  if (keys.every((k) => ids.includes(k))) return sel;
  return Object.fromEntries(keys.filter((k) => ids.includes(k)).map((k) => [k, sel[k]!]));
}

export const createSelectionSlice: SliceCreator<SelectionSlice> = (set, get) => ({
  selectedIds: [],
  activeTool: 'select',
  activeLayer: null,
  anchor: 6, // abajo a la izquierda: coincide con el origen del documento
  lockRatio: true,
  cornerRadius: 0,
  polygonSides: 6,
  starPoints: 5,
  starRatio: 0.5,
  shapeRounding: 0,
  transformAllow: { move: true, scale: true, rotate: true },
  scaleFromCenter: false,
  // Cerrada al arrancar: no tapa el lienzo; se abre con Ver > Propiedades del objeto.
  shapePropsOpen: false,
  priorityBadges: readPref('priorityBadges') === '1',
  nodeSel: {},
  nodeCorners: {},

  select: (ids) => {
    const unique = [...new Set(ids)];
    set((s) => ({ selectedIds: unique, nodeSel: keepNodeSel(s.nodeSel, unique) }));
  },
  toggleSelected: (id) =>
    set((s) => {
      const selectedIds = s.selectedIds.includes(id) ? s.selectedIds.filter((x) => x !== id) : [...s.selectedIds, id];
      return { selectedIds, nodeSel: keepNodeSel(s.nodeSel, selectedIds) };
    }),
  clearSelection: () =>
    set((s) => (s.selectedIds.length || Object.keys(s.nodeSel).length ? { selectedIds: [], nodeSel: {} } : s)),
  selectAll: () =>
    set((s) => {
      const layers = layerMap(s.doc.layers);
      const selectedIds = s.doc.objects.filter((o) => selectable(o, layers)).map((o) => o.id);
      return { selectedIds, nodeSel: keepNodeSel(s.nodeSel, selectedIds) };
    }),
  pruneSelection: () => {
    const { doc, selectedIds, nodeSel } = get();
    if (selectedIds.length === 0) {
      if (Object.keys(nodeSel).length) set({ nodeSel: {} });
      return;
    }
    const layers = layerMap(doc.layers);
    const idx = objectIndex(doc.objects);
    let kept: string[];
    if (idx.unique) {
      // Por índice de ids: solo se miran los seleccionados, no los 100 000 objetos del documento.
      kept = selectedIds.filter((id) => {
        const i = idx.at.get(id);
        return i !== undefined && selectable(doc.objects[i]!, layers);
      });
    } else {
      const ok = new Set(doc.objects.filter((o) => selectable(o, layers)).map((o) => o.id));
      kept = selectedIds.filter((id) => ok.has(id));
    }
    if (kept.length !== selectedIds.length) set({ selectedIds: kept });
    if (Object.keys(nodeSel).length === 0) return;
    // Deshacer, rehacer o borrar pueden cambiar cuántos nodos tiene un objeto: lo que ya no existe se descarta.
    const nodes = pruneNodeSel(nodeSel, doc.objects, new Set(kept));
    if (nodes !== nodeSel) set({ nodeSel: nodes });
  },
  selectLayerObjects: (color, additive = false) => {
    const { doc } = get();
    const layers = layerMap(doc.layers);
    const ids = doc.objects
      .filter((o) => o.layer.toLowerCase() === color.toLowerCase() && selectable(o, layers))
      .map((o) => o.id);
    set((s) => {
      const selectedIds = additive ? [...new Set([...s.selectedIds, ...ids])] : ids;
      return { selectedIds, nodeSel: keepNodeSel(s.nodeSel, selectedIds) };
    });
  },
  // Salir de la herramienta de nodos suelta sus nodos seleccionados.
  setTool: (activeTool) => set((s) => (s.activeTool === activeTool ? s : { activeTool, nodeSel: activeTool === 'nodes' ? s.nodeSel : {} })),
  setActiveLayer: (activeLayer) => set({ activeLayer }),
  setAnchor: (anchor) => set({ anchor: Math.min(8, Math.max(0, Math.round(anchor))) }),
  setLockRatio: (lockRatio) => set({ lockRatio }),
  setCornerRadius: (mm) => set({ cornerRadius: Math.max(0, Number.isFinite(mm) ? mm : 0) }),
  setPolygonSides: (n) =>
    set({ polygonSides: Math.min(MAX_POLYGON_SIDES, Math.max(MIN_POLYGON_SIDES, Math.round(Number.isFinite(n) ? n : 6))) }),
  setStarPoints: (n) =>
    set({ starPoints: Math.min(MAX_POLYGON_SIDES, Math.max(MIN_POLYGON_SIDES, Math.round(Number.isFinite(n) ? n : 5))) }),
  setStarRatio: (ratio) => set({ starRatio: Math.min(0.95, Math.max(0.05, Number.isFinite(ratio) ? ratio : 0.5)) }),
  setShapeRounding: (mm) => set({ shapeRounding: Math.max(0, Number.isFinite(mm) ? mm : 0) }),
  setTransformAllow: (patch) => set((s) => ({ transformAllow: { ...s.transformAllow, ...patch } })),
  setScaleFromCenter: (scaleFromCenter) => set({ scaleFromCenter }),
  setShapePropsOpen: (shapePropsOpen) => set({ shapePropsOpen }),
  setPriorityBadges: (priorityBadges) => {
    writePref('priorityBadges', priorityBadges ? '1' : '0');
    set({ priorityBadges });
  },
  setNodeSel: (sel) => set({ nodeSel: Object.fromEntries(Object.entries(sel).filter(([, v]) => v.length > 0)) }),
  clearNodeSel: () => set((s) => (Object.keys(s.nodeSel).length ? { nodeSel: {} } : s)),
  setNodeCorners: (id, keys) =>
    set((s) => {
      const next = { ...s.nodeCorners };
      if (keys.length) next[id] = [...new Set(keys)];
      else delete next[id];
      return { nodeCorners: next };
    }),
});

/** Quita de la selección de nodos los objetos que ya no están o no son vectores y los índices fuera de rango. */
export function pruneNodeSel(sel: Record<string, number[]>, objects: readonly DesignObject[], alive: ReadonlySet<string>): Record<string, number[]> {
  const ids = Object.keys(sel);
  if (ids.length === 0) return sel;
  let changed = false;
  const out: Record<string, number[]> = {};
  for (const id of ids) {
    const obj = alive.has(id) ? objects.find((o) => o.id === id) : undefined;
    if (!obj || obj.kind.type !== 'vector') {
      changed = true;
      continue;
    }
    const n = nodeCount(parseModel(obj.kind.paths));
    const kept = sel[id]!.filter((k) => k >= 0 && k < n);
    if (kept.length !== sel[id]!.length) changed = true;
    if (kept.length) out[id] = kept;
    else changed = true;
  }
  return changed ? out : sel;
}

function layerMap(layers: LayerSettings[]): Map<string, LayerSettings> {
  return new Map(layers.map((l) => [l.color.toLowerCase(), l]));
}
