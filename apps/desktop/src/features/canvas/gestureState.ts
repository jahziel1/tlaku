/** Tipos, constantes y estado compartido de `useCanvasTools` y sus manejadores de gestos. */
import type { Affine, DesignObject, Point } from '@/api/types';
import { fmt } from '@/lib/platform';
import type { PenNode } from './pen';
import type { Mods, ToolDeps } from './toolDeps';
import type { SnapSet } from './snap';
import type { HandleId } from './transform';
import type { Snapping } from './toolSnapping';
import type { PickContext } from './hit';

/** Doble clic al dibujar polilíneas: tiempo (ms) y distancia (px). */
export const DOUBLE_MS = 350;
export const DOUBLE_PX = 6;
export const CLOSE_PX = 8;
/** Tamaño mínimo (mm) de una forma arrastrada para que se cree. */
export const MIN_SHAPE_MM = 0.05;

export const modsOf = (e: { shiftKey: boolean; altKey: boolean; ctrlKey: boolean; metaKey: boolean }): Mods => ({
  shift: e.shiftKey,
  alt: e.altKey,
  ctrl: e.ctrlKey || e.metaKey,
});

export interface PointerInfo {
  sx: number;
  sy: number;
  mods: Mods;
}

export interface Common {
  pointerId: number;
  startScreen: Point;
  startWorld: Point;
  /** Superó el umbral de arrastre. */
  active: boolean;
}

export type Gesture =
  | (Common & { kind: 'press'; hit: DesignObject; wasSelected: boolean; shift: boolean })
  | (Common & { kind: 'move'; ids: string[]; frame: Bounds2; targets: SnapSet; dx: number; dy: number; grab: Point | null })
  | (Common & { kind: 'scale'; ids: string[]; frame: Bounds2; handle: HandleId; offset: Point; targets: SnapSet; matrix: Affine })
  | (Common & { kind: 'rotate'; ids: string[]; frame: Bounds2; baseDeg: number; matrix: Affine })
  | (Common & { kind: 'marquee'; shift: boolean })
  | (Common & { kind: 'draw'; tool: 'rect' | 'ellipse' | 'polygon' | 'star'; targets: SnapSet; end: Point })
  /** Arrastre de las manijas del nodo recién puesto con la herramienta Línea. */
  | (Common & { kind: 'pen'; index: number });

export type Bounds2 = [Point, Point];

export interface Poly {
  nodes: PenNode[];
  lastAt: number;
  lastScreen: Point;
  targets: SnapSet;
}

let gestureSeq = 0;
/** Clave de agrupación del historial: cada gesto es un solo paso de deshacer. */
export const dragKey = () => `drag:${++gestureSeq}`;

/** Estado mutable que comparten el hook y los manejadores de cada herramienta. */
export interface GestureState {
  /** Gesto en curso (seleccionar, mover, escalar, rotar, dibujar). */
  g: Gesture | null;
  /** Polilínea en construcción (herramienta Línea). */
  poly: Poly | null;
}

export const fmtMM = (v: number) => fmt(v, 2);

/** Lo que reciben los manejadores de gestos: las utilidades del hook más el ajuste magnético y el estado. */
export type GestureCtx = ToolDeps & Pick<Snapping, 'markOf' | 'KIND_LABEL' | 'applyPointSnap'> & {
  pickCtx: () => PickContext;
  state: GestureState;
};
