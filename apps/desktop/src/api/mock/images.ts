import type { LaserApi } from '../api';
import type { LayerSettings } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

/** PNG de 1 x 1 píxel blanco: el modo simulado no tiene imágenes en disco que tramar. */
const BLANK_PNG_BASE64 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAAAAAA6fptVAAAACklEQVR4nGP4DwABAQEAsTj2FAAAAABJRU5ErkJggg==';

/** Matriz de Bayer 4 x 4 (umbrales 0 a 15) para el tramado simulado. */
const BAYER4 = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];

/**
 * Cuadro tramado de ejemplo (un retrato abstracto con degradado) de `width` x `height` puntos, con el
 * brillo, el contraste, el gamma y la inversión de la capa. Solo vive en el navegador: sin canvas
 * (pruebas) devuelve el cuadro blanco. No es el algoritmo del motor, solo una imagen creíble.
 */
function simulatedDither(layer: LayerSettings, width: number, height: number, cover: [number, number, number, number], size: [number, number]): string {
  if (typeof document === 'undefined' || width * height > 4_000_000) return BLANK_PNG_BASE64;
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d');
  if (!ctx) return BLANK_PNG_BASE64;
  const img = ctx.createImageData(width, height);
  const { brightness, contrast, gamma, invert } = layer.image;
  const [x0, y0, x1, y1] = cover;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const u = (x0 + ((x + 0.5) / width) * (x1 - x0)) / size[0];
      const v = (y0 + ((y + 0.5) / height) * (y1 - y0)) / size[1];
      // Fondo en degradado, un óvalo claro (cara) y una franja oscura (cabello).
      let g = 0.25 + 0.55 * u * (1 - 0.4 * v);
      const e = ((u - 0.5) / 0.28) ** 2 + ((v - 0.55) / 0.38) ** 2;
      if (e < 1) g = 0.62 + 0.3 * (1 - e) - 0.18 * Math.abs(u - 0.5);
      if (v < 0.22 && e >= 1 && Math.abs(u - 0.5) < 0.34) g = 0.12;
      g = (g - 0.5) * (1 + contrast) + 0.5 + brightness * 0.5;
      g = Math.min(1, Math.max(0, g)) ** (1 / (gamma > 0 ? gamma : 1));
      if (invert) g = 1 - g;
      const on = g * 16 > BAYER4[(y % 4) * 4 + (x % 4)]!;
      const o = (y * width + x) * 4;
      img.data[o] = img.data[o + 1] = img.data[o + 2] = on ? 255 : 0;
      img.data[o + 3] = 255;
    }
  }
  ctx.putImageData(img, 0, 0);
  const url = canvas.toDataURL('image/png');
  const comma = url.indexOf(',');
  return comma > 0 ? url.slice(comma + 1) : BLANK_PNG_BASE64;
}

/**
 * Vista previa de imágenes del modo simulado: un cuadro tramado de ejemplo con las dimensiones y
 * el aviso de resolución que daría el motor, para ejercitar la interfaz sin archivos reales.
 */
export function mockImages(lag: Lag): Pick<LaserApi, 'previewImage'> {
  return {
    previewImage: ({ image, layer, region_mm, max_px }) =>
      lag(() => {
        const [w, h] = image.size_mm;
        const real = layer.interval > 0 ? layer.interval : 0.1;
        const side = Math.min(Math.max(32, Math.round(max_px)), 4096);
        const interval = region_mm ? real : Math.max(real, Math.max(w, h) / side);
        const [x0, y0, x1, y1] = region_mm ?? [0, 0, w, h];
        const width = Math.min(side, Math.max(1, Math.round((Math.min(x1, w) - Math.max(x0, 0)) / interval)));
        const height = Math.min(side, Math.max(1, Math.round((Math.min(y1, h) - Math.max(y0, 0)) / interval)));
        const covered = [Math.max(x0, 0), Math.max(y0, 0), Math.min(x1, w), Math.min(y1, h)] as [number, number, number, number];
        return {
          png_base64: simulatedDither(layer, width, height, covered, [w, h]),
          width_px: width,
          height_px: height,
          covered_mm: covered,
          interval_mm: interval,
          exact: interval <= real * (1 + 1e-9),
        };
      }),
  };
}
