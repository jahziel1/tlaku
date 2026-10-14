import type { DesignObject, Document, ImageObject } from '@/api/types';
import type { PreviewRegion } from './types';

/** Vista de la imagen: entera ajustada al visor, o a escala real (1 mm = 96/25,4 px) con zoom. */
export type Zoom = 'fit' | 1 | 2 | 4;
export const ZOOMS: readonly Zoom[] = ['fit', 1, 2, 4];

/** Píxeles CSS por milímetro a «tamaño real» (96 ppp de referencia). */
export const PX_PER_MM = 96 / 25.4;
/** Visor del panel, en píxeles CSS. */
export const VIEW_W = 520;
export const VIEW_H = 360;
/** Puntos máximos por lado que se piden al motor. */
export const MAX_PX = 2048;

export type ImageObj = DesignObject & { kind: ImageObject & { type: 'image' } };

/** Las imágenes de una capa, en el orden del documento. */
export function imagesOfLayer(doc: Document, color: string): ImageObj[] {
  return doc.objects.filter((o): o is ImageObj => o.layer === color && o.kind.type === 'image');
}

/** La imagen a mostrar: la pedida si sigue en la capa, si no una seleccionada de la capa, si no la primera. */
export function pickImage(images: readonly ImageObj[], wanted: string | null, selected: readonly string[]): ImageObj | null {
  return images.find((i) => i.id === wanted) ?? images.find((i) => selected.includes(i.id)) ?? images[0] ?? null;
}

/** Cuántos píxeles CSS ocupa un milímetro de la imagen con este zoom (para `fit`, el que la ajusta al visor). */
export function pxPerMm(zoom: Zoom, sizeMm: readonly [number, number]): number {
  if (zoom === 'fit') return Math.min(VIEW_W / sizeMm[0], VIEW_H / sizeMm[1]);
  return PX_PER_MM * zoom;
}

/** Centro de la vista dentro de la imagen: si el visor es más grande que la imagen en un eje, queda centrada en él. */
export function clampCenter(center: readonly [number, number], zoom: Zoom, sizeMm: readonly [number, number]): [number, number] {
  const k = pxPerMm(zoom, sizeMm);
  const half = [VIEW_W / k / 2, VIEW_H / k / 2];
  return [0, 1].map((i) => {
    const size = sizeMm[i]!;
    if (half[i]! >= size / 2) return size / 2;
    return Math.min(Math.max(center[i]!, half[i]!), size - half[i]!);
  }) as [number, number];
}

/**
 * Rectángulo de la imagen (mm locales, Y hacia arriba) que cabe en el visor con este zoom, centrado
 * en `center`. `null` para la vista entera ajustada, donde no se pide un trozo.
 */
export function viewRegion(zoom: Zoom, center: readonly [number, number], sizeMm: readonly [number, number]): PreviewRegion | null {
  if (zoom === 'fit') return null;
  const [cx, cy] = clampCenter(center, zoom, sizeMm);
  const k = pxPerMm(zoom, sizeMm);
  const [hw, hh] = [VIEW_W / k / 2, VIEW_H / k / 2];
  return [Math.max(0, cx - hw), Math.max(0, cy - hh), Math.min(sizeMm[0], cx + hw), Math.min(sizeMm[1], cy + hh)];
}

/** Desplaza el centro lo que arrastra el puntero `dx`, `dy` píxeles (la Y de la pantalla crece hacia abajo, la de la imagen hacia arriba). */
export function panCenter(center: readonly [number, number], dxPx: number, dyPx: number, zoom: Zoom, sizeMm: readonly [number, number]): [number, number] {
  const k = pxPerMm(zoom, sizeMm);
  return clampCenter([center[0] - dxPx / k, center[1] + dyPx / k], zoom, sizeMm);
}
