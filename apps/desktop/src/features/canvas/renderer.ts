import type { Affine, DesignObject, DeviceOrigin, JobPreview, LayerSettings, Point } from '@/api/types';
import type { LengthUnit } from '@/lib/units';
import type { ViewTransform } from '@/lib/viewport';
import type { BoxMode } from './hit';
import type { CameraBackdrop } from '@/store/camera';
import type { DraftShape, Measurement, SnapMark } from './interaction';
import type { SceneMarks } from './marks';
import type { NodeOverlay } from './nodeview';
import type { Guide } from './snap';
import type { SceneTiles } from '@/features/tiling/overlay';

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
  /** Guías del documento (opcional: sin él se usa el color de grabado). */
  docGuide?: string;
  /** Puentes manuales y marca del punto bajo el puntero en las herramientas de puentes e inicio. */
  tab?: string;
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
  /** Nodos y manejadores de la herramienta de nodos; null si no está activa. */
  nodes: NodeOverlay | null;
  /** Indicador del punto al que se pegó el cursor. */
  snapMark?: SnapMark | null;
  /** Medición de la herramienta Medir. */
  measure?: Measurement | null;
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
  /** Objetos de `selectedIds` en el orden del documento; si falta, el renderer los busca. */
  selected?: DesignObject[];
  wireframe: boolean;
  /** Forma principal (A) de la booleana en el diálogo de Organizar; se resalta distinto a la selección. */
  primaryId?: string | null;
  /** Si hay vista previa visible, se dibuja encima y el diseño se atenúa. */
  preview: { data: JobPreview; scrub: number } | null;
  palette: CanvasPalette;
  overlay: SceneOverlay;
  /** Posición del cabezal de la máquina en coordenadas de documento (mm), si se conoce. */
  head: Point | null;
  /** Imagen de la cámara, bajo la cuadrícula y los objetos; null si no hay cámara. */
  backdrop: CameraBackdrop | null;
  /** Guías del documento, puentes manuales y flechas de inicio. */
  marks?: SceneMarks;
  /** Unidad de la cuadrícula (mm por omisión): sus líneas siguen a las marcas de la regla. */
  gridUnit?: LengthUnit;
  /** Tramos numerados del asistente de tiling (vista previa o sesión en curso). */
  tiles?: SceneTiles | null;
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
