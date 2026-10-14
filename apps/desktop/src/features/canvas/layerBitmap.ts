import type { Affine } from '@/api/types';
import { invertAffine } from '@/lib/anchors';
import { multiplyAffine } from '@/lib/geometry';
import type { ViewTransform } from '@/lib/viewport';
import type { Box } from './geom';

/**
 * Capas en bitmap del lienzo (fuera de pantalla). Un diseño denso se pinta una vez en un bitmap con
 * margen y se reutiliza mientras no cambie lo que dibuja; con una matriz viva (arrastre) el bitmap se
 * compone transformado, sin volver a trazar ni una geometría. Este archivo solo trae la parte pura (las
 * matrices, las cajas) y el contenedor del bitmap; qué se pinta lo decide `canvas2d.ts`.
 */

/** Margen extra del bitmap (fracción del lado mayor) para que el paneo no muestre bordes vacíos. */
export const CACHE_MARGIN = 0.2;
/** Holgura (px) alrededor de las cajas al repintar una zona: el trazo sobresale de la caja de la geometría. */
export const REGION_PAD_PX = 4;
/** Anillo (px) que se pinta de más alrededor de cada zona y no se copia (ver `paintRegions`). */
export const RING_PX = 4;
/** Más zonas que esto se funden en una sola (cada zona recorre la lista de objetos). */
export const MAX_REGIONS = 24;

/**
 * Matriz (px CSS del bitmap -> px CSS de la pantalla) para componer un bitmap pintado con `bmView`
 * (ya con el margen sumado al pan) en la vista `view`, con `live` (en mm del documento) aplicada a lo que
 * contiene. Sin `live` es la escala y el desplazamiento de siempre.
 */
export function bitmapMatrix(view: ViewTransform, bmView: ViewTransform, live: Affine | null): Affine {
  // bitmap -> mm del documento (Y hacia arriba) -> live -> pantalla
  const inv: Affine = [1 / bmView.zoom, 0, 0, -1 / bmView.zoom, -bmView.panX / bmView.zoom, bmView.panY / bmView.zoom];
  const screen: Affine = [view.zoom, 0, 0, -view.zoom, view.panX, view.panY];
  const m = live ? multiplyAffine(live, inv) : inv;
  return multiplyAffine(screen, m);
}

/** Caja que contiene a `b` transformada por `m` (las cuatro esquinas). */
export function transformBox(b: Box, m: Affine): Box {
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  for (const [x, y] of [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]] as const) {
    const tx = m[0] * x + m[2] * y + m[4];
    const ty = m[1] * x + m[3] * y + m[5];
    if (tx < x0) x0 = tx;
    if (tx > x1) x1 = tx;
    if (ty < y0) y0 = ty;
    if (ty > y1) y1 = ty;
  }
  return [x0, y0, x1, y1];
}

/** Zona de documento que ve el trazado de lo que `live` mueve: la visible, pasada por la inversa de `live`. */
export function sourceBox(visible: Box, live: Affine | null): Box | null {
  if (!live) return visible;
  const inv = invertAffine(live);
  return inv ? transformBox(visible, inv) : null;
}

/** Une zonas: si son más de `MAX_REGIONS`, una sola que las contiene a todas. */
export function limitRegions(regions: Box[]): Box[] {
  if (regions.length <= MAX_REGIONS) return regions;
  const u: Box = [...regions[0]!];
  for (const b of regions) {
    if (b[0] < u[0]) u[0] = b[0];
    if (b[1] < u[1]) u[1] = b[1];
    if (b[2] > u[2]) u[2] = b[2];
    if (b[3] > u[3]) u[3] = b[3];
  }
  return [u];
}

/** Ensancha una caja de documento `px` píxeles de pantalla (a `zoom` px por mm). */
export function padBox(b: Box, px: number, zoom: number): Box {
  const d = px / zoom;
  return [b[0] - d, b[1] - d, b[2] + d, b[3] + d];
}

export function sameView(a: ViewTransform, b: ViewTransform): boolean {
  return a.zoom === b.zoom && a.panX === b.panX && a.panY === b.panY;
}

