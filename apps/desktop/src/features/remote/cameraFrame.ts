import { useStore } from '@/store';
import { getCorrectedCanvas } from '@/features/camera/session';

/** Ancho máximo del cuadro que se manda al celular. */
export const FRAME_WIDTH = 640;
/** Calidad JPEG (0 a 1): un cuadro de ~30 a 60 KB. */
export const FRAME_QUALITY = 0.6;

/**
 * Un cuadro reciente de la cámara de la PC como JPEG reducido en base64 (sin el prefijo `data:`), o
 * `null` si la cámara no está encendida o aún no hay imagen. Usa el lienzo ya corregido de lente que
 * muestra el panel de cámara: no abre la cámara ni toca el video.
 */
export function grabCameraFrame(): string | null {
  if (useStore.getState().cameraStatus !== 'live') return null;
  try {
    const source = getCorrectedCanvas();
    if (source.width === 0 || source.height === 0) return null;
    const width = Math.min(FRAME_WIDTH, source.width);
    const height = Math.max(1, Math.round((source.height * width) / source.width));
    const small = document.createElement('canvas');
    small.width = width;
    small.height = height;
    const ctx = small.getContext('2d');
    if (!ctx) return null;
    ctx.drawImage(source, 0, 0, width, height);
    const url = small.toDataURL('image/jpeg', FRAME_QUALITY);
    const comma = url.indexOf(',');
    return url.startsWith('data:image/jpeg') && comma > 0 ? url.slice(comma + 1) : null;
  } catch {
    return null;
  }
}
