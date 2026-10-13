import { describe, expect, it } from 'vitest';
import { mockNest, MOCK_NEST_MS } from '@/api/mock/nest';
import { applyAffine, objectBounds } from '@/lib/geometry';
import { buildNestRequest, DEFAULT_NEST_OPTIONS } from './request';
import { selectionUnits } from '@/features/arrange/units';
import { docOf, rectObject } from './testUtils';
import type { NestRequest, NestStatus } from './types';

/** Mock con reloj controlado y sin latencia. */
function setup() {
  let clock = 1000;
  const api = mockNest((f) => Promise.resolve().then(f), () => clock);
  return { api, tick: (ms: number) => (clock += ms) };
}

const requestOf = (objects: ReturnType<typeof rectObject>[], copies: Record<string, number>, container: NestRequest['container'], over: Partial<NestRequest> = {}): NestRequest => {
  const units = selectionUnits(docOf(objects), objects.map((o) => o.id));
  return { ...buildNestRequest({ units, copies, container, options: { ...DEFAULT_NEST_OPTIONS, spacing: 2, margin: 3 } }), ...over };
};

/** Cajas de las piezas ya colocadas, aplicando el transform del acomodo. */
function boxes(req: NestRequest, st: NestStatus) {
  return st.placements.map((p) => {
    const piece = req.pieces.find((x) => x.id === p.piece)!;
    const [[x0, y0], [x1, y1]] = objectBounds(piece.objects[0]!)!;
    const pts = [applyAffine(p.transform, [x0, y0]), applyAffine(p.transform, [x1, y1])];
    return [Math.min(pts[0]![0], pts[1]![0]), Math.min(pts[0]![1], pts[1]![1]), Math.max(pts[0]![0], pts[1]![0]), Math.max(pts[0]![1], pts[1]![1])] as const;
  });
}