/**
 * Un bitmap con margen. `tag` son las identidades de lo que dibuja (objetos, selección, capas...): si
 * alguna cambia, el contenido ya no sirve. `view` es la vista con la que se pintó (sin el margen).
 */
export class LayerBitmap {
  canvas: HTMLCanvasElement | null = null;
  private ctx: CanvasRenderingContext2D | null = null;
  /** Lienzo auxiliar donde se repintan las zonas parchadas. */
  private scratch: HTMLCanvasElement | null = null;
  margin = 0;
  view: ViewTransform = { zoom: 0, panX: 0, panY: 0 };
  tag: readonly unknown[] | null = null;
  /** Tamaño en px CSS (con margen) y densidad con que se creó. */
  private bw = 0;
  private bh = 0;
  private dpr = 1;

  /** ¿Es el mismo contenido (todas las identidades iguales)? */
  hasTag(tag: readonly unknown[]): boolean {
    const t = this.tag;
    return t !== null && t.length === tag.length && t.every((x, i) => x === tag[i]);
  }

  /** Prepara el lienzo para una vista de `w` x `h` px CSS; `false` si no se puede crear. */
  private prepare(w: number, h: number, dpr: number): boolean {
    const margin = Math.round(Math.max(w, h) * CACHE_MARGIN);
    const bw = w + margin * 2;
    const bh = h + margin * 2;
    const cw = Math.ceil(bw * dpr);
    const ch = Math.ceil(bh * dpr);
    if (!this.canvas || this.canvas.width !== cw || this.canvas.height !== ch) {
      const canvas = document.createElement('canvas');
      canvas.width = cw;
      canvas.height = ch;
      const ctx = canvas.getContext('2d');
      if (!ctx) return false;
      this.canvas = canvas;
      this.ctx = ctx;
    }
    this.margin = margin;
    this.bw = bw;
    this.bh = bh;
    this.dpr = dpr;
    return true;
  }

  /** Vista con la que se pinta dentro del bitmap (el margen se suma al pan). */
  get paintView(): ViewTransform {
    return { zoom: this.view.zoom, panX: this.view.panX + this.margin, panY: this.view.panY + this.margin };
  }

  /** Zona de documento (mm) que cubre el bitmap, margen incluido. */
  coverage(): Box {
    const { zoom, panX, panY } = this.paintView;
    return [-panX / zoom, (panY - this.bh) / zoom, (this.bw - panX) / zoom, panY / zoom];
  }

