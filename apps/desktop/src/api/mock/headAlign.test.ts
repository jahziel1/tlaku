import { describe, expect, it } from 'vitest';
import { applyH } from '@/features/camera/geometry';
import { createMockApi } from '.';
import { mockHeadPlan, mockHeadRefine } from './headAlign';

const BED: [number, number] = [400, 415];
const api = createMockApi({ latencyMs: 0 });

describe('plan simulado (mismo reparto que el motor)', () => {
  it('rejilla en serpentina dentro del área segura, con la referencia en la primera fila', () => {
    const p = mockHeadPlan(BED, 3, 60);
    expect(p.area).toEqual([60, 60, 340, 355]);
    expect(p.points).toHaveLength(9);
    expect(p.points[0]).toEqual([60, 60]);
    expect(p.points[3]).toEqual([340, 207.5]);
    expect(p.points[5]).toEqual([60, 207.5]);
    expect(p.reference).toEqual([130, 60]);
  });

  it('rechaza lo que el motor rechaza', () => {
    expect(() => mockHeadPlan(BED, 2, 20)).toThrow();
    expect(() => mockHeadPlan(BED, 7, 20)).toThrow();
    expect(() => mockHeadPlan(BED, 3, 200)).toThrow();
  });

  it('el afinado crece alrededor de lo visto y termina al cubrir toda el área', () => {
    const area: [number, number, number, number] = [20, 20, 380, 395];
    expect(mockHeadRefine(area, [[200, 207]], 3)?.area).toEqual([140, 147, 260, 267]);
    expect(mockHeadRefine(area, [], 3)).toBeNull();
    expect(mockHeadRefine(area, [[20, 20], [380, 395]], 3)).toBeNull();
  });
});

describe('solución simulada', () => {
  const frames = (spots: [number, number][]) => spots.map((machine_mm) => ({ png_base64: 'AAAA', machine_mm }));

  it('localiza el cabezal solo en la zona central y ajusta con los puntos vistos', async () => {
    const plan = await api.cameraHeadAlignPlan({ options: { bed: BED, grid: 3, margin_mm: 20 } });
    const r = await api.cameraHeadAlignSolve({ frames: frames([...plan.points, plan.reference]), config: { bed: BED, safe_area: plan.area } });
    expect(r.frames).toHaveLength(10);
    expect(r.detected).toBe(1);
    expect(r.fit).toBeNull();
    expect(r.fit_error).toBeTruthy();
    expect(r.issues).toContainEqual({ kind: 'not_detected', count: 9 });
    expect(r.frames.filter((f) => f.failure === 'no_change')).toHaveLength(9);
  });

  it('con puntos suficientes da una alineación de cabezal con error por punto', async () => {
    const inner: [number, number][] = [];
    for (let x = 100; x <= 300; x += 50) for (let y = 120; y <= 300; y += 60) inner.push([x, y]);
    const r = await api.cameraHeadAlignSolve({ frames: frames(inner), config: { bed: BED, offset_mm: [2, -1] } });
    expect(r.detected).toBe(inner.length);
    expect(r.fit?.quality).toBe('good');
    expect(r.alignment?.method).toBe('head_sweep');
    expect(r.alignment?.samples?.[0].image_px).toHaveLength(inner.length);
    expect(r.frames.every((f) => f.used && f.error_mm !== null && f.error_mm < 1)).toBe(true);
    // El desfase pedido entra en las posiciones de la cama de la muestra.
    expect(r.alignment?.samples?.[0].bed_mm[0]).toEqual([inner[0][0] + 2, inner[0][1] - 1]);
    expect(r.covered_mm).toEqual([100, 120, 300, 300]);
    await expect(api.cameraHeadAlignSolve({ frames: [], config: { bed: BED } })).rejects.toThrow();
  });

  it('el punto de referencia mueve la alineación para que el píxel caiga donde se pidió', async () => {
    const inner: [number, number][] = [[100, 120], [200, 120], [300, 120], [100, 300], [200, 300], [300, 300]];
    const r = await api.cameraHeadAlignSolve({ frames: frames(inner), config: { bed: BED } });
    const a = r.alignment!;
    const ref_px: [number, number] = [320, 240];
    const at = applyH(a.to_bed, ref_px);
    const out = await api.cameraHeadAlignShift({ alignment: a, lens: null, ref_px, ref_bed: [at[0] + 3, at[1] + 2] });
    expect(out.shift_mm[0]).toBeCloseTo(3, 9);
    expect(out.shift_mm[1]).toBeCloseTo(2, 9);
    const moved = applyH(out.alignment.to_bed, ref_px);
    expect(moved[0]).toBeCloseTo(at[0] + 3, 6);
    expect(moved[1]).toBeCloseTo(at[1] + 2, 6);
    expect(out.alignment.method).toBe('head_sweep');
  });
});
