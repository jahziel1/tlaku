import { describe, expect, it } from 'vitest';
import type { Point } from '@/api/types';
import { worldToScreen } from '@/lib/viewport';
import type { CameraBackdrop } from '@/store/camera';
import { affineBetween, backdropMesh, homography, meshDivisions, paintBackdrop, SEAM_PX, type Quad } from './backdrop';

const close = (a: Point, b: Point, eps = 1e-9) => Math.hypot(a[0] - b[0], a[1] - b[1]) < eps;
const apply = (m: number[], [x, y]: Point): Point => [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];

/** Cama vista en perspectiva: el lado de arriba es más corto que el de abajo. */
const PERSPECTIVE: Quad = [[30, 380], [370, 360], [400, 20], [0, 0]];
/** Paralelogramo (inclinado, pero sin perspectiva). */
const PARALLELOGRAM: Quad = [[10, 300], [310, 310], [330, 10], [30, 0]];
const W = 640;
const H = 480;

describe('homografía del fondo de la cámara', () => {
  it('las cuatro esquinas de la imagen caen en las esquinas dadas', () => {
    for (const q of [PERSPECTIVE, PARALLELOGRAM]) {
      const { map } = homography(q);
      expect(close(map(0, 0), q[0])).toBe(true);
      expect(close(map(1, 0), q[1])).toBe(true);
      expect(close(map(1, 1), q[2])).toBe(true);
      expect(close(map(0, 1), q[3])).toBe(true);
    }
  });

  it('el centro de la imagen cae en la intersección de las diagonales (propiedad proyectiva)', () => {
    const q = PERSPECTIVE;
    const c = homography(q).map(0.5, 0.5);
    // intersección de q0-q2 con q1-q3
    const [p, r, s, u] = [q[0], q[2], q[1], q[3]];
    const d1: Point = [r[0] - p[0], r[1] - p[1]];
    const d2: Point = [u[0] - s[0], u[1] - s[1]];
    const den = d1[0] * d2[1] - d1[1] * d2[0];
    const t = ((s[0] - p[0]) * d2[1] - (s[1] - p[1]) * d2[0]) / den;
    expect(close(c, [p[0] + d1[0] * t, p[1] + d1[1] * t], 1e-7)).toBe(true);
    // y con perspectiva no es el centro del cuadrilátero
    expect(close(c, [(q[0][0] + q[2][0]) / 2, (q[0][1] + q[2][1]) / 2], 1e-3)).toBe(false);
  });

  it('un paralelogramo es afín: dos triángulos bastan', () => {
    expect(homography(PARALLELOGRAM).affine).toBe(true);
    expect(meshDivisions(PARALLELOGRAM)).toBe(1);
    expect(backdropMesh(PARALLELOGRAM, W, H)).toHaveLength(2);
    expect(homography(PERSPECTIVE).affine).toBe(false);
    expect(meshDivisions(PERSPECTIVE)).toBeGreaterThan(4);
  });
});

