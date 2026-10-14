import type { LaserApi } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

/** PNG de 1 x 1 píxel blanco: el modo simulado no tiene imágenes en disco que tramar. */
const BLANK_PNG_BASE64 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAAAAAA6fptVAAAACklEQVR4nGP4DwABAQEAsTj2FAAAAABJRU5ErkJggg==';

/**
 * Vista previa de imágenes del modo simulado: devuelve un cuadro en blanco con las dimensiones y
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
        return {
          png_base64: BLANK_PNG_BASE64,
          width_px: width,
          height_px: height,
          covered_mm: [Math.max(x0, 0), Math.max(y0, 0), Math.min(x1, w), Math.min(y1, h)] as [number, number, number, number],
          interval_mm: interval,
          exact: interval <= real * (1 + 1e-9),
        };
      }),
  };
}
