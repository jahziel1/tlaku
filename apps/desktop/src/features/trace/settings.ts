import type { ImageObject, TraceSettings } from '@/api/types';

/** Mismos valores que `TraceSettings::default()` del motor (pensados para logotipos y dibujos). */
export const DEFAULT_TRACE_SETTINGS: TraceSettings = {
  threshold: 0.5,
  invert: false,
  blur_px: 0,
  speckle_px: 4,
  corner_threshold_deg: 60,
  smoothing: 0.2,
  mode: 'spline',
};

/** Rangos de los deslizadores (el motor admite más desenfoque, pero no tiene sentido usarlo). */
export const TRACE_LIMITS = {
  speckle_px: { min: 0, max: 50 },
  blur_px: { min: 0, max: 10 },
  corner_threshold_deg: { min: 0, max: 180 },
} as const;

/** Milímetros que mide un píxel de la imagen (en ancho). */
export function mmPerPixel(image: Pick<ImageObject, 'size_mm' | 'width_px'>): number {
  return image.width_px > 0 ? image.size_mm[0] / image.width_px : 0;
}

/** Equivalente en mm del lado de mancha mínimo (`speckle_px`). */
export function speckleMm(image: Pick<ImageObject, 'size_mm' | 'width_px'>, speckle_px: number): number {
  return speckle_px * mmPerPixel(image);
}

/** Cuantas formas o nodos hacen sospechar de un resultado áspero (miles de formas diminutas). */
export const ROUGH_SHAPES = 200;
export const ROUGH_NODES = 20000;
