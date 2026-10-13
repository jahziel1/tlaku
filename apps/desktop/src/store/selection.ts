import type { SliceCreator } from './types';

export type ToolId = 'select' | 'nodes' | 'line' | 'rect' | 'ellipse' | 'text' | 'pan' | 'measure';

export const TOOL_IDS: readonly ToolId[] = ['select', 'nodes', 'line', 'rect', 'ellipse', 'text', 'pan', 'measure'];

export interface SelectionSlice {
  selectedIds: string[];
  activeTool: ToolId;
  /** Color de capa para lo que se dibuje a continuación. */
  activeLayer: string | null;

  select: (ids: string[]) => void;
  toggleSelected: (id: string) => void;
  clearSelection: () => void;
  selectAll: () => void;
  /** Quita de la selección lo que ya no existe en el documento. */
  pruneSelection: () => void;
  selectLayerObjects: (color: string, additive?: boolean) => void;
  setTool: (tool: ToolId) => void;
  setActiveLayer: (color: string | null) => void;
}

export const createSelectionSlice: SliceCreator<SelectionSlice> = (set, get) => ({
  selectedIds: [],
  activeTool: 'select',
  activeLayer: null,

  select: (ids) => set({ selectedIds: [...new Set(ids)] }),
  toggleSelected: (id) =>
    set((s) => ({
      selectedIds: s.selectedIds.includes(id) ? s.selectedIds.filter((x) => x !== id) : [...s.selectedIds, id],
    })),
  clearSelection: () => set((s) => (s.selectedIds.length ? { selectedIds: [] } : s)),
  selectAll: () =>
    set((s) => ({ selectedIds: s.doc.objects.filter((o) => !o.hidden && !o.locked).map((o) => o.id) })),
  pruneSelection: () => {
    const exist = new Set(get().doc.objects.map((o) => o.id));
    const kept = get().selectedIds.filter((id) => exist.has(id));
    if (kept.length !== get().selectedIds.length) set({ selectedIds: kept });
  },
  selectLayerObjects: (color, additive = false) => {
    const ids = get()
      .doc.objects.filter((o) => o.layer === color && !o.hidden && !o.locked)
      .map((o) => o.id);
    set((s) => ({ selectedIds: additive ? [...new Set([...s.selectedIds, ...ids])] : ids }));
  },
  setTool: (activeTool) => set({ activeTool }),
  setActiveLayer: (activeLayer) => set({ activeLayer }),
});