describe('malla de triángulos', () => {
  it('cubre la imagen entera: las esquinas de la malla son las de la imagen y caen en las esquinas del fondo', () => {
    const mesh = backdropMesh(PERSPECTIVE, W, H, 8);
    expect(mesh).toHaveLength(8 * 8 * 2);
    const find = (src: Point) => {
      for (const t of mesh) for (let i = 0; i < 3; i++) if (close(t.src[i], src)) return t.dst[i];
      return null;
    };
    expect(close(find([0, 0])!, PERSPECTIVE[0])).toBe(true);
    expect(close(find([W, 0])!, PERSPECTIVE[1])).toBe(true);
    expect(close(find([W, H])!, PERSPECTIVE[2])).toBe(true);
    expect(close(find([0, H])!, PERSPECTIVE[3])).toBe(true);
    // el área en la imagen suma todo W x H (sin huecos ni traslapes)
    const area = mesh.reduce((s, t) => {
      const [a, b, c] = t.src;
      return s + Math.abs((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])) / 2;
    }, 0);
    expect(area).toBeCloseTo(W * H, 6);
  });

  it('cada vértice de la malla está sobre la homografía', () => {
    const { map } = homography(PERSPECTIVE);
    for (const t of backdropMesh(PERSPECTIVE, W, H, 6)) {
      for (let i = 0; i < 3; i++) expect(close(t.dst[i], map(t.src[i][0] / W, t.src[i][1] / H), 1e-7)).toBe(true);
    }
  });

  it('la afín de cada triángulo lleva sus tres vértices a su sitio y el borde común coincide con el vecino', () => {
    const mesh = backdropMesh(PERSPECTIVE, W, H, 6);
    for (const t of mesh) {
      const m = affineBetween(t.src, t.dst)!;
      for (let i = 0; i < 3; i++) expect(close(apply(m, t.src[i]), t.dst[i], 1e-6)).toBe(true);
    }
    // El punto medio de una arista compartida cae igual con la afín de cualquiera de los dos triángulos.
    const [t1, t2] = [mesh[0], mesh[1]]; // comparten la diagonal a-c
    const m1 = affineBetween(t1.src, t1.dst)!;
    const m2 = affineBetween(t2.src, t2.dst)!;
    const mid: Point = [(t1.src[0][0] + t1.src[2][0]) / 2, (t1.src[0][1] + t1.src[2][1]) / 2];
    expect(close(apply(m1, mid), apply(m2, mid), 1e-6)).toBe(true);
  });

  it('con pocas divisiones el punto central cae cerca de la homografía; con más, más cerca (error por afín)', () => {
    const { map } = homography(PERSPECTIVE);
    const err = (n: number) => {
      // punto del interior de la imagen que no es vértice
      const src: Point = [W * 0.37, H * 0.61];
      const tri = backdropMesh(PERSPECTIVE, W, H, n).find((t) => inside(src, t.src))!;
      const p = apply(affineBetween(tri.src, tri.dst)!, src);
      const e = map(src[0] / W, src[1] / H);
      return Math.hypot(p[0] - e[0], p[1] - e[1]);
    };
    expect(err(16)).toBeLessThan(err(2));
    expect(err(16)).toBeLessThan(0.5); // menos de medio mm sobre una cama de 400 mm
  });

  it('un triángulo degenerado no tiene afín', () => {
    expect(affineBetween([[0, 0], [1, 1], [2, 2]], [[0, 0], [1, 0], [0, 1]])).toBeNull();
  });
});

function inside(p: Point, [a, b, c]: [Point, Point, Point]): boolean {
  const s = (u: Point, v: Point, w: Point) => (u[0] - w[0]) * (v[1] - w[1]) - (v[0] - w[0]) * (u[1] - w[1]);
  const d1 = s(p, a, b);
  const d2 = s(p, b, c);
  const d3 = s(p, c, a);
  return !((d1 < 0 || d2 < 0 || d3 < 0) && (d1 > 0 || d2 > 0 || d3 > 0));
}

// ---- dibujo con un contexto simulado ------------------------------------------------------------------------------

interface Call {
  clip: Point[];
  transform: number[];
  src: number[];
}

