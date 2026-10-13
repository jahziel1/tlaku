import type { BezPath } from './bezpath';

/**
 * Convierte curvas Bézier en un `Path2D` para recortar (`ctx.clip(path, 'evenodd')`). Las curvas
 * están en las mismas coordenadas en que se dibuja después (aquí, las locales de la imagen).
 */
export function bezToPath2D(paths: readonly BezPath[], make: () => Path2D = () => new Path2D()): Path2D {
  const out = make();
  for (const path of paths) {
    for (const s of path) {
      switch (s.type) {
        case 'move_to':
          out.moveTo(s.to[0], s.to[1]);
          break;
        case 'line_to':
          out.lineTo(s.to[0], s.to[1]);
          break;
        case 'quad_to':
          out.quadraticCurveTo(s.ctrl[0], s.ctrl[1], s.to[0], s.to[1]);
          break;
        case 'cubic_to':
          out.bezierCurveTo(s.ctrl1[0], s.ctrl1[1], s.ctrl2[0], s.ctrl2[1], s.to[0], s.to[1]);
          break;
        case 'close':
          out.closePath();
          break;
      }
    }
  }
  return out;
}
