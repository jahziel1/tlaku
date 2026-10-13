import type { Affine, DesignObject, DeviceOrigin, JobPreview, LayerSettings, Point } from '@/api/types';
import type { ViewTransform } from '@/lib/viewport';
import type { BoxMode } from './hit';
import type { DraftShape } from './interaction';
import type { Guide } from './snap';

/**
 * Colores del lienzo, leídos de los tokens CSS (`--cv-*`) para que sigan al tema.
 * Se usan hex o rgba (no oklch) porque Canvas 2D no los entiende en todos los motores web.
 */
export interface CanvasPalette {
  workspace: string;
  bed: string;
  bedEdge: string;
  gridMinor: string;
  gridMajor: string;
  origin: string;
  selection: string;
  selectionFill: string;
  travel: string;
  cut: string;
  /** Barridos de relleno e imagen (potencia baja). */
  engrave: string;
  overscan: string;
  head: string;
  ink: string;
  /** Guías del ajuste magnético. */
  guide: string;
  /** Caja de selección "toca" (de derecha a izquierda). */
  crossing: string;
}

/** Lo que dibuja la interacción encima del diseño (efímero, no es parte del documento). */
export interface SceneOverlay {
  hoverId: string | null;
  /** Matriz viva de un gesto en curso; se aplica a los objetos seleccionados. */
  live: Affine | null;
  /** Hay un gesto de giro: el marco se dibuja orientado y sin manijas. */
  rotating: boolean;
  rotateMode: boolean;
  canRotate: boolean;
  marquee: { a: Point; b: Point; mode: BoxMode } | null;
  guides: Guide[];
  draft: DraftShape | null;
  pivot: Point | null;
}

export interface Scene {
  /** Tamaño del lienzo en px CSS. */
  width: number;
  height: number;
  dpr: number;
  view: ViewTransform;
  bed: { width: number; height: number; origin: DeviceOrigin };
  objects: DesignObject[];
  layers: Map<string, LayerSettings>;
  selectedIds: ReadonlySet<string>;
  wireframe: boolean;
  /** Si hay vista previa visible, se dibuja encima y el diseño se atenúa. */
  preview: { data: JobPreview; scrub: number } | null;
  palette: CanvasPalette;
  overlay: SceneOverlay;
  /** Posición del cabezal de la máquina en coordenadas de documento (mm), si se conoce. */
  head: Point | null;
}

/**
 * Interfaz que separa el dibujo del resto de la app. La implementación inicial usa Canvas 2D;
 * el agente de `features/canvas` puede sustituirla por WebGL sin tocar nada más:
 * basta con crear otro `Renderer` y registrarlo en `createRenderer()` (ver `index.ts`).
 */
export interface Renderer {
  readonly kind: 'canvas2d' | 'webgl';
  attach(canvas: HTMLCanvasElement): void;
  /** Tamaño en px CSS y densidad de píxeles. */
  resize(width: number, height: number, dpr: number): void;
  render(scene: Scene): void;
  dispose(): void;
  /** El renderer pide volver a dibujar cuando algo asíncrono termina (una imagen que cargó). */
  setInvalidate(cb: () => void): void;
}

export type RendererFactory = () => Renderer;
