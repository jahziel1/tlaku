import { describe, expect, it } from 'vitest';
import { applyH, type Mat3, type Vec2 } from './geometry';
import { centerFromPoints, circleFromThree, fitCircle } from './center';
import { headPointMm } from './head';
import { toBedAt } from './height';
import type { CameraAlignment, HeadCalibration } from '@/api/types';

const ring = (c: Vec2, r: number, deg: number[]): Vec2[] => deg.map((d) => [c[0] + r * Math.cos((d * Math.PI) / 180), c[1] + r * Math.sin((d * Math.PI) / 180)]);

describe('buscador de centro', () => {
  it('tres puntos del borde dan el centro y el radio exactos', () => {
    const c = circleFromThree(...(ring([123.4, 56.7], 31.25, [10, 140, 260]) as [Vec2, Vec2, Vec2]))!;
    expect(c.center[0]).toBeCloseTo(123.4, 9);
    expect(c.center[1]).toBeCloseTo(56.7, 9);
    expect(c.radius).toBeCloseTo(31.25, 9);
  });

  it('tres puntos alineados, o repetidos, no definen círculo', () => {
    expect(circleFromThree([0, 0], [1, 1], [2, 2])).toBeNull();
    expect(circleFromThree([5, 5], [5, 5], [5, 5])).toBeNull();
    expect(centerFromPoints([[0, 0], [1, 0], [2, 0]])).toEqual({ error: 'collinear' });
    expect(centerFromPoints([[0, 0], [1, 0]])).toEqual({ error: 'tooFew' });
    // Casi alineados: el círculo sería enorme.
    expect(centerFromPoints([[0, 0], [100, 0.001], [200, 0]])).toEqual({ error: 'collinear' });
  });

  it('con muchos puntos ruidosos el ajuste mejora el resultado de tres', () => {
    const noise = (k: number) => {
      const v = Math.sin(k * 91.7) * 43758.5453;
      return (v - Math.floor(v) - 0.5) * 0.3;
    };
    const deg = Array.from({ length: 48 }, (_, i) => i * 7.5);
    const pts = ring([80, 40], 25, deg).map((p, i): Vec2 => [p[0] + noise(2 * i), p[1] + noise(2 * i + 1)]);
    const fit = fitCircle(pts)!;
    expect(Math.hypot(fit.center[0] - 80, fit.center[1] - 40)).toBeLessThan(0.1);
    expect(Math.abs(fit.radius - 25)).toBeLessThan(0.1);
    expect(fit.rms).toBeLessThan(0.2);
    // Tres puntos con el mismo ruido: peor.
    const three = circleFromThree(pts[0]!, pts[16]!, pts[32]!)!;
    expect(Math.hypot(three.center[0] - 80, three.center[1] - 40)).toBeGreaterThan(Math.hypot(fit.center[0] - 80, fit.center[1] - 40) * 0.5);
  });

  it('con la cámara fija: el círculo visto en perspectiva se ajusta en mm, no en píxeles', () => {
    // Cama 400x300 mm vista en 1280x720 con perspectiva fuerte (trapecio).
    const bedToImage: Mat3 = [[2.6, 0.35, 120], [0.1, -2.1, 650], [0.00045, -0.0006, 1]];
    const inv = invert(bedToImage);
    const alignment = { image_size: [1280, 720], to_bed: inv, height: null } as unknown as CameraAlignment;
    const toBed = toBedAt(alignment, 0);
    // Disco real de 40 mm de radio centrado en (310, 200) mm.
    const truthCenter: Vec2 = [310, 200];
    const edgePx = ring(truthCenter, 40, [20, 150, 280]).map((p) => applyH(bedToImage, p));
    const mm = edgePx.map((p) => applyH(toBed, p));
    const r = centerFromPoints(mm);
    if ('error' in r) throw new Error(r.error);
    expect(Math.hypot(r.circle.center[0] - 310, r.circle.center[1] - 200)).toBeLessThan(1e-6);
    expect(r.circle.radius).toBeCloseTo(40, 6);
    // El centro del círculo en la imagen NO es la imagen del centro (por eso se ajusta en mm).
    const pxCircle = circleFromThree(...(edgePx as [Vec2, Vec2, Vec2]))!;
    const wrong = applyH(toBed, pxCircle.center);
    expect(Math.hypot(wrong[0] - 310, wrong[1] - 200)).toBeGreaterThan(1);
  });

  it('con la cámara del cabezal: el centro sale igual con el láser en otra posición', () => {
    const cal: HeadCalibration = { image_size: [1280, 720], ref_px: [640, 360], px_to_mm: [[0.125, 0], [0, -0.125]], offset_mm: [20, -35], rms_mm: 0, observations: 4 };
    const truth: Vec2 = [210, 150];
    const px = (laser: Vec2, mm: Vec2): Vec2 => [640 + (mm[0] - laser[0] - 20) / 0.125, 360 - (mm[1] - laser[1] + 35) / 0.125];
    // Tres clics hechos con el cabezal en tres posiciones distintas (cada uno se pasa a mm al momento).
    const picks: [Vec2, Vec2][] = [
      [[160, 190], ring(truth, 18, [0])[0]!],
      [[190, 195], ring(truth, 18, [120])[0]!],
      [[200, 170], ring(truth, 18, [240])[0]!],
    ];
    const mm = picks.map(([laser, edge]) => headPointMm(cal, laser, px(laser, edge)));
    const r = centerFromPoints(mm);
    if ('error' in r) throw new Error(r.error);
    expect(r.circle.center[0]).toBeCloseTo(210, 6);
    expect(r.circle.center[1]).toBeCloseTo(150, 6);
  });
});

function invert(m: Mat3): Mat3 {
  const [[a, b, c], [d, e, f], [g, h, i]] = m;
  const A = e * i - f * h;
  const B = -(d * i - f * g);
  const C = d * h - e * g;
  const det = a * A + b * B + c * C;
  const r: Mat3 = [
    [A / det, -(b * i - c * h) / det, (b * f - c * e) / det],
    [B / det, (a * i - c * g) / det, -(a * f - c * d) / det],
    [C / det, -(a * h - b * g) / det, (a * e - b * d) / det],
  ];
  const s = r[2][2];
  return r.map((row) => row.map((v) => v / s)) as Mat3;
}
