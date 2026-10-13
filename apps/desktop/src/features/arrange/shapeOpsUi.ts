import { create } from 'zustand';
import type { FilletKind, Point, SlotAnchor } from '@/api/types';

/** Operaciones de forma que se configuran en un diálogo (recortar no tiene: se aplica con un clic). */
export type ShapeDialogOp = 'break_apart' | 'optimize' | 'fillet' | 'cut_shapes' | 'resize_slots';

export interface OptimizeParams {
  /** Error máximo permitido (mm). */
  tolerance: number;
  /** Ajustar curvas a los tramos suaves. */
  smooth: boolean;
  /** Giro (grados) desde el cual un vértice es una esquina que se conserva. */
  cornerAngle: number;
}

export interface FilletParams {
  radius: number;
  style: FilletKind;
  /** Solo las esquinas de los nodos seleccionados con la herramienta de nodos. */
  onlyNodes: boolean;
}

export interface CutParams {
  /** Piezas cerradas (para rellenar) en vez de tramos abiertos (para cortar por línea). */
  closePieces: boolean;
  keepKnife: boolean;
  /** Objeto que hace de cuchilla; si ya no está en la selección, el último seleccionado. */
  knifeId: string | null;
}

export interface SlotParams {
  fromWidth: number;
  toWidth: number;
  tolerance: number;
  anchor: SlotAnchor;
  notches: boolean;
  holes: boolean;
  tabs: boolean;
}

export interface TrimParams {
  /** Conservar el tramo borrado como objeto aparte. */
  keepRemoved: boolean;
  /** Distancia máxima (mm) del clic al trazo. */
  maxDist: number;
}

export const SHAPE_DEFAULTS = {
  breakApart: { keepHoles: false },
  optimize: { tolerance: 0.05, smooth: false, cornerAngle: 35 } satisfies OptimizeParams,
  fillet: { radius: 3, style: 'round', onlyNodes: false } satisfies FilletParams as FilletParams,
  cut: { closePieces: true, keepKnife: false, knifeId: null } satisfies CutParams as CutParams,
  slots: { fromWidth: 3, toWidth: 3.2, tolerance: 0.15, anchor: 'center', notches: true, holes: true, tabs: false } satisfies SlotParams as SlotParams,
  trim: { keepRemoved: false, maxDist: 5 } satisfies TrimParams,
};

interface ShapeOpsUi {
  dialog: ShapeDialogOp | null;
  breakApart: { keepHoles: boolean };
  optimize: OptimizeParams;
  fillet: FilletParams;
  cut: CutParams;
  slots: SlotParams;
  trim: TrimParams;
  /** Último punto (mm, documento) donde se hizo clic en el lienzo: lo usa Recortar. */
  lastPoint: Point | null;

  open: (op: ShapeDialogOp) => void;
  close: () => void;
  setLastPoint: (p: Point | null) => void;
  patchBreakApart: (patch: Partial<ShapeOpsUi['breakApart']>) => void;
  patchOptimize: (patch: Partial<OptimizeParams>) => void;
  patchFillet: (patch: Partial<FilletParams>) => void;
  patchCut: (patch: Partial<CutParams>) => void;
  patchSlots: (patch: Partial<SlotParams>) => void;
  patchTrim: (patch: Partial<TrimParams>) => void;
}

/** Estado de interfaz de las operaciones de forma (diálogos, últimos valores usados y último clic). */
export const useShapeOps = create<ShapeOpsUi>()((set) => ({
  dialog: null,
  breakApart: { ...SHAPE_DEFAULTS.breakApart },
  optimize: { ...SHAPE_DEFAULTS.optimize },
  fillet: { ...SHAPE_DEFAULTS.fillet },
  cut: { ...SHAPE_DEFAULTS.cut },
  slots: { ...SHAPE_DEFAULTS.slots },
  trim: { ...SHAPE_DEFAULTS.trim },
  lastPoint: null,

  open: (dialog) => set({ dialog }),
  close: () => set({ dialog: null }),
  setLastPoint: (lastPoint) => set({ lastPoint }),
  patchBreakApart: (patch) => set((s) => ({ breakApart: { ...s.breakApart, ...patch } })),
  patchOptimize: (patch) => set((s) => ({ optimize: { ...s.optimize, ...patch } })),
  patchFillet: (patch) => set((s) => ({ fillet: { ...s.fillet, ...patch } })),
  patchCut: (patch) => set((s) => ({ cut: { ...s.cut, ...patch } })),
  patchSlots: (patch) => set((s) => ({ slots: { ...s.slots, ...patch } })),
  patchTrim: (patch) => set((s) => ({ trim: { ...s.trim, ...patch } })),
}));
