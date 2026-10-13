import { Canvas2DRenderer } from './canvas2d';
import type { Renderer } from './renderer';

/** Punto único para cambiar de renderer (por ejemplo, a WebGL): devolver otra implementación de `Renderer`. */
export function createRenderer(): Renderer {
  return new Canvas2DRenderer();
}
