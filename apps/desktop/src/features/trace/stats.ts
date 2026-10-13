import type { BezPath } from '@/api/types';
import { ROUGH_NODES, ROUGH_SHAPES } from './settings';

export interface TraceStats {
  shapes: number;
  /** Nodos = puntos de anclaje (sin contar los controles de las curvas). */
  nodes: number;
}

export function traceStats(paths: readonly BezPath[]): TraceStats {
  let nodes = 0;
  for (const path of paths) for (const seg of path) if (seg.type !== 'close') nodes++;
  return { shapes: paths.length, nodes };
}

/** Muchas formas o muchos nodos: señal de un resultado áspero que conviene limpiar. */
export function looksRough(stats: TraceStats): boolean {
  return stats.shapes > ROUGH_SHAPES || stats.nodes > ROUGH_NODES;
}

const num = (n: number) => String(Math.round(n * 1000) / 1000);

/** Datos de un `<path>` SVG con las curvas tal cual (mm). Rellenar con la regla par-impar. */
export function bezToSvgD(paths: readonly BezPath[]): string {
  const out: string[] = [];
  for (const path of paths) {
    for (const s of path) {
      switch (s.type) {
        case 'move_to':
          out.push(`M${num(s.to[0])} ${num(s.to[1])}`);
          break;
        case 'line_to':
          out.push(`L${num(s.to[0])} ${num(s.to[1])}`);
          break;
        case 'quad_to':
          out.push(`Q${num(s.ctrl[0])} ${num(s.ctrl[1])} ${num(s.to[0])} ${num(s.to[1])}`);
          break;
        case 'cubic_to':
          out.push(
            `C${num(s.ctrl1[0])} ${num(s.ctrl1[1])} ${num(s.ctrl2[0])} ${num(s.ctrl2[1])} ${num(s.to[0])} ${num(s.to[1])}`,
          );
          break;
        case 'close':
          out.push('Z');
          break;
      }
    }
  }
  return out.join('');
}
