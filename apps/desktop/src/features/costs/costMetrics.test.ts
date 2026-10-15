import { describe, expect, it } from 'vitest';
import type { DesignObject, Document, JobPreview } from '@/api/types';
import type { JobLogEntry } from '@/features/help/jobLog';
import { ellipseBez, polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { calibrationFactor, evenOddArea, jobMetrics, lastRealSeconds, outputObjects, resolveTime } from './costMetrics';

const RED = '#ff3b4a';
const BLUE = '#2563eb';

const box = (w: number, h: number, x = 0, y = 0) => polylineBez([[x, y], [x + w, y], [x + w, y + h], [x, y + h]], true);

function vector(id: string, paths: ReturnType<typeof box>[], patch: Partial<DesignObject> = {}): DesignObject {
  return { id, layer: RED, transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null, kind: { type: 'vector', paths }, ...patch };
}

const doc = (objects: DesignObject[], layers = [makeLayer(RED, 'Rojo')]): Pick<Document, 'objects' | 'layers'> => ({ objects, layers });

describe('área real con la regla par-impar', () => {
  const sq = (x: number, y: number, s: number): [number, number][] => [[x, y], [x + s, y], [x + s, y + s], [x, y + s]];

  it('un cuadrado y un anillo con hueco', () => {
    expect(evenOddArea([sq(0, 0, 100)])).toBe(10_000);
    expect(evenOddArea([sq(0, 0, 100), sq(30, 30, 40)])).toBe(8_400);
  });

  it('el hueco de un hueco vuelve a sumar', () => {
    expect(evenOddArea([sq(0, 0, 100), sq(10, 10, 80), sq(30, 30, 40)])).toBe(10_000 - 6_400 + 1_600);
  });

  it('piezas separadas suman y los contornos degenerados no cuentan', () => {
    expect(evenOddArea([sq(0, 0, 10), sq(50, 0, 10)])).toBe(200);
    expect(evenOddArea([[[0, 0], [10, 0]]])).toBe(0);
    expect(evenOddArea([])).toBe(0);
  });
});

describe('qué objetos salen en el trabajo', () => {
  const layers = [makeLayer(RED, 'Rojo'), makeLayer(BLUE, 'Azul', { output: false })];

  it('se descartan los ocultos y los de capas sin salida', () => {
    const list = [vector('a', [box(10, 10)]), vector('b', [box(10, 10)], { hidden: true }), vector('c', [box(10, 10)], { layer: BLUE })];
    expect(outputObjects({ objects: list, layers }, null).map((o) => o.id)).toEqual(['a']);
  });

  it('solo la selección filtra por ids', () => {
    const list = [vector('a', [box(10, 10)]), vector('b', [box(10, 10)])];
    expect(outputObjects({ objects: list, layers }, ['b']).map((o) => o.id)).toEqual(['b']);
    expect(outputObjects({ objects: list, layers }, []).map((o) => o.id)).toEqual(['a', 'b']);
  });
});

describe('medidas del trabajo', () => {
  it('sin vista previa hay diseño y áreas pero no tiempo', () => {
    const m = jobMetrics(doc([vector('a', [box(100, 100), box(40, 40, 30, 30)])]), null, null);
    expect(m).toMatchObject({ hasDesign: true, hasPreview: false, totalS: 0, bboxMm2: 10_000, realMm2: 8_400, pieces: 1 });
  });

  it('un diseño vacío no tiene nada', () => {
    expect(jobMetrics(doc([]), null, null)).toMatchObject({ hasDesign: false, bboxMm2: 0, realMm2: 0, pieces: 0 });
  });

  it('un círculo cuesta π r² y las líneas abiertas no tienen área', () => {
    const circle = vector('c', [ellipseBez(0, 0, 10, 10)]);
    expect(jobMetrics(doc([circle]), null, null).realMm2).toBeCloseTo(Math.PI * 100, 0);
    const line = vector('l', [polylineBez([[0, 0], [50, 0]], false)]);
    const m = jobMetrics(doc([line]), null, null);
    expect(m.realMm2).toBe(0);
    expect(m.hasDesign).toBe(true);
  });

  it('la transformación del objeto se aplica a la caja y al área', () => {
    const o = vector('a', [box(10, 10)], { transform: [2, 0, 0, 2, 5, 5] });
    const m = jobMetrics(doc([o]), null, null);
    expect(m.bboxMm2).toBe(400);
    expect(m.realMm2).toBeCloseTo(400, 6);
  });

  it('las copias virtuales suman piezas, área y caja', () => {
    const o = vector('a', [box(10, 10)], { array: { type: 'grid', cols: 3, rows: 2, dx: 5, dy: 5 } });
    const m = jobMetrics(doc([o]), null, null);
    expect(m.pieces).toBe(6);
    expect(m.realMm2).toBe(600);
    expect(m.bboxMm2).toBe((3 * 10 + 2 * 5) * (2 * 10 + 5));
  });

  it('un grupo cuenta como una pieza', () => {
    const list = [vector('a', [box(10, 10)], { group: 'g' }), vector('b', [box(5, 5, 20)], { group: 'g' }), vector('c', [box(5, 5, 40)])];
    expect(jobMetrics(doc(list), null, null).pieces).toBe(2);
  });

  it('tiempo, tiempo con láser y longitud de corte salen de la vista previa', () => {
    const preview: JobPreview = {
      layers: [{ color: RED, mode: 'line', seconds: 5, summary: 'Rojo', travel_saved_pct: null, explanations: [] }],
      moves: [0, 0, 0, 0, 1, 100, 0, 80, 0, 100, 10, 0, 1, 0, 10, 40],
      move_times: [0, 2, 3, 5],
      total_s: 6,
      bounds: [[0, 0], [100, 10]],
      order_note: null,
      warnings: [],
    };
    const m = jobMetrics(doc([vector('a', [box(100, 10)])]), preview, null);
    expect(m.hasPreview).toBe(true);
    expect(m.totalS).toBe(6);
    expect(m.cutMm).toBe(200);
    expect(m.laserOnS).toBe(4);
    expect(m.approximate).toBe(false);
  });
});

const entry = (patch: Partial<JobLogEntry> & { actualS: number }): JobLogEntry => ({
  id: String(Math.random()),
  at: 1,
  machine: 'Falcon',
  document: null,
  estimatedS: 100,
  result: 'finished',
  message: '',
  ...patch,
});

describe('calibración con el registro de trabajos', () => {
  it('la mediana de real ÷ estimado, con al menos 3 trabajos de la máquina', () => {
    const log = [entry({ actualS: 150 }), entry({ actualS: 200 }), entry({ actualS: 120 }), entry({ actualS: 999, machine: 'Otra' })];
    expect(calibrationFactor(log, 'Falcon')).toEqual({ factor: 1.5, jobs: 3 });
    expect(calibrationFactor(log.slice(0, 2), 'Falcon')).toBeNull();
  });

  it('la mediana resiste un trabajo raro y con par promedia los dos del centro', () => {
    const log = [entry({ actualS: 130 }), entry({ actualS: 140 }), entry({ actualS: 135 }), entry({ actualS: 1000 })];
    expect(calibrationFactor(log, 'Falcon')!.factor).toBeCloseTo(1.375, 6);
  });

  it('cancelados, con error, sin estimado o sin duración no cuentan', () => {
    const log = [
      entry({ actualS: 150, result: 'cancelled' }),
      entry({ actualS: 150, result: 'error' }),
      entry({ actualS: 150, estimatedS: null }),
      entry({ actualS: 0 }),
      entry({ actualS: 110 }),
    ];
    expect(calibrationFactor(log, 'Falcon')).toBeNull();
    expect(lastRealSeconds(log, 'Falcon')).toBe(150);
  });

  const m = { totalS: 100, laserOnS: 60 };
  const three = [entry({ actualS: 150 }), entry({ actualS: 150 }), entry({ actualS: 150 })];

  it('resolveTime: estimado, calibrado o el real del último trabajo', () => {
    expect(resolveTime('estimate', m, three, 'Falcon')).toEqual({ runS: 100, laserOnS: 60, used: 'estimate', factor: null });
    expect(resolveTime('calibrated', m, three, 'Falcon')).toEqual({ runS: 150, laserOnS: 90, used: 'calibrated', factor: 1.5 });
    const real = resolveTime('lastReal', m, [entry({ actualS: 200 })], 'Falcon');
    expect(real.runS).toBe(200);
    expect(real.laserOnS).toBeCloseTo(120, 6);
    expect(real.used).toBe('lastReal');
  });

  it('sin historial suficiente cae en el estimado y lo dice', () => {
    expect(resolveTime('calibrated', m, [], 'Falcon').used).toBe('estimate');
    expect(resolveTime('lastReal', m, three, 'Otra').used).toBe('estimate');
    expect(resolveTime('calibrated', m, three.slice(0, 2), 'Falcon').used).toBe('estimate');
  });
});
