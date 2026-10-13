import { describe, expect, it } from 'vitest';
import {
  FRAGMENT_SHADER,
  correctedSize,
  correctedToRaw,
  distortNormalized,
  insideRaw,
  lensFitsStream,
  lensUniforms,
  correctedFocal,
  isFisheye,
  viewZoom,
  type LensParams,
} from './lens';

// Valores calculados con LensCalibration::distort_normalized de tlaku-camera (Rust).
const CAL: LensParams = {
  width: 1280, height: 720, fx: 950, fy: 960, cx: 645, cy: 355,
  k1: -0.32, k2: 0.11, p1: 0.0012, p2: -0.0008, k3: -0.02,
};

describe('modelo de Brown-Conrady (igual que tlaku-camera)', () => {
  it('distorsiona coordenadas normalizadas', () => {
    const a = distortNormalized(CAL, 0.3, -0.2);
    expect(a[0]).toBeCloseTo(0.287672518, 8);
    expect(a[1]).toBeCloseTo(-0.191695012, 8);
    const b = distortNormalized(CAL, -0.6, 0.35);
    expect(b[0]).toBeCloseTo(-0.5228432643125, 8);
    expect(b[1]).toBeCloseTo(0.305345737515625, 8);
  });

  it('del píxel corregido al crudo (el que muestrea el shader)', () => {
    const cases: [number, number, number, number][] = [
      [0, 0, 100.19279197687217, 56.08345926111997],
      [640, 360, 639.99994144824, 360.0000800441664],
      [1279, 719, 1180.5393574854018, 663.406313634374],
      [100, 600, 158.97256438707728, 573.8088734461359],
    ];
    for (const [u, v, x, y] of cases) {
      const [rx, ry] = correctedToRaw(CAL, u, v);
      expect(rx).toBeCloseTo(x, 6);
      expect(ry).toBeCloseTo(y, 6);
    }
  });

  it('sin distorsión es la identidad', () => {
    const flat = { ...CAL, k1: 0, k2: 0, k3: 0, p1: 0, p2: 0 };
    const [x, y] = correctedToRaw(flat, 321.5, 77.25);
    expect(x).toBeCloseTo(321.5, 9);
    expect(y).toBeCloseTo(77.25, 9);
  });

  it('con barril (k1 < 0) la esquina corregida sale de la imagen cruda y queda transparente', () => {
    const [x, y] = correctedToRaw({ ...CAL, k1: 0.4 }, 0, 0);
    expect(insideRaw(CAL, x, y)).toBe(false);
    expect(insideRaw(CAL, 100, 50)).toBe(true);
    expect(insideRaw(CAL, 1280, 50)).toBe(false);
  });
});

describe('parámetros del shader', () => {
  it('empaqueta uniformes en píxeles de la calibración', () => {
    expect(lensUniforms(CAL)).toEqual({
      size: [1280, 720],
      k: [950, 960, 645, 355],
      rad: [-0.32, 0.11, -0.02, 0],
      tan: [0.0012, -0.0008],
      zoom: 1,
      fisheye: 0,
    });
  });

  it('el shader usa el mismo orden de coeficientes que la referencia', () => {
    expect(FRAGMENT_SHADER).toContain('uRad.x + r2 * (uRad.y + r2 * uRad.z)');
    expect(FRAGMENT_SHADER).toContain('2.0 * uTan.x * n.x * n.y + uTan.y * (r2 + 2.0 * n.x * n.x)');
    expect(FRAGMENT_SHADER).toContain('uTan.x * (r2 + 2.0 * n.y * n.y) + 2.0 * uTan.y * n.x * n.y');
  });

  it('acepta un video con la misma proporción y rechaza otra', () => {
    expect(lensFitsStream(CAL, 1280, 720)).toBe(true);
    expect(lensFitsStream(CAL, 640, 360)).toBe(true);
    expect(lensFitsStream(CAL, 640, 480)).toBe(false);
    expect(lensFitsStream(CAL, 0, 0)).toBe(false);
  });

  it('limita el tamaño de la imagen corregida', () => {
    expect(correctedSize(CAL)).toEqual([1280, 720]);
    expect(correctedSize({ ...CAL, width: 1920, height: 1080 })).toEqual([1280, 720]);
    expect(correctedSize({ ...CAL, width: 640, height: 480 })).toEqual([640, 480]);
  });
});

