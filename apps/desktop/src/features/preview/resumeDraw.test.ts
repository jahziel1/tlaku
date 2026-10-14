import { describe, expect, it } from 'vitest';
import type { ResumeView } from '@/api/types';
import { drawResume, fitBounds, project, unproject } from './resumeDraw';

function view(): ResumeView {
  // Punto de partida (0,0), un corte a (10,0), un desplazamiento a (10,10) y un corte a (0,10).
  return {
    job: {} as ResumeView['job'],
    moves: [0, 0, 0, 0, 1, 10, 0, 50, 0, 10, 10, 0, 1, 0, 10, 50],
    move_lines: [0, 1, 2, 3],
    bounds: [
      [0, 0],
      [10, 10],
    ],
    last_acked_move: 2,
    suggested: { line: 2, move_index: 2, at: null, executing_line: 1, distance_mm: 0, mismatch: false, row_restart: false, percent: 50, z_delta: 0 },
    warnings: [],
  };
}

describe('resumeDraw', () => {
  it('fits the job into the canvas and the projection round-trips', () => {
    const fit = fitBounds([[0, 0], [10, 10]], 116, 116, 8);
    expect(fit.scale).toBeCloseTo(10, 6);
    const [px, py] = project(fit, 0, 0);
    expect(px).toBeCloseTo(8, 6);
    expect(py).toBeCloseTo(108, 6); // la Y crece hacia arriba
    const [x, y] = unproject(fit, ...project(fit, 3.5, 7.25));
    expect(x).toBeCloseTo(3.5, 6);
    expect(y).toBeCloseTo(7.25, 6);
  });

  it('strokes done cuts solid and the rest dashed, around the restart point', () => {
    const calls: string[] = [];
    const ctx = {
      save: () => undefined,
      restore: () => undefined,
      clearRect: () => undefined,
      fillRect: () => undefined,
      beginPath: () => calls.push('begin'),
      moveTo: () => undefined,
      lineTo: () => undefined,
      stroke: () => calls.push('stroke'),
      fill: () => calls.push('fill'),
      arc: () => calls.push('arc'),
      setLineDash: (d: number[]) => calls.push(`dash:${d.length}`),
      strokeStyle: '' as string,
      fillStyle: '' as string,
      lineWidth: 1,
    };
    const styles: string[] = [];
    Object.defineProperty(ctx, 'strokeStyle', { set: (v: string) => styles.push(v), get: () => '' });
    drawResume(ctx, 100, 100, view(), 2, null, { done: 'D', todo: 'T', travel: 'V', marker: 'M', bed: 'B' });
    expect(styles.slice(0, 3)).toEqual(['V', 'D', 'T']);
    // El trazo discontinuo va después del continuo y se restablece para el marcador.
    expect(calls.indexOf('dash:2')).toBeGreaterThan(calls.indexOf('dash:0'));
    expect(calls.filter((c) => c === 'arc')).toHaveLength(2);
  });
});
