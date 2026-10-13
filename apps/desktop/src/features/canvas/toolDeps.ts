import type { Point } from '@/api/types';
import type { AppState } from '@/store';
import type { ViewTransform } from '@/lib/viewport';
import type { SnapOptions, SnapSet } from './snap';

export interface Mods {
  shift: boolean;
  alt: boolean;
  ctrl: boolean;
}

/** Lo que `useCanvasTools` les da a las herramientas que viven en su propio archivo. */
export interface ToolDeps {
  el: HTMLElement;
  st: () => AppState;
  viewOf: () => ViewTransform;
  world: (sx: number, sy: number) => Point;
  capture: (e: PointerEvent) => void;
  release: (id: number) => void;
  snapOpts: (mods: Mods, suspendWithAlt?: boolean) => SnapOptions | null;
  targetsFor: (skip: ReadonlySet<string>) => SnapSet;
  setTip: (sx: number, sy: number, text: string) => void;
  /** Pega `p` a puntos, líneas y cuadrícula según las opciones y deja el indicador del punto. */
  snapTo: (p: Point, targets: SnapSet, mods: Mods) => Point;
}

/** Distancia en px antes de que un clic se convierta en arrastre. */
export const DRAG_PX = 3;
