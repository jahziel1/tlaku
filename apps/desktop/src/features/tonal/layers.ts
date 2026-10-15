/**
 * Imágenes de la vista «por tonos» que ve el lienzo. Son dos: `full`, toda la hoja de material con una
 * resolución moderada, y `detail`, solo lo que se ve cuando el zoom pide más (se vuelve a pedir al
 * motor al acercar o mover la vista). El lienzo las dibuja con `drawTonal`.
 */
import type { ViewTransform } from '@/lib/viewport';
import type { Box } from './types';

export interface TonalLayer {
  image: CanvasImageSource;
  /** Rectángulo que cubre la imagen (mm del documento). */
  covered: Box;
  /** Lado de un píxel (mm). */
  pxMm: number;
}

interface Layers {
  full: TonalLayer | null;
  detail: TonalLayer | null;
  /** Hoja de material supuesta, para saber hasta dónde llega. */
  sheet: Box | null;
}

let layers: Layers = { full: null, detail: null, sheet: null };
const listeners = new Set<() => void>();

export const getTonalLayers = (): Readonly<Layers> => layers;

export function setTonalLayers(patch: Partial<Layers>): void {
  layers = { ...layers, ...patch };
  for (const l of listeners) l();
}

export function clearTonalLayers(): void {
  if (!layers.full && !layers.detail && !layers.sheet) return;
  layers = { full: null, detail: null, sheet: null };
  for (const l of listeners) l();
}

/** Avisa al lienzo cuando cambian las imágenes. Devuelve la función que deja de avisar. */
export function onTonalChange(fn: () => void): () => void {
  listeners.add(fn);
  return () => void listeners.delete(fn);
}

/** ¿Hay una imagen que dibujar? */
export const hasTonalImage = (): boolean => layers.full !== null || layers.detail !== null;

/** Dibuja las imágenes (la general y, encima, el detalle) bajo la transformación de la vista. */
export function drawTonal(ctx: CanvasRenderingContext2D, view: ViewTransform, dpr: number): void {
  const { full, detail } = layers;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  for (const layer of [full, detail]) {
    if (!layer) continue;
    const [x0, y0, x1, y1] = layer.covered;
    const ax = view.panX + x0 * view.zoom;
    const ay = view.panY - y1 * view.zoom;
    const w = (x1 - x0) * view.zoom;
    const h = (y1 - y0) * view.zoom;
    // Acercado a más de un píxel de pantalla por píxel de imagen se ven los puntos tal cual.
    ctx.imageSmoothingEnabled = layer.pxMm * view.zoom < 1;
    ctx.drawImage(layer.image, ax, ay, w, h);
  }
  ctx.restore();
}
