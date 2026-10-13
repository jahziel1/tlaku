import { create } from 'zustand';
import { DEFAULT_CIRCULAR, DEFAULT_GRID, type CircularParams, type GridParams } from './arrays';

/** Respecto a qué se alinea: la caja de la selección, un objeto elegido o la cama. */
export type AlignReference = 'selection' | 'object' | 'bed';
export type JoinStyle = 'round' | 'miter' | 'bevel';
export type BooleanDialogOp = 'difference' | 'intersection' | 'xor';

/** Limpieza de trazos: operaciones del motor que aceptan formas abiertas y una tolerancia en mm. */
export type CleanupOp = 'join' | 'close' | 'dedupe';

/**
 * Tolerancias iniciales (mm). Las mismas que usa el motor cuando no se le manda ninguna:
 * unir solo extremos que casi se tocan; cerrar admite más hueco porque es una sola trayectoria
 * consigo misma; quitar duplicados usa el error del aplanado de curvas.
 */
export const CLEANUP_DEFAULTS: Record<CleanupOp, number> = { join: 0.05, close: 0.5, dedupe: 0.01 };

export interface OffsetParams {
  /** Positiva = hacia afuera (la pieza crece y sus agujeros se encogen). */
  distance: number;
  join: JoinStyle;
  keepOriginal: boolean;
}

export type ArrangeDialog =
  | { kind: 'boolean'; op: BooleanDialogOp }
  | { kind: 'offset' }
  | { kind: 'cleanup'; op: CleanupOp }
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
  /** Última tolerancia (mm) usada en cada limpieza de trazos. */
  cleanup: Record<CleanupOp, number>;
  grid: GridParams;
  circular: CircularParams;

  open: (dialog: ArrangeDialog) => void;
  close: () => void;
  setBusy: (busy: boolean) => void;
  setAlign: (reference: AlignReference, objectId?: string | null) => void;
  setPrimary: (id: string | null) => void;
  patchOffset: (patch: Partial<OffsetParams>) => void;
  setCleanupTolerance: (op: CleanupOp, tolerance: number) => void;
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
  cleanup: { ...CLEANUP_DEFAULTS },
  grid: DEFAULT_GRID,
  circular: { ...DEFAULT_CIRCULAR, centerX: 0, centerY: 0 },

  open: (dialog) => set({ dialog }),
  close: () => set({ dialog: null, primaryId: null }),
  setBusy: (busy) => set({ busy }),
  setAlign: (alignReference, objectId) =>
    set((s) => ({ alignReference, alignObjectId: objectId === undefined ? s.alignObjectId : objectId })),
  setPrimary: (primaryId) => set({ primaryId }),
  patchOffset: (patch) => set((s) => ({ offset: { ...s.offset, ...patch } })),
  setCleanupTolerance: (op, tolerance) => set((s) => ({ cleanup: { ...s.cleanup, [op]: tolerance } })),
  patchGrid: (patch) => set((s) => ({ grid: { ...s.grid, ...patch } })),
  patchCircular: (patch) => set((s) => ({ circular: { ...s.circular, ...patch } })),
}));
