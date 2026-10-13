import type { DesignObject, DeviceOrigin, JobPreview, LayerSettings } from '@/api/types';
import type { ViewTransform } from '@/lib/viewport';

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
}

export type RendererFactory = () => Renderer;
