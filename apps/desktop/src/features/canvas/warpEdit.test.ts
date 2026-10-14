import { describe, expect, it } from 'vitest';
import type { Point } from '@/api/types';
import type { Bounds } from '@/lib/geometry';
import { makeWarp } from '@/lib/warp';
import { rectObj } from './testUtil';
import {
  boxCorners, envelopeOfSession, guidePaths, handleCount, handlePoints, hitHandle, isIdentity, isValid, movePoint, newSession,
  previewPaths, previewSource, resetSession, setMode, warpOf,
} from './warpEdit';

const box: Bounds = [[0, 0], [30, 30]];
const near = (a: Point, b: Point) => {
  expect(a[0]).toBeCloseTo(b[0], 6);
  expect(a[1]).toBeCloseTo(b[1], 6);
};

describe('mapeo de la malla de control', () => {
  it('cuatro manijas en bilineal y perspectiva, dieciséis en la malla', () => {
    expect(handleCount('bilinear')).toBe(4);
    expect(handleCount('perspective')).toBe(4);
    expect(handleCount('mesh')).toBe(16);
    expect(handlePoints(newSession(box)).length).toBe(4);
    expect(handlePoints(newSession(box, 'mesh')).length).toBe(16);
  });

  it('la malla nueva está sobre la caja: esquinas en su orden y rejilla uniforme', () => {
    const s = newSession(box, 'mesh');
    expect(s.corners).toEqual(boxCorners(box));
    // Índice j * 4 + i: fila de abajo hacia arriba, de izquierda a derecha.
    const pts = handlePoints(s);
    near(pts[0]!, [0, 0]);
    near(pts[3]!, [30, 0]);
    near(pts[5]!, [10, 10]);
    near(pts[12]!, [0, 30]);
    near(pts[15]!, [30, 30]);
    expect(isIdentity(s)).toBe(true);
  });

  it('mover una manija de la malla solo cambia ese punto y deforma el diseño', () => {
    const s = movePoint(newSession(box, 'mesh'), 5, [8, 14]);
    expect(handlePoints(s)[5]).toEqual([8, 14]);
    expect(handlePoints(s)[6]).toEqual([20, 10]);
    expect(isIdentity(s)).toBe(false);
    // La superficie pasa por el punto movido.
    near(warpOf(s)!([10, 10]), [8, 14]);
    near(warpOf(s)!([0, 0]), [0, 0]);
  });

  it('mover una esquina bilineal mueve esa esquina y no las otras', () => {
    const s = movePoint(newSession(box), 2, [40, 35]);
    expect(s.corners[2]).toEqual([40, 35]);
    const warp = warpOf(s)!;
    near(warp([30, 30]), [40, 35]);
    near(warp([0, 0]), [0, 0]);
    near(warp([30, 0]), [30, 0]);
  });

  it('la envolvente que se manda al motor depende del modo', () => {
    const bil = envelopeOfSession(movePoint(newSession(box), 2, [40, 35]));
    expect(bil.grid).toBeUndefined();
    expect(bil.perspective).toBeUndefined();
    expect(bil.top[3]).toEqual([40, 35]);
    const per = envelopeOfSession(movePoint(newSession(box, 'perspective'), 2, [25, 30]));
    expect(per.perspective?.[2]).toEqual([25, 30]);
    const mesh = envelopeOfSession(newSession(box, 'mesh'));
    expect(mesh.grid?.[3]![3]).toEqual([30, 30]);
  });

  it('un índice fuera de rango o un punto no finito no cambian nada', () => {
    const s = newSession(box);
    expect(movePoint(s, 4, [1, 1])).toBe(s);
    expect(movePoint(s, -1, [1, 1])).toBe(s);
    expect(movePoint(s, 0, [Number.NaN, 1])).toBe(s);
  });

  it('la perspectiva rechaza un cuadrilátero cóncavo y conserva el último válido', () => {
    const s = newSession(box, 'perspective');
    const bad = movePoint(s, 2, [3, 3]);
    expect(bad).toBe(s);
    expect(isValid(movePoint(s, 2, [25, 28]))).toBe(true);
  });
});

describe('selección de manija', () => {
  it('elige la más cercana dentro de la tolerancia y -1 si no hay', () => {
    const s = newSession(box, 'mesh');
    expect(hitHandle(s, [10.4, 9.8], 1)).toBe(5);
    expect(hitHandle(s, [29.5, 0.3], 1)).toBe(3);
    expect(hitHandle(s, [15, 15], 1)).toBe(-1);
  });

  it('con dos manijas empatadas gana la primera', () => {
    const s = movePoint(movePoint(newSession(box), 0, [10, 10]), 1, [12, 10]);
    expect(hitHandle(s, [11, 10], 2)).toBe(0);
  });

  it('en bilineal solo hay cuatro manijas aunque el punto esté sobre la malla interior', () => {
    expect(hitHandle(newSession(box), [10, 10], 1)).toBe(-1);
  });
});

describe('cambio de modo', () => {
  it('de esquinas a malla la deformación se conserva', () => {
    const bil = movePoint(newSession(box), 2, [40, 35]);
    const mesh = setMode(bil, 'mesh');
    expect(mesh.mode).toBe('mesh');
    const a = warpOf(bil)!;
    const b = warpOf(mesh)!;
    for (const p of [[0, 0], [30, 30], [10, 20], [30, 15]] as Point[]) near(b(p), a(p));
  });

  it('de malla a esquinas se toman los cuatro vértices de la malla', () => {
    const mesh = movePoint(newSession(box, 'mesh'), 15, [41, 33]);
    const per = setMode(mesh, 'perspective');
    expect(per.corners[2]).toEqual([41, 33]);
    expect(per.corners[0]).toEqual([0, 0]);
  });

  it('el mismo modo devuelve la misma sesión y Restablecer vuelve a la caja', () => {
    const s = movePoint(newSession(box, 'mesh'), 5, [1, 1]);
    expect(setMode(s, 'mesh')).toBe(s);
    const r = resetSession(s);
    expect(r.mode).toBe('mesh');
    expect(isIdentity(r)).toBe(true);
  });
});

describe('vista previa viva', () => {
  const obj = rectObj('r', 5, 5, 25, 25);

  it('la vista previa sigue la misma función que el resultado', () => {
    const s = movePoint(newSession(box, 'mesh'), 5, [8, 14]);
    const src = previewSource([obj], box);
    const warp = warpOf(s)!;
    const paths = previewPaths(src, warp);
    expect(paths).toHaveLength(1);
    const pts = paths[0]!.flatMap((seg) => (seg.type === 'move_to' || seg.type === 'line_to' ? [seg.to] : []));
    // Cada punto es la imagen de uno de la fuente.
    expect(pts).toHaveLength(src.polys[0]!.points.length);
    pts.forEach((p, i) => near(p, warp(src.polys[0]!.points[i]!)));
    // Es determinista.
    expect(previewPaths(src, warp)).toEqual(paths);
  });

  it('una recta se subdivide para poder curvarse', () => {
    const src = previewSource([obj], box);
    expect(src.count).toBeGreaterThan(8);
  });

  it('las líneas guía: contorno en esquinas y 4 + 4 en la malla', () => {
    const warp = makeWarp(box, envelopeOfSession(newSession(box)))!;
    expect(guidePaths(newSession(box), warp)).toHaveLength(4);
    expect(guidePaths(newSession(box, 'mesh'), warp)).toHaveLength(8);
  });
});
