import { describe, expect, it } from 'vitest';
import type { DeviceProfile } from '@/api/types';
import { FALLBACK_DEVICE } from '@/lib/device';
import { warpActive, warpGrid, warpPoint, type Pt } from './galvoWarp';

type Galvo = DeviceProfile['galvo'];

const galvo = (over: Partial<Galvo> = {}): Galvo => ({ ...structuredClone(FALLBACK_DEVICE.galvo), ...over });
const cylinder = (diameter: number, distance: number, axis: 'x' | 'y' = 'x', concave = false): Galvo =>
  galvo({ cylinder: { enabled: true, diameter_mm: diameter, mirror_distance_mm: distance, axis, concave } });

/** Centro de la cama de las pruebas del motor (50, 50); `rel` devuelve el resultado respecto de él. */
const C: Pt = [50, 50];
const rel = (g: Galvo, p: Pt) => {
  const w = warpPoint(g, C, [C[0] + p[0], C[1] + p[1]])!;
  return { x: w.point[0] - C[0], y: w.point[1] - C[1], visible: w.visible };
};

// Los casos son los mismos, calculados a mano, que `hand_computed_reference_points`, `the_convex_horizon_is_the_tangent_ray`
// y `cone_and_cylinder_combine_with_the_local_radius` de crates/tlaku-pipeline/src/cylinder.rs.
describe('vista previa de la deformación (copia del motor)', () => {
  it('cóncavo, R = 10, D = 100, 90 grados: h = 90, c\' = 100 / 9 y a = 9 pasa a 10', () => {
    const r = rel(cylinder(20, 100, 'x', true), [9, (Math.PI * 10) / 2]);
    expect(r.visible).toBe(true);
    expect(r.x).toBeCloseTo(10, 9);
    expect(r.y).toBeCloseTo(100 / 9, 9);
  });

  it('convexo, R = 30, D = 90, 60 grados: s = 6/7, a = 21 pasa a 18 exacto y c\' = 22,2692246687', () => {
    const r = rel(cylinder(60, 90), [21, (Math.PI * 30) / 3]);
    expect(r.x).toBeCloseTo(18, 9);
    expect(r.y).toBeCloseTo(22.2692246687, 9);
  });

  it('cóncavo, R = 30, D = 90, 60 grados: s = 6/5, a = 15 pasa a 18 y c\' = 31,1769145362', () => {
    const r = rel(cylinder(60, 90, 'x', true), [15, (Math.PI * 30) / 3]);
    expect(r.x).toBeCloseTo(18, 9);
    expect(r.y).toBeCloseTo(31.1769145362, 9);
  });

  it('convexo y cóncavo, R = 30, D = 150, c = 20, a = 10', () => {
    const convex = rel(cylinder(60, 150), [10, 20]);
    expect(convex.x).toBeCloseTo(9.5893592066, 9);
    expect(convex.y).toBeCloseTo(17.7893104924, 9);
    const concave = rel(cylinder(60, 150, 'x', true), [10, 20]);
    expect(concave.x).toBeCloseTo(10.4473835833, 9);
    expect(concave.y).toBeCloseTo(19.3810395871, 9);
    // Simétrico en la curvatura.
    expect(rel(cylinder(60, 150), [10, -20]).y).toBeCloseTo(-17.7893104924, 9);
  });

  it('el centro y su eje no se mueven; con el eje en Y se intercambian los papeles', () => {
    expect(rel(cylinder(60, 160), [30, 0]).y).toBeCloseTo(0, 12);
    expect(rel(cylinder(60, 160), [30, 0]).x).toBeCloseTo(30, 12);
    const a = rel(cylinder(60, 160, 'x'), [13, 21]);
    const b = rel(cylinder(60, 160, 'y'), [21, 13]);
    expect(a.x).toBeCloseTo(b.y, 12);
    expect(a.y).toBeCloseTo(b.x, 12);
  });

  it('el horizonte convexo es el rayo tangente: R sqrt(D / (D + 2R)) y se recorta más allá', () => {
    const g = cylinder(20, 80);
    const edge = 10 * Math.sqrt(80 / 100);
    const thetaT = Math.acos(10 / 90);
    const at = rel(g, [10, 10 * thetaT]);
    expect(at.visible).toBe(true);
    expect(at.y).toBeCloseTo(edge, 9);
    expect(at.x).toBeCloseTo(9, 9);
    for (const c of [15, 25, 1000]) {
      const r = rel(g, [0, c]);
      expect(r.visible).toBe(false);
      expect(r.y).toBeCloseTo(edge, 9);
    }
    // El cóncavo ve la pared hasta los 90 grados.
    expect(rel(cylinder(20, 80, 'x', true), [0, (Math.PI * 10) / 2]).visible).toBe(true);
    expect(rel(cylinder(20, 80, 'x', true), [0, 17]).visible).toBe(false);
  });

  it('cono más cilindro: radio local 15 en el extremo fino y 22,5 en el medio', () => {
    const g = galvo({
      cylinder: { enabled: true, diameter_mm: 60, mirror_distance_mm: 150, axis: 'x', concave: false },
      taper: { enabled: true, top_diameter_mm: 60, bottom_diameter_mm: 30, length_mm: 100, axis: 'x', top_at_positive: true },
    });
    const thin = warpPoint(g, [0, 0], [-50, 15])!;
    expect(thin.point[1]).toBeCloseTo(7.104412679, 8);
    expect(thin.point[0]).toBeCloseTo((-50 * 150) / 151.836261572, 6);
    expect(warpPoint(g, [0, 0], [0, 15])!.point[1]).toBeCloseTo(10.592567374, 8);
    const thick = warpPoint(g, [0, 0], [50, 15])!;
    expect(thick.point[1]).toBeCloseTo(14.039041461, 8);
    expect(thick.point[0]).toBeCloseTo(48.805081398, 8);
  });

  it('el cono solo escala la curvatura con el diámetro local', () => {
    const g = galvo({ taper: { enabled: true, top_diameter_mm: 80, bottom_diameter_mm: 40, length_mm: 100, axis: 'x', top_at_positive: true } });
    expect(warpPoint(g, [0, 0], [50, 10])!.point).toEqual([50, 10]);
    expect(warpPoint(g, [0, 0], [-50, 10])!.point[1]).toBeCloseTo(5, 12);
    expect(warpPoint(g, [0, 0], [0, 10])!.point[1]).toBeCloseTo(7.5, 12);
    const flipped = galvo({ taper: { ...g.taper, top_at_positive: false } });
    expect(warpPoint(flipped, [0, 0], [50, 10])!.point[1]).toBeCloseTo(5, 12);
  });

  it('sin deformación o con parámetros inválidos no hay rejilla', () => {
    expect(warpActive(galvo())).toBe(false);
    expect(warpGrid(galvo(), [100, 100])).toEqual([]);
    expect(warpActive(cylinder(0, 160))).toBe(false);
    expect(warpGrid(cylinder(-5, 160), [100, 100])).toEqual([]);
    const mismatch = galvo({
      cylinder: { enabled: true, diameter_mm: 60, mirror_distance_mm: 150, axis: 'x', concave: false },
      taper: { enabled: true, top_diameter_mm: 60, bottom_diameter_mm: 30, length_mm: 100, axis: 'y', top_at_positive: true },
    });
    expect(warpActive(mismatch)).toBe(false);
  });

  it('la rejilla deformada tiene una línea por cada 10 mm en los dos sentidos y la línea central queda recta', () => {
    const g = cylinder(60, 160);
    const lines = warpGrid(g, [100, 100], 10, 2);
    // La línea del centro (y = 50) no se deforma.
    const centerLine = lines.find((l) => l.points.every((p) => Math.abs(p[1] - 50) < 1e-9) && l.visible);
    expect(centerLine).toBeDefined();
    // Hay tramos fuera del horizonte (arco de hasta 50 mm con R = 30 son 95 grados).
    expect(lines.some((l) => !l.visible)).toBe(true);
  });
});
