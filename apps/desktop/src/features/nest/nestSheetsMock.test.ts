import { describe, expect, it } from 'vitest';
import { mockNest, MOCK_NEST_MS } from '@/api/mock/nest';
import { selectionUnits } from '@/features/arrange/units';
import { buildNestRequest, DEFAULT_NEST_OPTIONS, MAX_NEST_SHEETS } from './request';
import type { Remnant } from './remnants';
import { docOf, rectObject } from './testUtils';
import type { NestPieceRotation, NestRequest } from './types';

/** Mock con reloj controlado y sin latencia. */
function setup() {
  let clock = 1000;
  const api = mockNest((f) => Promise.resolve().then(f), () => clock);
  return { api, tick: (ms: number) => (clock += ms) };
}

const bed = { type: 'rect', width: 100, height: 100 } as const;
const remnant = (id: string, w: number, h: number): Remnant => ({
  id,
  name: id,
  material: 'Triplay',
  thickness_mm: 3,
  width_mm: w,
  height_mm: h,
  area_mm2: w * h,
  rect: true,
  paths: [[[0, 0], [w, 0], [w, h], [0, h]]],
  created_ms: 1,
});

function requestOf(over: Partial<NestRequest> = {}, opts: Partial<typeof DEFAULT_NEST_OPTIONS> = {}, remnants: Remnant[] = [], rotations: Record<string, NestPieceRotation> = {}, copies = 6) {
  const objects = [rectObject('a', 40, 40)];
  const units = selectionUnits(docOf(objects), ['a']);
  return {
    ...buildNestRequest({
      units,
      copies: { a: copies },
      container: bed,
      options: { ...DEFAULT_NEST_OPTIONS, spacing: 0, margin: 0, useRemnants: true, ...opts },
      rotations,
      remnants,
    }),
    ...over,
  };
}

async function finish(api: ReturnType<typeof setup>['api'], tick: (ms: number) => number, request: NestRequest) {
  const id = await api.nestStart({ request });
  tick(MOCK_NEST_MS * 3);
  return api.nestPoll({ job_id: id });
}

