import { create } from 'zustand';
import type { Affine, BezPath, Point } from '@/api/types';
import type { BoxMode } from './hit';
import type { EditModel } from './nodeedit';
import type { Guide } from './snap';

/**
 * Estado efímero de la interacción con el lienzo (arrastres, guías, borradores). No es parte del
 * documento ni se guarda: vive aquí para que el renderer, la barra numérica y el registro de
 * comandos (Esc, C) lo lean sin pasar por el foco de ningún componente.
 */
export interface DraftShape {
  /** Contornos a dibujar como vista previa (mm). */
  paths: BezPath[];
  /** Polilínea en curso: se dibuja el último tramo hasta el cursor. */
  rubber?: boolean;
}

export interface Tip {
  /** Posición en px del lienzo. */
  x: number;
  y: number;
  text: string;
}

export type GestureKind = 'move' | 'scale' | 'rotate' | 'marquee' | 'draw' | 'polyline' | 'nodes';

/** Resultado vivo de un arrastre de nodos o manejadores: el objeto se dibuja con él hasta soltar. */
export interface NodePreview {
  model: EditModel;
  paths: BezPath[];
}

/** Lo que hay bajo el puntero con la herramienta de nodos (resalta el nodo o manejador). */
export type NodeHover = { id: string; k: number; handle: 'in' | 'out' | null } | null;

export interface InteractionState {
  hoverId: string | null;
  /** Gesto en curso, si lo hay. */
  gesture: GestureKind | null;
  /** Matriz viva que se aplica a la selección mientras se arrastra (no está en el documento). */
  live: Affine | null;
  /** Segundo clic sobre la selección: las esquinas giran en lugar de escalar. */
  rotateMode: boolean;
  marquee: { a: Point; b: Point; mode: BoxMode } | null;
  guides: Guide[];
  draft: DraftShape | null;
  tip: Tip | null;
  /** Cursor CSS (`data-cursor` del área). */
  cursor: string;
  /** Pivote del giro en curso, para dibujarlo. */
  pivot: Point | null;
  /** Puntos de la polilínea en curso. */
  polyPoints: number;
  /** Objetos cuyos nodos se están arrastrando, por id (no están en el documento hasta soltar). */
  nodePreview: Record<string, NodePreview> | null;
  nodeHover: NodeHover;
}

const INITIAL: InteractionState = {
  hoverId: null,
  gesture: null,
  live: null,
  rotateMode: false,
  marquee: null,
  guides: [],
  draft: null,
  tip: null,
  cursor: 'default',
  pivot: null,
  polyPoints: 0,
  nodePreview: null,
  nodeHover: null,
};

export const useInteraction = create<InteractionState>()(() => ({ ...INITIAL }));

export function patchInteraction(patch: Partial<InteractionState>): void {
  useInteraction.setState(patch);
}

/** Limpia todo lo que dibuja un gesto (conserva hover, cursor y modo giro). */
export function clearGesture(): void {
  useInteraction.setState({
    gesture: null, live: null, marquee: null, guides: [], draft: null, tip: null, pivot: null, polyPoints: 0, nodePreview: null,
  });
}

// ---- cancelación y cierre desde el registro de comandos ------------------------------------------

interface Hooks {
  cancel: (() => void) | null;
  closePolyline: (() => void) | null;
}
const hooks: Hooks = { cancel: null, closePolyline: null };

export function registerGestureHooks(h: Partial<Hooks>): () => void {
  Object.assign(hooks, h);
  return () => {
    for (const k of Object.keys(h) as (keyof Hooks)[]) if (hooks[k] === h[k]) hooks[k] = null;
  };
}

/** Cancela el gesto en curso. Devuelve `true` si había uno (Esc lo usa antes de quitar la selección). */
export function cancelGesture(): boolean {
  if (useInteraction.getState().gesture === null) return false;
  hooks.cancel?.();
  return true;
}

export function polylineInProgress(): boolean {
  return useInteraction.getState().gesture === 'polyline';
}

/** Cierra y termina la polilínea en curso (tecla C). */
export function closePolyline(): void {
  hooks.closePolyline?.();
}
