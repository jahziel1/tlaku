import type { Point } from '@/api/types';
import './design.css';

export interface Outline {
  points: Point[];
  closed: boolean;
}

/** Capa de la vista previa: contornos con un tono (clase `dg-tone--*`). */
export interface PreviewLayer {
  outlines: readonly Outline[];
  tone: 'original' | 'result' | 'ghost' | 'accent' | 'warn';
}

export interface PreviewCircle {
  center: Point;
  radius: number;
  tone: PreviewLayer['tone'];
}

const pathData = (list: readonly Outline[]): string =>
  list.map((p) => `M${p.points.map(([x, y]) => `${x.toFixed(3)} ${y.toFixed(3)}`).join('L')}${p.closed ? 'Z' : ''}`).join('');

/** Rectángulo `[[x0, y0], [x1, y1]]` como contorno cerrado. */
export const boxOutline = ([[x0, y0], [x1, y1]]: [Point, Point]): Outline => ({ points: [[x0, y0], [x1, y0], [x1, y1], [x0, y1]], closed: true });

/**
 * Vista previa en mm (Y hacia arriba): ajusta el encuadre a todo lo que se dibuja. Es un dibujo
 * descriptivo: lleva `role="img"` y la etiqueta que se le pasa.
 */
export function DesignPreview({ layers, circles = [], label, height = 220 }: { layers: readonly PreviewLayer[]; circles?: readonly PreviewCircle[]; label: string; height?: number }) {
  const xs: number[] = [];
  const ys: number[] = [];
  for (const l of layers) {
    for (const o of l.outlines) {
      for (const [x, y] of o.points) {
        xs.push(x);
        ys.push(y);
      }
    }
  }
  for (const c of circles) {
    xs.push(c.center[0] - c.radius, c.center[0] + c.radius);
    ys.push(c.center[1] - c.radius, c.center[1] + c.radius);
  }
  if (xs.length === 0) return <div className="ar-preview ar-preview--empty" style={{ height }} />;
  const [x0, x1] = [Math.min(...xs), Math.max(...xs)];
  const [y0, y1] = [Math.min(...ys), Math.max(...ys)];
  const pad = Math.max(x1 - x0, y1 - y0, 1) * 0.06;
  return (
    <svg className="ar-preview" style={{ height }} role="img" aria-label={label} viewBox={`${x0 - pad} ${-(y1 + pad)} ${x1 - x0 + 2 * pad} ${y1 - y0 + 2 * pad}`} preserveAspectRatio="xMidYMid meet">
      <g transform="scale(1 -1)">
        {layers.map((l, i) => (
          <path key={i} className={`dg-line dg-tone--${l.tone}`} d={pathData(l.outlines)} />
        ))}
        {circles.map((c, i) => (
          <circle key={i} className={`dg-line dg-tone--${c.tone}`} cx={c.center[0]} cy={c.center[1]} r={c.radius} />
        ))}
      </g>
    </svg>
  );
}

/** Miniatura de un resultado (relleno par-impar) dentro de una caja común `box`, para comparar varias. */
export function Thumb({ outlines, box, label }: { outlines: readonly Outline[]; box: [Point, Point]; label: string }) {
  const [[x0, y0], [x1, y1]] = box;
  const pad = Math.max(x1 - x0, y1 - y0, 1) * 0.06;
  return (
    <svg className="dg-thumb" role="img" aria-label={label} viewBox={`${x0 - pad} ${-(y1 + pad)} ${x1 - x0 + 2 * pad} ${y1 - y0 + 2 * pad}`} preserveAspectRatio="xMidYMid meet">
      <g transform="scale(1 -1)">
        <path className="dg-thumb__shape" d={pathData(outlines)} />
      </g>
    </svg>
  );
}
