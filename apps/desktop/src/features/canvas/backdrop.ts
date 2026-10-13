import type { Affine, Point } from '@/api/types';
import { worldToScreen, type ViewTransform } from '@/lib/viewport';
import type { CameraBackdrop } from '@/store/camera';

/**
 * Imagen de la cámara bajo el diseño. Canvas 2D solo sabe de transformaciones afines, pero la imagen
 * de la cama es una proyección (homografía): se parte en una malla de triángulos y cada uno se dibuja
 * con la afín que lleva sus tres vértices a su sitio. Los vértices de la malla caen exactos sobre la
 * homografía, así que con suficientes divisiones el error visual es subpíxel.
 */

/** Esquinas en mm de documento: arriba-izq., arriba-der., abajo-der., abajo-izq. de la imagen. */
export type Quad = CameraBackdrop['corners'];

export type Tri = [Point, Point, Point];

export interface MeshTriangle {
  /** Triángulo en píxeles de la imagen. */
  src: Tri;
  /** El mismo triángulo en mm de documento. */
  dst: Tri;
}

const EPS = 1e-9;

/**
 * Proyección del cuadrado unidad (u, v) a un cuadrilátero con p0 en (0,0), p1 en (1,0), p2 en (1,1) y p3 en (0,1)
 * (Heckbert). Devuelve la función y si el cuadrilátero es un paralelogramo (afín exacto).
 */
export function homography(q: Quad): { map: (u: number, v: number) => Point; affine: boolean } {
  const [[x0, y0], [x1, y1], [x2, y2], [x3, y3]] = q;
  const sx = x0 - x1 + x2 - x3;
  const sy = y0 - y1 + y2 - y3;
  if (Math.abs(sx) < EPS && Math.abs(sy) < EPS) {
    return { map: (u, v) => [x0 + (x1 - x0) * u + (x3 - x0) * v, y0 + (y1 - y0) * u + (y3 - y0) * v], affine: true };
  }
  const dx1 = x1 - x2;
  const dx2 = x3 - x2;
  const dy1 = y1 - y2;
  const dy2 = y3 - y2;
  const den = dx1 * dy2 - dx2 * dy1;
  const g = den === 0 ? 0 : (sx * dy2 - dx2 * sy) / den;
  const h = den === 0 ? 0 : (dx1 * sy - sx * dy1) / den;
  const a = x1 - x0 + g * x1;
  const b = x3 - x0 + h * x3;
  const d = y1 - y0 + g * y1;
  const e = y3 - y0 + h * y3;
  return {
    map: (u, v) => {
      const w = g * u + h * v + 1;
      return [(a * u + b * v + x0) / w, (d * u + e * v + y0) / w];
    },
    affine: false,
  };
}

/** Divisiones por lado de la malla: un paralelogramo es exacto con 2 triángulos; con perspectiva, más. */
export function meshDivisions(q: Quad, max = 16): number {
  return homography(q).affine ? 1 : max;
}

/** Malla de triángulos de una imagen de `w` x `h` px proyectada sobre `q`. */
export function backdropMesh(q: Quad, w: number, h: number, divisions = meshDivisions(q)): MeshTriangle[] {
  const n = Math.max(1, Math.floor(divisions));
  const { map } = homography(q);
  const src = (ix: number, iy: number): Point => [(ix / n) * w, (iy / n) * h];
  const dst = (ix: number, iy: number): Point => map(ix / n, iy / n);
  const out: MeshTriangle[] = [];
  for (let iy = 0; iy < n; iy++) {
    for (let ix = 0; ix < n; ix++) {
      const a = [ix, iy] as const;
      const b = [ix + 1, iy] as const;
      const c = [ix + 1, iy + 1] as const;
      const d = [ix, iy + 1] as const;
      out.push({ src: [src(...a), src(...b), src(...c)], dst: [dst(...a), dst(...b), dst(...c)] });
      out.push({ src: [src(...a), src(...c), src(...d)], dst: [dst(...a), dst(...c), dst(...d)] });
    }
  }
  return out;
}

/** Afín que lleva el triángulo `from` al triángulo `to` (null si `from` es degenerado). */
export function affineBetween(from: Tri, to: Tri): Affine | null {
  const [[ax, ay], [bx, by], [cx, cy]] = from;
  const det = (bx - ax) * (cy - ay) - (cx - ax) * (by - ay);
  if (Math.abs(det) < EPS) return null;
  // Inversa de la parte lineal de `from`, aplicada a las diferencias de `to`.
  const i00 = (cy - ay) / det;
  const i01 = -(cx - ax) / det;
  const i10 = -(by - ay) / det;
  const i11 = (bx - ax) / det;
  const du1 = to[1][0] - to[0][0];
  const dv1 = to[1][1] - to[0][1];
  const du2 = to[2][0] - to[0][0];
  const dv2 = to[2][1] - to[0][1];
  const a = du1 * i00 + du2 * i10;
  const c = du1 * i01 + du2 * i11;
  const b = dv1 * i00 + dv2 * i10;
  const d = dv1 * i01 + dv2 * i11;
  return [a, b, c, d, to[0][0] - a * ax - c * ay, to[0][1] - b * ax - d * ay];
}

