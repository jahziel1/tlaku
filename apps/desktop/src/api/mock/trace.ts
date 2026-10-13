import { ellipseBez, polylineBez, roundedRectBez, type BezPath } from '@/lib/bezpath';
import type { LaserApi } from '../api';
import type { TraceSettings } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

/** Ancho de imagen que se supone para pasar de píxeles a mm (la simulada no tiene píxeles reales). */
const MOCK_WIDTH_PX = 400;

/** Elipse aproximada con rectas (modo polígono): más lados cuanto menos suavizado. */
function polygonEllipse(cx: number, cy: number, rx: number, ry: number, smoothing: number): BezPath {
  const n = 8 + Math.round((1 - smoothing) * 40);
  const pts = Array.from({ length: n }, (_, i): [number, number] => {
    const a = (i / n) * Math.PI * 2;
    return [cx + rx * Math.cos(a), cy + ry * Math.sin(a)];
  });
  return polylineBez(pts, true);
}

/**
 * Resultado plausible: un disco, un anillo (con hueco), una barra con esquinas redondeadas y
 * unas motas. Crece con el umbral, se vuelve rectas en modo polígono, pierde las motas al subir
 * "eliminar manchas" y con `invert` pasa a ser el fondo con las formas como huecos.
 */
export function mockTracePaths(settings: TraceSettings, [w, h]: [number, number]): BezPath[] {
  const grow = 0.35 + settings.threshold * 1.3; // 0.35..1.65
  const unit = Math.min(w, h);
  const round = (cx: number, cy: number, r: number): BezPath =>
    settings.mode === 'polygon'
      ? polygonEllipse(cx, cy, r, r, settings.smoothing)
      : ellipseBez(cx, cy, r, r);

  const shapes: BezPath[] = [];
  const rDisc = 0.16 * unit * grow;
  if (rDisc > 0.2) shapes.push(round(0.28 * w, 0.62 * h, rDisc));

  // Anillo: contorno exterior y hueco en la misma trayectoria (regla par-impar).
  const rOuter = 0.17 * unit * grow;
  const rInner = Math.max(0, rOuter - 0.1 * unit * (0.5 + settings.threshold * 0.5));
  if (rOuter > 0.2) {
    const outer = round(0.7 * w, 0.62 * h, rOuter);
    shapes.push(rInner > 0.2 ? [...outer, ...round(0.7 * w, 0.62 * h, rInner)] : outer);
  }

  const barH = 0.05 * unit * grow;
  if (barH > 0.1) {
    const bar = roundedRectBez(0.12 * w, 0.18 * h - barH, 0.88 * w, 0.18 * h + barH, settings.mode === 'polygon' ? 0 : barH / 2);
    shapes.push(bar);
  }

  // Motas de ~3 px de lado: sobreviven solo si "eliminar manchas" no pasa de 3 px.
  const mmPx = w / MOCK_WIDTH_PX;
  if (settings.speckle_px <= 3) {
    for (const [fx, fy] of [[0.5, 0.45], [0.46, 0.85], [0.9, 0.33], [0.08, 0.4]] as const) {
      shapes.push(roundedRectBez(fx * w, fy * h, fx * w + 3 * mmPx, fy * h + 3 * mmPx));
    }
  }

  if (!settings.invert) return shapes;
  // Invertido: el rectángulo completo con las formas como huecos.
  return [[...polylineBez([[0, 0], [w, 0], [w, h], [0, h]], true), ...shapes.flat()]];
}

/** Métodos simulados del área `trace`: formas de ejemplo que cambian con los ajustes. */
export function mockTrace(lag: Lag): Pick<LaserApi, 'traceImage'> {
  return {
    traceImage: ({ path, settings, size_mm }) =>
      lag(() => {
        if (!path) throw new Error('La imagen no tiene una ruta de archivo');
        if (!(size_mm[0] > 0 && size_mm[1] > 0)) throw new Error('El tamaño de salida en mm debe ser positivo y finito');
        const paths = mockTracePaths(settings, size_mm);
        const warnings: string[] = [];
        if (settings.speckle_px > 3) {
          warnings.push(`Se descartaron manchas menores de ${settings.speckle_px} px de lado`);
        }
        if (paths.length === 0) {
          warnings.push(
            'La imagen no tiene zonas que trazar con estos ajustes; revisa el umbral o la opción de invertir',
          );
        }
        return { paths, warnings };
      }),
  };
}
