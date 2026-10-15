import type { BezPath } from '@/api/types';
import { bezBounds } from '@/lib/bezpath';

const r = (n: number) => Math.round(n * 100) / 100;

/** Datos de un trayecto SVG con Y hacia abajo (el motor trabaja con Y hacia arriba). */
export function previewPathData(paths: readonly BezPath[]): string {
  return paths
    .map((path) =>
      path
        .map((s) => {
          switch (s.type) {
            case 'move_to':
              return `M${r(s.to[0])} ${r(-s.to[1])}`;
            case 'line_to':
              return `L${r(s.to[0])} ${r(-s.to[1])}`;
            case 'quad_to':
              return `Q${r(s.ctrl[0])} ${r(-s.ctrl[1])} ${r(s.to[0])} ${r(-s.to[1])}`;
            case 'cubic_to':
              return `C${r(s.ctrl1[0])} ${r(-s.ctrl1[1])} ${r(s.ctrl2[0])} ${r(-s.ctrl2[1])} ${r(s.to[0])} ${r(-s.to[1])}`;
            case 'close':
              return 'Z';
          }
        })
        .join(''),
    )
    .join('');
}

/** Caja común a varias muestras, para que "sin" y "con" se vean a la misma escala. */
export function previewBox(...groups: readonly (readonly BezPath[])[]): { x: number; y: number; w: number; h: number } {
  const b = bezBounds(groups.flat());
  if (!b) return { x: 0, y: -10, w: 10, h: 12 };
  const pad = 1;
  return { x: b[0][0] - pad, y: -b[1][1] - pad, w: Math.max(1, b[1][0] - b[0][0] + 2 * pad), h: Math.max(1, b[1][1] - b[0][1] + 2 * pad) };
}

interface Props {
  paths: readonly BezPath[];
  box: { x: number; y: number; w: number; h: number };
  title: string;
  muted?: boolean;
}

/** Miniatura de una muestra de texto (decorativa: el nombre y el estado los dice la casilla). */
export function FeaturePreview({ paths, box, title, muted }: Props) {
  return (
    <svg
      className={muted ? 'ot-preview ot-preview--off' : 'ot-preview'}
      viewBox={`${r(box.x)} ${r(box.y)} ${r(box.w)} ${r(box.h)}`}
      preserveAspectRatio="xMidYMid meet"
      role="img"
      aria-label={title}
    >
      <title>{title}</title>
      <path d={previewPathData(paths)} fillRule="evenodd" />
    </svg>
  );
}