describe('nest simulado: varias hojas y retazos', () => {
  it('con una sola hoja lo que no cabe se reporta, como antes', async () => {
    const { api, tick } = setup();
    const st = await finish(api, tick, requestOf());
    expect(st.sheets).toHaveLength(1);
    expect(st.placed).toBe(4);
    expect(st.unplaced).toHaveLength(2);
    expect(st.placements.every((p) => p.sheet === 0)).toBe(true);
  });

  it('con hojas nuevas reparte las piezas y resume cada hoja', async () => {
    const { api, tick } = setup();
    const st = await finish(api, tick, requestOf({}, { multiSheet: true, maxSheets: 3 }));
    expect(st.unplaced).toEqual([]);
    expect(st.sheets.map((s) => [s.index, s.placed, s.source])).toEqual([[0, 4, 'container'], [1, 2, 'container']]);
    expect(st.sheets[0]!.utilization).toBeCloseTo(64, 5);
    expect(st.sheets[1]!.utilization).toBeCloseTo(32, 5);
    // aprovechamiento global: 6 piezas de 1600 entre 2 hojas de 10000
    expect(st.utilization).toBeCloseTo(48, 5);
    expect(st.placements.filter((p) => p.sheet === 1)).toHaveLength(2);
    expect(st.sheets[0]!.bounds).toEqual([0, 0, 100, 100]);
  });

  it('el tope de hojas se respeta', async () => {
    const { api, tick } = setup();
    const st = await finish(api, tick, requestOf({}, { multiSheet: true, maxSheets: 1 }, [], {}, 9));
    expect(st.sheets).toHaveLength(1);
    expect(st.unplaced).toHaveLength(5);
  });

  it('los retazos van antes que la hoja nueva y el que no admite nada se salta', async () => {
    const { api, tick } = setup();
    const rems = [remnant('chico', 10, 10), remnant('bueno', 90, 50)];
    const st = await finish(api, tick, requestOf({}, { multiSheet: true, maxSheets: 2 }, rems, {}, 3));
    expect(st.unplaced).toEqual([]);
    expect(st.sheets.map((s) => [s.source, s.remnant_id, s.placed])).toEqual([['remnant', 'bueno', 2], ['container', null, 1]]);
    expect(st.sheets[0]!.bounds).toEqual([0, 0, 90, 50]);
    expect(st.sheets[0]!.utilization).toBeCloseTo((2 * 1600) / (90 * 50) * 100, 5);
  });

  it('con useRemnants apagado los retazos no entran en la petición', () => {
    const req = requestOf({}, { useRemnants: false }, [remnant('bueno', 90, 50)]);
    expect(req.remnants).toEqual([]);
    expect(req.max_sheets).toBe(1);
    const on = requestOf({}, { useRemnants: true, multiSheet: true, maxSheets: 4 }, [remnant('bueno', 90, 50)]);
    expect(on.remnants.map((r) => r.id)).toEqual(['bueno']);
    expect(on.max_sheets).toBe(5);
  });

  it('el sobrante solo llega al terminar y si hay franja aprovechable', async () => {
    const { api, tick } = setup();
    const req = requestOf({ leftover_min_mm: 10 }, {}, [], {}, 2);
    const id = await api.nestStart({ request: req });
    tick(MOCK_NEST_MS / 4);
    const mid = await api.nestPoll({ job_id: id });
    expect(mid.running).toBe(true);
    expect(mid.sheets.every((s) => s.leftovers.length === 0)).toBe(true);
    tick(MOCK_NEST_MS * 2);
    const done = await api.nestPoll({ job_id: id });
    expect(done.sheets[0]!.leftovers).toHaveLength(1);
    const ring = done.sheets[0]!.leftovers[0]![0]!;
    expect(ring).toHaveLength(4);
    // la franja queda sobre las piezas (40 de alto) y dentro de la hoja
    expect(Math.min(...ring.map((p) => p[1]))).toBeGreaterThanOrEqual(40);
    expect(Math.max(...ring.map((p) => p[1]))).toBeLessThanOrEqual(100);
    const none = await finish(api, tick, requestOf({ leftover_min_mm: 0 }, {}, [], {}, 2));
    expect(none.sheets[0]!.leftovers).toEqual([]);
    const full = await finish(api, tick, requestOf({ leftover_min_mm: 30 }, {}, [], {}, 4));
    // 4 piezas llenan hasta 80 y sobran 20: menos que el ancho mínimo pedido
    expect(full.sheets[0]!.leftovers).toEqual([]);
  });

  it('la veta y el sin giro impiden girar 90°', async () => {
    const { api, tick } = setup();
    const narrow = { type: 'rect', width: 100, height: 30 } as const;
    const bar = (rotation: NestPieceRotation) => {
      const units = selectionUnits(docOf([rectObject('bar', 10, 80)]), ['bar']);
      return buildNestRequest({ units, copies: {}, container: narrow, options: { ...DEFAULT_NEST_OPTIONS, spacing: 0, margin: 0 }, rotations: { bar: rotation } });
    };
    expect((await finish(api, tick, bar('inherit'))).placed).toBe(1);
    expect((await finish(api, tick, bar('grain'))).placed).toBe(0);
    expect((await finish(api, tick, bar('none'))).placed).toBe(0);
  });

  it('valida el máximo de hojas y los retazos', async () => {
    const { api } = setup();
    await expect(api.nestStart({ request: requestOf({ max_sheets: 0 }) })).rejects.toThrow('máximo de hojas');
    await expect(api.nestStart({ request: requestOf({ remnants: [{ id: 'malo', paths: [[[0, 0], [1, 1]]] }] }) })).rejects.toThrow('«malo»');
  });

  it('el tope de hojas nuevas de la interfaz queda dentro del contrato', () => {
    const req = requestOf({}, { multiSheet: true, maxSheets: 9999 });
    expect(req.max_sheets).toBe(MAX_NEST_SHEETS);
    expect(requestOf({}, { multiSheet: true, maxSheets: 0 }).max_sheets).toBe(1);
    expect(requestOf({}, { remnantMinMm: 1 }).leftover_min_mm).toBe(5);
    expect(requestOf({}, { remnantMinMm: 99999 }).leftover_min_mm).toBe(500);
  });
});
