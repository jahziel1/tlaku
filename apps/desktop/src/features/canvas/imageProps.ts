import type { DesignObject, ImageDitherMode, LayerMode, LayerSettings } from '@/api/types';

/** Lo que la ventana de propiedades muestra de una imagen (D35). */
export interface ImageInfo {
  widthPx: number;
  heightPx: number;
  /** Tamaño en el documento (con la escala del objeto), en mm. */
  widthMm: number;
  heightMm: number;
  /** Resolución efectiva: píxeles de la imagen por pulgada del documento, en cada eje. */
  dpiX: number;
  dpiY: number;
  layerColor: string;
  layerName: string;
  layerMode: LayerMode | null;
  /** Tramado de la capa; solo cuenta si la capa graba como imagen. */
  dither: ImageDitherMode | null;
  /** Resolución a la que graba la capa (25,4 ÷ intervalo); `null` si no graba como imagen. */
  layerDpi: number | null;
}

const MM_PER_INCH = 25.4;

/** Información de la imagen `obj`; `null` si el objeto no es una imagen. */
export function imageInfo(obj: DesignObject, layer: LayerSettings | undefined): ImageInfo | null {
  if (obj.kind.type !== 'image') return null;
  const [a, b, c, d] = obj.transform;
  const [w, h] = obj.kind.size_mm;
  const widthMm = w * Math.hypot(a, b);
  const heightMm = h * Math.hypot(c, d);
  const dpi = (px: number, mm: number) => (mm > 1e-9 ? px / (mm / MM_PER_INCH) : 0);
  const asImage = layer?.mode === 'image';
  return {
    widthPx: obj.kind.width_px,
    heightPx: obj.kind.height_px,
    widthMm,
    heightMm,
    dpiX: dpi(obj.kind.width_px, widthMm),
    dpiY: dpi(obj.kind.height_px, heightMm),
    layerColor: obj.layer,
    layerName: layer?.name ?? '',
    layerMode: layer?.mode ?? null,
    dither: asImage ? layer.image.mode : null,
    layerDpi: asImage && layer.interval > 0 ? MM_PER_INCH / layer.interval : null,
  };
}

/** Resolución para mostrar: sin decimales por encima de 100, con uno por debajo. */
export function formatDpi(dpi: number): string {
  if (!Number.isFinite(dpi) || dpi <= 0) return '–';
  return dpi >= 100 ? String(Math.round(dpi)) : (Math.round(dpi * 10) / 10).toString();
}
