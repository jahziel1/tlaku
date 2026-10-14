import type { ImageObject, LayerSettings } from '@/api/types';

/** Rectángulo `[x0, y0, x1, y1]` en mm locales de la imagen (origen abajo a la izquierda, Y hacia arriba). */
export type PreviewRegion = [number, number, number, number];

export interface ImagePreviewArgs {
  image: ImageObject;
  layer: LayerSettings;
  /** `null` = la imagen entera; con un rectángulo, ese trozo a la resolución real del trabajo. */
  region_mm: PreviewRegion | null;
  /** Máximo de puntos por lado de la vista. */
  max_px: number;
}

/** Vista previa de lo que se grabaría de una imagen (ver `edit/imaging.rs`). */
export interface ImagePreview {
  /** PNG en escala de grises (negro = potencia máxima) en base64, sin prefijo `data:`. */
  png_base64: string;
  width_px: number;
  height_px: number;
  /** Rectángulo realmente cubierto, mm locales. */
  covered_mm: PreviewRegion;
  /** Separación entre puntos de la vista, mm. */
  interval_mm: number;
  /** `true` si es la rejilla real del trabajo; `false` si es más gruesa para ir rápido. */
  exact: boolean;
}