describe('nest simulado', () => {
  const bed = { type: 'rect', width: 100, height: 60 } as const;

  it('es progresivo: revela más piezas con el tiempo y termina a los 1,5 s', async () => {
    const { api, tick } = setup();
    const req = requestOf([rectObject('a', 20, 10)], { a: 8 }, bed);
    const id = await api.nestStart({ request: req });
    const s0 = await api.nestPoll({ job_id: id });
    expect(s0).toMatchObject({ running: true, placed: 0, total: 8, error: null, timed_out: false });
    tick(MOCK_NEST_MS / 2);
    const s1 = await api.nestPoll({ job_id: id });
    expect(s1.running).toBe(true);
    expect(s1.placed).toBe(4);
    expect(s1.placements).toHaveLength(4);
    expect(s1.utilization).toBeGreaterThan(0);
    tick(MOCK_NEST_MS);
    const s2 = await api.nestPoll({ job_id: id });
    expect(s2).toMatchObject({ running: false, placed: 8, unplaced: [], elapsed_ms: MOCK_NEST_MS });
    expect(s2.utilization).toBeGreaterThan(s1.utilization);
    expect(s2.utilization).toBeCloseTo(((8 * 200) / (100 * 60)) * 100, 5);
  });

  it('respeta separación, margen y contenedor, sin que se toquen', async () => {
    const { api, tick } = setup();
    const req = requestOf([rectObject('a', 20, 10, [5, 5])], { a: 10 }, bed);
    const id = await api.nestStart({ request: req });
    tick(5000);
    const st = await api.nestPoll({ job_id: id });
    const bs = boxes(req, st);
    expect(bs).toHaveLength(10);
    for (const [x0, y0, x1, y1] of bs) {
      expect(x0).toBeGreaterThanOrEqual(3 - 1e-6);
      expect(y0).toBeGreaterThanOrEqual(3 - 1e-6);
      expect(x1).toBeLessThanOrEqual(97 + 1e-6);
      expect(y1).toBeLessThanOrEqual(57 + 1e-6);
    }
    bs.forEach((p, i) =>
      bs.slice(i + 1).forEach((q) => {
        const gapX = Math.max(q[0] - p[2], p[0] - q[2]);
        const gapY = Math.max(q[1] - p[3], p[1] - q[3]);
        expect(Math.max(gapX, gapY)).toBeGreaterThanOrEqual(2 - 1e-6);
      }),
    );
  });

  it('las piezas que no caben van a unplaced; gira 90° si así cabe', async () => {
    const { api, tick } = setup();
    // 70 de alto no cabe en 60, pero girada (70 de ancho) sí en 100 de ancho
    const req = requestOf([rectObject('tall', 10, 52), rectObject('big', 500, 500)], { tall: 1, big: 2 }, bed, { rotation: { mode: 'quarter', step_deg: 15 } });
    const id = await api.nestStart({ request: req });
    tick(5000);
    const st = await api.nestPoll({ job_id: id });
    expect(st.running).toBe(false);
    expect(st.placed).toBe(1);
    expect(st.total).toBe(3);
    expect(st.unplaced).toEqual([{ piece: 'big', copy: 0 }, { piece: 'big', copy: 1 }]);

    const narrow = { type: 'rect', width: 100, height: 30 } as const;
    const rot = requestOf([rectObject('bar', 10, 80)], { bar: 1 }, narrow, { margin: 0 });
    const rid = await api.nestStart({ request: rot });
    tick(5000);
    const r = await api.nestPoll({ job_id: rid });
    expect(r.placements[0]).toMatchObject({ rotation_deg: 90 });
    const none = requestOf([rectObject('bar', 10, 80)], { bar: 1 }, narrow, { margin: 0, rotation: { mode: 'none', step_deg: 15 } });
    const nid = await api.nestStart({ request: none });
    tick(5000);
    const n = await api.nestPoll({ job_id: nid });
    expect(n.placed).toBe(0);
    expect(n.unplaced).toHaveLength(1);
  });

  it('nestCancel congela el avance y lo que llevaba queda en placements', async () => {
    const { api, tick } = setup();
    const req = requestOf([rectObject('a', 20, 10)], { a: 9 }, bed);
    const id = await api.nestStart({ request: req });
    tick(MOCK_NEST_MS / 3);
    await api.nestCancel({ job_id: id });
    tick(10_000);
    const st = await api.nestPoll({ job_id: id });
    expect(st.running).toBe(false);
    expect(st.placed).toBe(3);
    expect(st.unplaced).toHaveLength(6);
    expect(st.timed_out).toBe(false);
  });

  it('un nestStart nuevo cancela el anterior', async () => {
    const { api, tick } = setup();
    const req = requestOf([rectObject('a', 20, 10)], { a: 4 }, bed);
    const first = await api.nestStart({ request: req });
    tick(300);
    const second = await api.nestStart({ request: req });
    tick(10_000);
    expect((await api.nestPoll({ job_id: first })).placed).toBe(0);
    expect((await api.nestPoll({ job_id: second })).placed).toBe(4);
  });

  it('el material es la caja del objeto y el tope de tiempo corta antes', async () => {
    const { api, tick } = setup();
    const material = rectObject('m', 40, 30, [200, 100]);
    const req = requestOf([rectObject('a', 10, 10)], { a: 4 }, { type: 'object', object: material }, { margin: 0, spacing: 0, time_limit_s: 1 });
    const id = await api.nestStart({ request: req });
    tick(5000);
    const st = await api.nestPoll({ job_id: id });
    expect(st.timed_out).toBe(true);
    expect(st.running).toBe(false);
    expect(st.placed).toBeLessThan(4);
    const full = await api.nestStart({ request: { ...req, time_limit_s: 20 } });
    tick(5000);
    const all = await api.nestPoll({ job_id: full });
    expect(all.timed_out).toBe(false);
    expect(all.placed).toBe(4);
    for (const [x0, y0, x1, y1] of boxes(req, all)) {
      expect(x0).toBeGreaterThanOrEqual(200 - 1e-6);
      expect(y0).toBeGreaterThanOrEqual(100 - 1e-6);
      expect(x1).toBeLessThanOrEqual(240 + 1e-6);
      expect(y1).toBeLessThanOrEqual(130 + 1e-6);
    }
    // el área neta del contenedor (40x30) manda en la utilización
    expect(all.utilization).toBeCloseTo((400 / 1200) * 100, 5);
  });

  it('rechaza en español una petición sin piezas o con contenedor inválido', async () => {
    const { api } = setup();
    const ok = requestOf([rectObject('a', 5, 5)], {}, bed);
    await expect(api.nestStart({ request: { ...ok, pieces: [] } })).rejects.toThrow('No hay piezas que acomodar');
    await expect(api.nestStart({ request: { ...ok, container: { type: 'rect', width: 0, height: 10 } } })).rejects.toThrow('ancho y alto positivos');
    await expect(api.nestStart({ request: { ...ok, container: { type: 'object', object: { ...rectObject('m', 1, 1), kind: { type: 'vector', paths: [] } } } } })).rejects.toThrow('contorno cerrado');
    await expect(api.nestStart({ request: { ...ok, time_limit_s: 0 } })).rejects.toThrow('tope de tiempo');
    await expect(api.nestPoll({ job_id: 'nada' })).rejects.toThrow('no existe');
  });
});
