import { create } from 'zustand';
import type { VirtualArray } from '@/api/types';
import { DEFAULT_ALONG, type AlongPathParams } from './alongPath';
import { DEFAULT_DOCK, type DockParams } from './dock';
import { DEFAULT_WARP_AMOUNT, NO_WARP, type WarpParams } from './warpPresets';

/** Diálogos de diseño avanzado (ola 3). Viven aparte de `useArrangeUi` para no pisar su estado. */
export type DesignDialog = 'warp' | 'hull' | 'tangent' | 'boolAssistant' | 'alongPath' | 'virtualArray' | 'dock';

export interface HullParams {
  /** Casco cóncavo (con `maxEdge`) en vez del convexo. */
  concave: boolean;
  /** Largo máximo (mm) de las aristas del casco cóncavo. */
  maxEdge: number;
  keepOriginal: boolean;
}

export interface VirtualDraft {
  kind: VirtualArray['type'];
  cols: number;
  rows: number;
  dx: number;
  dy: number;
  count: number;
  centerX: number;
  centerY: number;
  sweep: number;
  rotate: boolean;
}

export const DEFAULT_VIRTUAL: VirtualDraft = { kind: 'grid', cols: 3, rows: 3, dx: 5, dy: 5, count: 6, centerX: 0, centerY: 0, sweep: 360, rotate: true };

/** Del dibujo guardado en el objeto al formulario. */
export function draftOf(array: VirtualArray, base: VirtualDraft = DEFAULT_VIRTUAL): VirtualDraft {
  return array.type === 'grid'
    ? { ...base, kind: 'grid', cols: array.cols, rows: array.rows, dx: array.dx, dy: array.dy }
    : { ...base, kind: 'circular', count: array.count, centerX: array.center[0], centerY: array.center[1], sweep: array.sweep_deg, rotate: array.rotate };
}

/** Del formulario al contrato. */
export function arrayOf(d: VirtualDraft): VirtualArray {
  return d.kind === 'grid'
    ? { type: 'grid', cols: Math.round(d.cols), rows: Math.round(d.rows), dx: d.dx, dy: d.dy }
    : { type: 'circular', count: Math.round(d.count), center: [d.centerX, d.centerY], sweep_deg: d.sweep, rotate: d.rotate };
}

interface DesignUi {
  dialog: DesignDialog | null;
  busy: boolean;
  warp: { params: WarpParams; amount: number };
  hull: HullParams;
  /** Radio del círculo tangente con dos formas. */
  tangentRadius: number;
  along: AlongPathParams;
  /** Unidad que hace de trayecto (por defecto, la última de la selección). */
  alongPathId: string | null;
  dock: DockParams;
  /** Unidad que se queda quieta al acoplar (por defecto, la primera). */
  dockRefId: string | null;
  virtual: VirtualDraft;
  /** Operación elegida por última vez en el asistente booleano. */
  assistantPrimaryId: string | null;

  open: (dialog: DesignDialog) => void;
  close: () => void;
  setBusy: (busy: boolean) => void;
  patchWarp: (patch: Partial<{ params: WarpParams; amount: number }>) => void;
  patchHull: (patch: Partial<HullParams>) => void;
  setTangentRadius: (radius: number) => void;
  patchAlong: (patch: Partial<AlongPathParams>) => void;
  setAlongPath: (id: string | null) => void;
  patchDock: (patch: Partial<DockParams>) => void;
  setDockRef: (id: string | null) => void;
  patchVirtual: (patch: Partial<VirtualDraft>) => void;
  setAssistantPrimary: (id: string | null) => void;
}

export const useDesignUi = create<DesignUi>()((set) => ({
  dialog: null,
  busy: false,
  warp: { params: NO_WARP, amount: DEFAULT_WARP_AMOUNT },
  hull: { concave: false, maxEdge: 20, keepOriginal: true },
  tangentRadius: 10,
  along: DEFAULT_ALONG,
  alongPathId: null,
  dock: DEFAULT_DOCK,
  dockRefId: null,
  virtual: DEFAULT_VIRTUAL,
  assistantPrimaryId: null,

  open: (dialog) => set({ dialog }),
  close: () => set({ dialog: null }),
  setBusy: (busy) => set({ busy }),
  patchWarp: (patch) => set((s) => ({ warp: { ...s.warp, ...patch } })),
  patchHull: (patch) => set((s) => ({ hull: { ...s.hull, ...patch } })),
  setTangentRadius: (tangentRadius) => set({ tangentRadius }),
  patchAlong: (patch) => set((s) => ({ along: { ...s.along, ...patch } })),
  setAlongPath: (alongPathId) => set({ alongPathId }),
  patchDock: (patch) => set((s) => ({ dock: { ...s.dock, ...patch } })),
  setDockRef: (dockRefId) => set({ dockRefId }),
  patchVirtual: (patch) => set((s) => ({ virtual: { ...s.virtual, ...patch } })),
  setAssistantPrimary: (assistantPrimaryId) => set({ assistantPrimaryId }),
}));