  /** Borra y pinta todo: `paint` recibe el contexto del bitmap, su vista y su tamaño en px CSS. */
  paintAll(view: ViewTransform, w: number, h: number, dpr: number, tag: readonly unknown[], paint: (ctx: CanvasRenderingContext2D, v: ViewTransform, bw: number, bh: number) => void): boolean {
    if (!this.prepare(w, h, dpr)) {
      this.tag = null;
      return false;
    }
    const ctx = this.ctx!;
    this.view = { zoom: view.zoom, panX: view.panX, panY: view.panY };
    this.tag = tag;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, this.canvas!.width, this.canvas!.height);
    paint(ctx, this.paintView, this.bw, this.bh);
    return true;
  }

  /** Copia el contenido de `other` (mismo tamaño y vista) como si se hubiera pintado aquí con `tag`. */
  copyFrom(other: LayerBitmap, tag: readonly unknown[]): boolean {
    if (!other.canvas || !other.ctx) return false;
    const cw = other.canvas.width;
    const ch = other.canvas.height;
    if (!this.canvas || this.canvas.width !== cw || this.canvas.height !== ch) {
      const canvas = document.createElement('canvas');
      canvas.width = cw;
      canvas.height = ch;
      const ctx = canvas.getContext('2d');
      if (!ctx) return false;
      this.canvas = canvas;
      this.ctx = ctx;
    }
    this.margin = other.margin;
    this.bw = other.bw;
    this.bh = other.bh;
    this.dpr = other.dpr;
    this.view = { ...other.view };
    this.tag = tag;
    const ctx = this.ctx!;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, cw, ch);
    ctx.drawImage(other.canvas, 0, 0);
    return true;
  }

  /**
   * Borra y repinta solo las zonas de documento `regions`. Cada zona se pinta primero en un lienzo
   * auxiliar del mismo tamaño y con la misma vista (así cada trazo sale con exactamente los mismos números
   * que al pintar todo), recortado a la zona más un anillo de `RING_PX` px, y de ahí solo se copia el
   * interior: el borde de un recorte da en sus píxeles de orilla una cobertura distinta de la de pintar todo,
   * y así esos píxeles nunca llegan al bitmap. `paint` recibe el contexto del auxiliar, la vista del bitmap y la
   * caja de documento que cubre el recorte.
   */
  paintRegions(regions: readonly Box[], tag: readonly unknown[], paint: (ctx: CanvasRenderingContext2D, v: ViewTransform, bw: number, bh: number, region: Box) => void): void {
    const ctx = this.ctx;
    const canvas = this.canvas;
    if (!ctx || !canvas) return;
    const v = this.paintView;
    const d = this.dpr;
    const sctx = this.scratchCtx();
    if (!sctx) return;
    for (const r of regions) {
      const g = padBox(r, REGION_PAD_PX, v.zoom);
      // zona en px del bitmap (Y hacia abajo), redondeada hacia afuera
      const x0 = Math.max(0, Math.floor((v.panX + g[0] * v.zoom) * d));
      const x1 = Math.min(canvas.width, Math.ceil((v.panX + g[2] * v.zoom) * d));
      const y0 = Math.max(0, Math.floor((v.panY - g[3] * v.zoom) * d));
      const y1 = Math.min(canvas.height, Math.ceil((v.panY - g[1] * v.zoom) * d));
      if (x1 <= x0 || y1 <= y0) continue;
      const rx0 = Math.max(0, x0 - RING_PX);
      const ry0 = Math.max(0, y0 - RING_PX);
      const rx1 = Math.min(canvas.width, x1 + RING_PX);
      const ry1 = Math.min(canvas.height, y1 + RING_PX);
      sctx.save();
      sctx.setTransform(1, 0, 0, 1, 0, 0);
      sctx.beginPath();
      sctx.rect(rx0, ry0, rx1 - rx0, ry1 - ry0);
      sctx.clip();
      sctx.clearRect(rx0, ry0, rx1 - rx0, ry1 - ry0);
      // la zona de documento de ese rectángulo de px, para descartar los objetos que no la tocan
      const cover: Box = [(rx0 / d - v.panX) / v.zoom, (v.panY - ry1 / d) / v.zoom, (rx1 / d - v.panX) / v.zoom, (v.panY - ry0 / d) / v.zoom];
      paint(sctx, v, this.bw, this.bh, cover);
      sctx.restore();
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.clearRect(x0, y0, x1 - x0, y1 - y0);
      ctx.drawImage(this.scratch!, x0, y0, x1 - x0, y1 - y0, x0, y0, x1 - x0, y1 - y0);
    }
    this.tag = tag;
  }

  /** Contexto del lienzo auxiliar (del tamaño del bitmap; se crea al primer parche). */
  private scratchCtx(): CanvasRenderingContext2D | null {
    const canvas = this.canvas;
    if (!canvas) return null;
    if (!this.scratch || this.scratch.width !== canvas.width || this.scratch.height !== canvas.height) {
      const s = document.createElement('canvas');
      s.width = canvas.width;
      s.height = canvas.height;
      if (!s.getContext('2d')) return null;
      this.scratch = s;
    }
    return this.scratch.getContext('2d');
  }

  /** Compone el bitmap en `ctx` (px CSS de la pantalla) con `view` y, si hay, la matriz viva `live`. */
  drawOnto(ctx: CanvasRenderingContext2D, view: ViewTransform, live: Affine | null = null): void {
    if (!this.canvas) return;
    const m = bitmapMatrix(view, this.paintView, live);
    const d = this.dpr;
    ctx.save();
    ctx.setTransform(d * m[0], d * m[1], d * m[2], d * m[3], d * m[4], d * m[5]);
    ctx.drawImage(this.canvas, 0, 0, this.canvas.width / d, this.canvas.height / d);
    ctx.restore();
  }

  release(): void {
    this.canvas = null;
    this.ctx = null;
    this.scratch = null;
    this.tag = null;
  }
}
