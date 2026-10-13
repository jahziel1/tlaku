import type { DesignObject, LayerSettings } from '@/api/types';
import type { SliceCreator } from './types';

/** Un objeto oculto, bloqueado o de una capa invisible no se puede seleccionar. */
function selectable(o: DesignObject, layers: Map<string, LayerSettings>): boolean {
  return !o.hidden && !o.locked && (layers.get(o.layer.toLowerCase())?.visible ?? true);
}

export type ToolId = 'select' | 'nodes' | 'line' | 'rect' | 'ellipse' | 'text' | 'pan' | 'measure';

export const TOOL_IDS: readonly ToolId[] = ['select', 'nodes', 'line', 'rect', 'ellipse', 'text', 'pan', 'measure'];

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
}

export const createSelectionSlice: SliceCreator<SelectionSlice> = (set, get) => ({
  selectedIds: [],
  activeTool: 'select',
  activeLayer: null,
  anchor: 6, // abajo a la izquierda: coincide con el origen del documento
  lockRatio: true,
  cornerRadius: 0,

  select: (ids) => set({ selectedIds: [...new Set(ids)] }),
  toggleSelected: (id) =>
    set((s) => ({
      selectedIds: s.selectedIds.includes(id) ? s.selectedIds.filter((x) => x !== id) : [...s.selectedIds, id],
    })),
  clearSelection: () => set((s) => (s.selectedIds.length ? { selectedIds: [] } : s)),
  selectAll: () =>
    set((s) => {
      const layers = layerMap(s.doc.layers);
      return { selectedIds: s.doc.objects.filter((o) => selectable(o, layers)).map((o) => o.id) };
    }),
  pruneSelection: () => {
    const { doc, selectedIds } = get();
    if (selectedIds.length === 0) return;
    const layers = layerMap(doc.layers);
    const ok = new Set(doc.objects.filter((o) => selectable(o, layers)).map((o) => o.id));
    const kept = selectedIds.filter((id) => ok.has(id));
    if (kept.length !== selectedIds.length) set({ selectedIds: kept });
  },
  selectLayerObjects: (color, additive = false) => {
    const { doc } = get();
    const layers = layerMap(doc.layers);
    const ids = doc.objects
      .filter((o) => o.layer.toLowerCase() === color.toLowerCase() && selectable(o, layers))
      .map((o) => o.id);
    set((s) => ({ selectedIds: additive ? [...new Set([...s.selectedIds, ...ids])] : ids }));
  },
  setTool: (activeTool) => set({ activeTool }),
  setActiveLayer: (activeLayer) => set({ activeLayer }),
  setAnchor: (anchor) => set({ anchor: Math.min(8, Math.max(0, Math.round(anchor))) }),
  setLockRatio: (lockRatio) => set({ lockRatio }),
  setCornerRadius: (mm) => set({ cornerRadius: Math.max(0, Number.isFinite(mm) ? mm : 0) }),
});

function layerMap(layers: LayerSettings[]): Map<string, LayerSettings> {
  return new Map(layers.map((l) => [l.color.toLowerCase(), l]));
}
