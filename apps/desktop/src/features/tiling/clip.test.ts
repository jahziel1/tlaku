import { describe, expect, it } from 'vitest';
import { mockComputeJob } from '@/api/mock/job';
import { sampleDevices } from '@/api/mock/sample';
import type { DeviceProfile, Document, JobOptions } from '@/api/types';
import { DEFAULT_JOB_OPTIONS } from '@/store/preview';
import { rectObject } from '@/features/nest/testUtils';
import { factoryLayer } from '@/lib/layers';
import { clipMoves, clipSegment, type Move } from './clip';
import { mockPlanTiles } from '@/api/mock/tilePlan';
import type { TileSpec } from './types';

const rectTile = (x0: number, y0: number, x1: number, y1: number, offset: [number, number] = [0, 0]): TileSpec => ({ rect: [[x0, y0], [x1, y1]], offset });

const length = (moves: Move[]) => {
  let total = 0;
  let prev: [number, number] | null = null;
  for (const m of moves) {
    if (m[0] === 1 && prev) total += Math.hypot(m[1] - prev[0], m[2] - prev[1]);
    prev = [m[1], m[2]];
  }
  return total;
};

describe('clipSegment', () => {
  const r: TileSpec['rect'] = [[0, 0], [10, 10]];
  it('devuelve la parte de dentro, entera o nada', () => {
    expect(clipSegment(r, [2, 2], [8, 8])).toEqual([0, 1]);
    expect(clipSegment(r, [-10, 5], [20, 5])).toEqual([1 / 3, 2 / 3]);
    expect(clipSegment(r, [11, 0], [11, 10])).toBeNull();
    expect(clipSegment(r, [-5, -5], [-1, 20])).toBeNull();
  });
  it('un segmento sobre el borde cuenta como dentro', () => {
    expect(clipSegment(r, [0, 0], [10, 0])).toEqual([0, 1]);
  });
});

describe('clipMoves', () => {
  it('parte el corte en el borde y desplaza', () => {
    const moves: Move[] = [[0, -5, 5, 0], [1, 15, 5, 50]];
    expect(clipMoves(moves, rectTile(0, 0, 10, 10, [1, 2]))).toEqual([[0, 1, 7, 0], [1, 11, 7, 50]]);
  });
  it('descarta lo que cae fuera y los viajes', () => {
    const moves: Move[] = [[0, 20, 20, 0], [1, 30, 20, 50], [0, 1, 1, 0], [1, 2, 2, 50]];
    expect(clipMoves(moves, rectTile(0, 0, 10, 10))).toEqual([[0, 1, 1, 0], [1, 2, 2, 50]]);
  });
  it('un cuadrado cortado en 4 tramos suma el perímetro más los solapes', () => {
    const sq: Move[] = [[0, 0, 0, 0], [1, 100, 0, 50], [1, 100, 60, 50], [1, 0, 60, 50], [1, 0, 0, 50]];
    const r = mockPlanTiles({ design: [[0, 0], [100, 60]], size: [55, 35], overlap: 10, order: 'rows', margin: 0 });
    if (!r.ok) throw new Error(r.error.kind);
    const total = r.plan.tiles.reduce((sum, t) => sum + length(clipMoves(sq, t.spec)), 0);
    expect(total).toBeCloseTo(320 + 40);
  });
});

describe('mockComputeJob con tramo', () => {
  const device: DeviceProfile = { ...sampleDevices()[0]!, bed: [55, 35] };
  const doc: Document = {
    version: 2,
    device_id: null,
    layers: [factoryLayer('#ff0000', 'Rojo')],
    objects: [rectObject('a', 100, 60)],
  };
  const options = (tile: TileSpec | null): JobOptions => ({ ...DEFAULT_JOB_OPTIONS, tile });

  it('cada tramo cabe en la cama y el simulador no avisa de que se salga', () => {
    const r = mockPlanTiles({ design: [[0, 0], [100, 60]], size: [55, 35], overlap: 10, order: 'rows', margin: 0 });
    if (!r.ok) throw new Error(r.error.kind);
    for (const { spec } of r.plan.tiles) {
      const job = mockComputeJob(doc, device, options(spec));
      expect(job.moves.length).toBeGreaterThan(0);
      for (let i = 0; i < job.moves.length; i += 4) {
        expect(job.moves[i + 1]).toBeGreaterThanOrEqual(-1e-9);
        expect(job.moves[i + 1]).toBeLessThanOrEqual(55 + 1e-9);
        expect(job.moves[i + 2]).toBeGreaterThanOrEqual(-1e-9);
        expect(job.moves[i + 2]).toBeLessThanOrEqual(35 + 1e-9);
      }
      expect(job.warnings.some((w) => w.includes('se sale de la cama'))).toBe(false);
    }
  });

  it('sin tramo, el mismo diseño se sale de la cama', () => {
    expect(mockComputeJob(doc, device, options(null)).warnings.some((w) => w.includes('se sale de la cama'))).toBe(true);
  });
});