/** Contexto que solo anota cada triángulo dibujado. */
function fakeContext(): { ctx: CanvasRenderingContext2D; calls: Call[] } {
  const calls: Call[] = [];
  let path: Point[] = [];
  let clip: Point[] = [];
  let base = [1, 0, 0, 1, 0, 0];
  let cur = [1, 0, 0, 1, 0, 0];
  const stack: { clip: Point[]; cur: number[] }[] = [];
  const mul = (o: number[], i: number[]) => [
    o[0] * i[0] + o[2] * i[1], o[1] * i[0] + o[3] * i[1], o[0] * i[2] + o[2] * i[3], o[1] * i[2] + o[3] * i[3],
    o[0] * i[4] + o[2] * i[5] + o[4], o[1] * i[4] + o[3] * i[5] + o[5],
  ];
  const ctx = {
    canvas: { width: 100, height: 100 },
    save: () => stack.push({ clip, cur }),
    restore: () => {
      const s = stack.pop()!;
      clip = s.clip;
      cur = s.cur;
    },
    setTransform: (a: number, b: number, c: number, d: number, e: number, f: number) => {
      base = [a, b, c, d, e, f];
      cur = base;
    },
    transform: (a: number, b: number, c: number, d: number, e: number, f: number) => {
      cur = mul(cur, [a, b, c, d, e, f]);
    },
    clearRect: () => {},
    beginPath: () => {
      path = [];
    },
    moveTo: (x: number, y: number) => path.push([x, y]),
    lineTo: (x: number, y: number) => path.push([x, y]),
    closePath: () => {},
    clip: () => {
      clip = path;
    },
    drawImage: (_s: unknown, sx: number, sy: number, sw: number, sh: number) => calls.push({ clip, transform: cur, src: [sx, sy, sw, sh] }),
  } as unknown as CanvasRenderingContext2D;
  return { ctx, calls };
}

const backdrop = (corners: Quad): CameraBackdrop => ({ source: {} as CanvasImageSource, width: W, height: H, corners, opacity: 0.6, frame: 1 });

describe('paintBackdrop', () => {
  const view = { zoom: 2, panX: 20, panY: 900 };

  it('dibuja cada triángulo con su afín: las esquinas de la imagen caen en pantalla donde la homografía manda', () => {
    const { ctx, calls } = fakeContext();
    paintBackdrop(ctx, backdrop(PERSPECTIVE), view, 1200, 1000, 1);
    expect(calls.length).toBeGreaterThan(100);
    // la afín acumulada (px de imagen -> px de pantalla) lleva la esquina (0, 0) de la imagen a la esquina 0 del fondo
    const corner = (src: Point): Point | null => {
      for (const c of calls) {
        const [sx, sy] = c.src;
        if (sx === 0 && sy === 0 && src[0] === 0 && src[1] === 0) return apply(c.transform, src);
      }
      return null;
    };
    const p = corner([0, 0])!;
    expect(close(p, worldToScreen(view, PERSPECTIVE[0]), 1e-6)).toBe(true);
  });

  it('cada triángulo se recorta un poco más grande que el real para que no haya costuras', () => {
    const { ctx, calls } = fakeContext();
    paintBackdrop(ctx, backdrop(PARALLELOGRAM), view, 1200, 1000, 1);
    const mesh = backdropMesh(PARALLELOGRAM, W, H);
    expect(calls).toHaveLength(2);
    const real = mesh[0].dst.map((p) => worldToScreen(view, p));
    const clip = calls[0].clip;
    const area = (t: Point[]) => Math.abs((t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1])) / 2;
    expect(area(clip)).toBeGreaterThan(area(real));
    expect(SEAM_PX).toBeGreaterThan(0);
  });

  it('omite los triángulos que quedan fuera de la vista', () => {
    const { ctx, calls } = fakeContext();
    paintBackdrop(ctx, backdrop(PERSPECTIVE), { zoom: 1, panX: 5000, panY: 5000 }, 1200, 1000, 1);
    expect(calls).toHaveLength(0);
    const all = fakeContext();
    paintBackdrop(all.ctx, backdrop(PERSPECTIVE), view, 1200, 1000, 1);
    const near = fakeContext();
    paintBackdrop(near.ctx, backdrop(PERSPECTIVE), { zoom: 8, panX: -1000, panY: 3300 }, 400, 300, 1);
    expect(near.calls.length).toBeLessThan(all.calls.length);
  });

  it('una imagen vacía no dibuja nada', () => {
    const { ctx, calls } = fakeContext();
    paintBackdrop(ctx, { ...backdrop(PERSPECTIVE), width: 0 }, view, 1200, 1000, 1);
    expect(calls).toHaveLength(0);
  });
});
