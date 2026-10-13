import { create } from 'zustand';
import type { Affine, BezPath, GuideAxis, Point } from '@/api/types';
import type { BoxMode } from './hit';
import type { EditModel } from './nodeedit';
import type { Guide, SnapPointKind } from './snap';

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
  /** Nodos ya puestos de una curva en curso (se dibujan como cuadritos). */
  nodes?: Point[];
  /** Manijas de Bézier a mostrar: pares (nodo, extremo de la manija). */
  handles?: [Point, Point][];
}

/** Medición hecha con la herramienta Medir: la línea y, si se hizo clic en una forma, su id. */
export interface Measurement {
  a: Point;
  b: Point;
  /** Forma sobre la que se hizo clic (sin arrastrar). */
  objectId: string | null;
}

/** Indicador del punto al que se pegó el cursor (nodo, punto medio, centro, intersección o cuadrícula). */
export interface SnapMark {
  p: Point;
  kind: SnapPointKind | 'grid';
  /** Nombre del tipo, ya traducido. */
  label: string;
}

/**
 * Modos de dos pasos que piden puntos al lienzo: elegir la línea de un espejo (A y B) o el pivote
 * y el punto de referencia de rotar/escalar por dos puntos.
 */
export interface PickState {
  mode: 'mirror' | 'twoPoint';
  /** Puntos ya fijados (mm). */
  points: Point[];
}

export interface Tip {
  /** Posición en px del lienzo. */
  x: number;
  y: number;
  text: string;
}

/**
 * Guía del documento que se está creando (arrastrando desde una regla) o moviendo. `index` es
 * `null` mientras todavía no existe en el documento. Con `remove`, al soltar se borra.
 */
export interface GuideDrag {
  index: number | null;
  axis: GuideAxis;
  /** Posición viva (mm). */
  pos: number;
  remove: boolean;
}

export type GestureKind = 'move' | 'scale' | 'rotate' | 'marquee' | 'draw' | 'polyline' | 'nodes' | 'pick' | 'measure' | 'guide';

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
  /** Punto al que está pegado el cursor, para dibujar su indicador. */
  snapMark: SnapMark | null;
  /** Última medición (sigue visible hasta Esc o cambiar de herramienta). */
  measure: Measurement | null;
  /** Modo de elegir puntos en curso (espejo sobre línea, rotar/escalar por dos puntos). */
  pick: PickState | null;
  /** Guía del documento en creación o en movimiento. */
  guideDrag: GuideDrag | null;
  /** Guía bajo el puntero (posición en `Document.guides`), para resaltarla. */
  guideHover: number | null;
  /** Punto del contorno bajo el puntero con las herramientas de puentes e inicio (mm). */
  markHover: Point | null;
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
  snapMark: null,
  measure: null,
  pick: null,
  guideDrag: null,
  guideHover: null,
  markHover: null,
};

export const useInteraction = create<InteractionState>()(() => ({ ...INITIAL }));

export function patchInteraction(patch: Partial<InteractionState>): void {
  useInteraction.setState(patch);
}

/** Limpia todo lo que dibuja un gesto (conserva hover, cursor, modo giro y la última medición). */
export function clearGesture(): void {
  useInteraction.setState({
    gesture: null, live: null, marquee: null, guides: [], draft: null, tip: null, pivot: null, polyPoints: 0, nodePreview: null,
    snapMark: null, pick: null, guideDrag: null,
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