// Ojo de pez de Kannala-Brandt. Referencia calculada aparte, con la fórmula
// θd = θ (1 + k1 θ² + k2 θ⁴ + k3 θ⁶ + k4 θ⁸) y (xd, yd) = (x, y) · θd / r, r = hypot(x, y), θ = atan(r).
const FISH: LensParams = {
  width: 640, height: 480, fx: 330, fy: 331, cx: 321, cy: 241,
  k1: -0.04, k2: 0.012, p1: 0, p2: 0, k3: 0.003, k4: -0.001,
  model: 'fisheye', view_zoom: 0.62,
};

describe('ojo de pez de Kannala-Brandt (igual que tlaku-camera)', () => {
  it('distorsiona coordenadas normalizadas, también las de ángulos grandes', () => {
    const cases: [number, number, number, number][] = [
      [0.3, -0.2, 0.28660026068080313, -0.1910668404538688],
      [-1.2, 0.9, -0.7661016856319043, 0.5745762642239283],
      [2.0, 1.5, 0.9254918007743138, 0.6941188505807354],
    ];
    for (const [x, y, ex, ey] of cases) {
      const [dx, dy] = distortNormalized(FISH, x, y);
      expect(dx).toBeCloseTo(ex, 9);
      expect(dy).toBeCloseTo(ey, 9);
    }
    expect(distortNormalized(FISH, 0, 0)).toEqual([0, 0]);
  });

  it('del píxel corregido al crudo usa la focal reducida por view_zoom', () => {
    const cases: [number, number, number, number][] = [
      [0, 0, 38.692967527666724, 29.049860355662503],
      [320, 240, 319.3871254554245, 239.38712545542447],
      [639, 479, 602.5402224914698, 451.71249356279816],
      [100, 400, 78.72362764748107, 415.30743531244576],
    ];
    for (const [u, v, x, y] of cases) {
      const [rx, ry] = correctedToRaw(FISH, u, v);
      expect(rx).toBeCloseTo(x, 6);
      expect(ry).toBeCloseTo(y, 6);
    }
  });

  it('una calibración guardada sin modelo es Brown-Conrady con zoom 1', () => {
    expect(isFisheye(CAL)).toBe(false);
    expect(viewZoom(CAL)).toBe(1);
    expect(correctedFocal(CAL)).toEqual([950, 960]);
    expect(correctedFocal(FISH)).toEqual([330 * 0.62, 331 * 0.62]);
    expect(viewZoom({ ...CAL, view_zoom: 0 })).toBe(1);
    expect(viewZoom({ ...CAL, view_zoom: NaN })).toBe(1);
    // Con model "brown_conrady" explícito, mismos números que sin él.
    expect(correctedToRaw({ ...CAL, model: 'brown_conrady', k4: 0.5 }, 100, 600)).toEqual(correctedToRaw(CAL, 100, 600));
  });

  it('empaqueta los uniformes del ojo de pez y el shader tiene la misma fórmula', () => {
    const u = lensUniforms(FISH);
    expect(u.fisheye).toBe(1);
    expect(u.zoom).toBe(0.62);
    expect(u.rad).toEqual([-0.04, 0.012, 0.003, -0.001]);
    expect(FRAGMENT_SHADER).toContain('uRad.x + t2 * (uRad.y + t2 * (uRad.z + t2 * uRad.w))');
    expect(FRAGMENT_SHADER).toContain('float th = atan(r);');
    expect(FRAGMENT_SHADER).toContain('(uK.xy * uZoom)');
  });
});