/** Aleja un poco cada vértice del centroide: los triángulos vecinos se traslapan y no queda costura. */
function inflate(t: Tri, px: number): Tri {
  const cx = (t[0][0] + t[1][0] + t[2][0]) / 3;
  const cy = (t[0][1] + t[1][1] + t[2][1]) / 3;
  const grow = ([x, y]: Point): Point => {
    const dx = x - cx;
    const dy = y - cy;
    const l = Math.hypot(dx, dy) || 1;
    return [x + (dx / l) * px, y + (dy / l) * px];
  };
  return [grow(t[0]), grow(t[1]), grow(t[2])];
}

/** Traslape (px de pantalla) entre triángulos vecinos para tapar la costura del suavizado. */
export const SEAM_PX = 0.75;

/** Todo lo que cambia el dibujo de la capa proyectada. */
export interface BackdropKey {
  source: CanvasImageSource;
  frame: number;
  corners: Quad;
  zoom: number;
  panX: number;
  panY: number;
  w: number;
  h: number;
  dpr: number;
}

export function backdropKeyOf(b: CameraBackdrop, view: ViewTransform, w: number, h: number, dpr: number): BackdropKey {
  return { source: b.source, frame: b.frame, corners: b.corners, zoom: view.zoom, panX: view.panX, panY: view.panY, w, h, dpr };
}

export function sameBackdropKey(a: BackdropKey, b: BackdropKey): boolean {
  return a.source === b.source && a.frame === b.frame && a.corners === b.corners && a.zoom === b.zoom &&
    a.panX === b.panX && a.panY === b.panY && a.w === b.w && a.h === b.h && a.dpr === b.dpr;
}

/**
 * Dibuja la imagen proyectada, a opacidad plena, en `ctx` (un lienzo del tamaño de la vista).
 * Los triángulos fuera de la vista se omiten. El llamador aplica la opacidad al componerla.
 */
export function paintBackdrop(ctx: CanvasRenderingContext2D, b: CameraBackdrop, view: ViewTransform, w: number, h: number, dpr: number): void {
  if (b.width <= 0 || b.height <= 0) return;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
  for (const tri of backdropMesh(b.corners, b.width, b.height)) {
    const scr = tri.dst.map((p) => worldToScreen(view, p)) as Tri;
    const minX = Math.min(scr[0][0], scr[1][0], scr[2][0]);
    const maxX = Math.max(scr[0][0], scr[1][0], scr[2][0]);
    const minY = Math.min(scr[0][1], scr[1][1], scr[2][1]);
    const maxY = Math.max(scr[0][1], scr[1][1], scr[2][1]);
    if (maxX < 0 || minX > w || maxY < 0 || minY > h) continue;
    const m = affineBetween(tri.src, scr);
    if (!m) continue;
    const clip = inflate(scr, SEAM_PX);
    ctx.save();
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.beginPath();
    ctx.moveTo(clip[0][0], clip[0][1]);
    ctx.lineTo(clip[1][0], clip[1][1]);
    ctx.lineTo(clip[2][0], clip[2][1]);
    ctx.closePath();
    ctx.clip();
    ctx.transform(m[0], m[1], m[2], m[3], m[4], m[5]);
    // Solo el trozo de la imagen que cubre el triángulo (con un píxel de margen).
    const sx = Math.max(0, Math.floor(Math.min(tri.src[0][0], tri.src[1][0], tri.src[2][0])) - 1);
    const sy = Math.max(0, Math.floor(Math.min(tri.src[0][1], tri.src[1][1], tri.src[2][1])) - 1);
    const ex = Math.min(b.width, Math.ceil(Math.max(tri.src[0][0], tri.src[1][0], tri.src[2][0])) + 1);
    const ey = Math.min(b.height, Math.ceil(Math.max(tri.src[0][1], tri.src[1][1], tri.src[2][1])) + 1);
    if (ex > sx && ey > sy) ctx.drawImage(b.source, sx, sy, ex - sx, ey - sy, sx, sy, ex - sx, ey - sy);
    ctx.restore();
  }
}
