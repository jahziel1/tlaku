import { create } from 'zustand';
import { DEFAULT_CIRCULAR, DEFAULT_GRID, type CircularParams, type GridParams } from './arrays';

/** Respecto a qué se alinea: la caja de la selección, un objeto elegido o la cama. */
export type AlignReference = 'selection' | 'object' | 'bed';
export type JoinStyle = 'round' | 'miter' | 'bevel';
export type BooleanDialogOp = 'difference' | 'intersection' | 'xor';

export interface OffsetParams {
  /** Positiva = hacia afuera (la pieza crece y sus agujeros se encogen). */
  distance: number;
  join: JoinStyle;
  keepOriginal: boolean;
}

export type ArrangeDialog =
  | { kind: 'boolean'; op: BooleanDialogOp }
  | { kind: 'offset' }
  | { kind: 'grid' }
  | { kind: 'circular' }
  | { kind: 'align' };

interface ArrangeUi {
  dialog: ArrangeDialog | null;
  /** Hay una operación geométrica en curso (el motor tarda en formas pesadas). */
  busy: boolean;
  alignReference: AlignReference;
  /** Objeto de referencia cuando `alignReference` es `object`; si ya no está seleccionado, el primero. */
  alignObjectId: string | null;
  /**
   * Forma principal (A) de la booleana en el diálogo: es la que se conserva y a la que se le resta el resto.
   * El lienzo puede leerla para resaltarla mientras el diálogo está abierto.
   */
  primaryId: string | null;
  offset: OffsetParams;
  grid: GridParams;
  circular: CircularParams;

  open: (dialog: ArrangeDialog) => void;
  close: () => void;
  setBusy: (busy: boolean) => void;
  setAlign: (reference: AlignReference, objectId?: string | null) => void;
  setPrimary: (id: string | null) => void;
  patchOffset: (patch: Partial<OffsetParams>) => void;
  patchGrid: (patch: Partial<GridParams>) => void;
  patchCircular: (patch: Partial<CircularParams>) => void;
}

/** Estado de interfaz del área Organizar. Aparte del store de la app porque no es parte del documento. */
export const useArrangeUi = create<ArrangeUi>()((set) => ({
  dialog: null,
  busy: false,
  alignReference: 'selection',
  alignObjectId: null,
  primaryId: null,
  offset: { distance: 2, join: 'round', keepOriginal: true },
  grid: DEFAULT_GRID,
  circular: { ...DEFAULT_CIRCULAR, centerX: 0, centerY: 0 },

  open: (dialog) => set({ dialog }),
  close: () => set({ dialog: null, primaryId: null }),
  setBusy: (busy) => set({ busy }),
  setAlign: (alignReference, objectId) =>
    set((s) => ({ alignReference, alignObjectId: objectId === undefined ? s.alignObjectId : objectId })),
  setPrimary: (primaryId) => set({ primaryId }),
  patchOffset: (patch) => set((s) => ({ offset: { ...s.offset, ...patch } })),
  patchGrid: (patch) => set((s) => ({ grid: { ...s.grid, ...patch } })),
  patchCircular: (patch) => set((s) => ({ circular: { ...s.circular, ...patch } })),
}));
