import type { SliceCreator } from './types';

/**
 * Imagen de la cámara dibujada bajo el diseño. `source` ya viene corregida de lente; `corners`
 * son sus cuatro esquinas en mm de documento (arriba-izq., arriba-der., abajo-der., abajo-izq.),
 * así el lienzo la dibuja con una homografía sin saber nada de la calibración.
 */
export interface CameraBackdrop {
  source: CanvasImageSource;
  width: number;
  height: number;
  corners: [[number, number], [number, number], [number, number], [number, number]];
  opacity: number;
  /** Cambia con cada cuadro nuevo para que el lienzo sepa que debe redibujar. */
  frame: number;
}

export interface CameraSlice {
  cameraBackdrop: CameraBackdrop | null;
  setCameraBackdrop: (backdrop: CameraBackdrop | null) => void;
}

export const createCameraSlice: SliceCreator<CameraSlice> = (set) => ({
  cameraBackdrop: null,
  setCameraBackdrop: (cameraBackdrop) => set({ cameraBackdrop }),
});
