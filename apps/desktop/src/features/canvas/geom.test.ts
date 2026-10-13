import { describe, expect, it } from 'vitest';
import type { DesignObject, TextObject } from '@/api/types';
import { ellipseBez, polylineBez, roundedRectBez } from '@/lib/bezpath';
import { buildGeom, chunkPath, fillPath, OP_CUBIC, OP_CLOSE } from './geom';
import { hitObject, pickObject, selectByBox } from './hit';
import { ctxFor } from './testUtil';
import { newVectorObject } from './shapes';

/** Path2D de mentira: anota las llamadas para comprobar que se usan curvas y no segmentos. */
class FakePath {
  calls: [string, ...number[]][] = [];
  moveTo(...a: number[]) { this.calls.push(['moveTo', ...a]); }
  lineTo(...a: number[]) { this.calls.push(['lineTo', ...a]); }
  quadraticCurveTo(...a: number[]) { this.calls.push(['quadraticCurveTo', ...a]); }
  bezierCurveTo(...a: number[]) { this.calls.push(['bezierCurveTo', ...a]); }
  closePath() { this.calls.push(['closePath']); }
}
const make = () => new FakePath() as unknown as Path2D;
const callsOf = (p: Path2D) => (p as unknown as FakePath).calls;

const circle = (r: number, transform: DesignObject['transform'] = [1, 0, 0, 1, 0, 0]): DesignObject => ({
  ...newVectorObject('c', '#ff0000', [ellipseBez(0, 0, r, r)]),
  transform,
});

describe('geometría con curvas', () => {
  it('el círculo se dibuja con cuatro bezierCurveTo y se cierra', () => {
    const g = buildGeom(circle(10));
    expect(g.chunks).toHaveLength(1);
    const calls = callsOf(chunkPath(g, g.chunks[0], make));
    expect(calls.filter((c) => c[0] === 'bezierCurveTo')).toHaveLength(4);
    expect(calls.some((c) => c[0] === 'lineTo')).toBe(false);
    expect(calls[0]).toEqual(['moveTo', 10, 0]);
    expect(calls.at(-1)).toEqual(['closePath']);
    expect(g.polys[0].ops[0]).toBe(OP_CUBIC);
    expect(g.polys[0].ops.at(-1)).toBe(OP_CLOSE);
  });

  it('las curvas siguen el transform: los controles se transforman, no se aplanan', () => {
    const g = buildGeom(circle(1, [10, 0, 0, 10, 50, 0]));
    const calls = callsOf(chunkPath(g, g.chunks[0], make));
    expect(calls[0]).toEqual(['moveTo', 60, 0]);
    const first = calls[1];
    expect(first[0]).toBe('bezierCurveTo');
    expect(first[1]).toBeCloseTo(60, 9); // ctrl1.x = 50 + 10·1
    expect(first[2]).toBeCloseTo(5.522847498, 6); // ctrl1.y = 10·κ
  });

  it('la caja es exacta y el aplanado sirve para hit-test', () => {
    const g = buildGeom(circle(10, [1, 0, 0, 1, 100, 100]));
    expect(g.box).toEqual([90, 90, 110, 110]);
    const o = circle(10, [1, 0, 0, 1, 100, 100]);
    expect(hitObject(g, 110, 100, 0.2, false)).toBe(true); // sobre el trazo
    expect(hitObject(g, 100, 100, 0.2, false)).toBe(false); // centro: sin relleno no cuenta
    expect(hitObject(g, 100, 100, 0.2, true)).toBe(true);
    expect(pickObject(ctxFor([o]), [100 + 10 * Math.SQRT1_2, 100 + 10 * Math.SQRT1_2], 0.05)?.id).toBe('c');
    expect(selectByBox(ctxFor([o]), [85, 85, 115, 115], 'window')).toEqual(['c']);
    expect(selectByBox(ctxFor([o]), [95, 95, 105, 105], 'window')).toEqual([]);
  });

  it('un rectángulo redondeado mezcla rectas y esquinas cúbicas', () => {
    const o = newVectorObject('r', '#ff0000', [roundedRectBez(0, 0, 40, 20, 5)]);
    const g = buildGeom(o);
    const calls = callsOf(fillPath(g, make));
    expect(calls.filter((c) => c[0] === 'bezierCurveTo')).toHaveLength(4);
    expect(calls.filter((c) => c[0] === 'lineTo')).toHaveLength(4);
    expect(g.box).toEqual([0, 0, 40, 20]);
  });

  it('una cuadrática usa quadraticCurveTo', () => {
    const o = newVectorObject('q', '#ff0000', [[{ type: 'move_to', to: [0, 0] }, { type: 'quad_to', ctrl: [5, 10], to: [10, 0] }]]);
    const g = buildGeom(o);
    expect(callsOf(chunkPath(g, g.chunks[0], make))[1]).toEqual(['quadraticCurveTo', 5, 10, 10, 0]);
    expect(g.box![3]).toBeCloseTo(5, 9);
  });

  it('varios subtrayectos y trazos tras close generan contornos separados', () => {
    const path = [
      ...polylineBez([[0, 0], [10, 0], [10, 10]], true),
      { type: 'line_to' as const, to: [20, 20] as [number, number] }, // continúa desde (0, 0) tras el close
      ...polylineBez([[50, 50], [60, 50]], false),
    ];
    const g = buildGeom(newVectorObject('m', '#ff0000', [path]));
    expect(g.polys).toHaveLength(3);
    expect(g.polys[0].closed).toBe(true);
    expect(g.polys[1].closed).toBe(false);
    expect(Array.from(g.polys[1].co.slice(0, 2))).toEqual([0, 0]);
  });

  it('un círculo grande y uno chico se aplanan con la misma tolerancia', () => {
    const small = buildGeom(circle(1));
    const big = buildGeom(circle(200));
    expect(big.polys[0].pts.length).toBeGreaterThan(small.polys[0].pts.length);
    for (let i = 0; i < big.polys[0].pts.length; i += 2) {
      expect(Math.abs(Math.hypot(big.polys[0].pts[i], big.polys[0].pts[i + 1]) - 200)).toBeLessThan(0.01 + 0.0003 * 200);
    }
  });
});

describe('texto', () => {
  const text = (outline: TextObject['outline']): DesignObject => ({
    id: 't', layer: '#ff0000', transform: [1, 0, 0, 1, 10, 20], locked: false, hidden: false, group: null,
    kind: {
      type: 'text', text: 'Hola\nmundo', font_family: 'Arial', bold: false, italic: false, size_mm: 10,
      letter_spacing_mm: 0, line_spacing: 1.2, align: 'left', on_path: null, outline,
    },
  });

  it('con contorno se dibuja el contorno', () => {
    const g = buildGeom(text([polylineBez([[0, 0], [4, 0], [4, 6]], true)]));
    expect(g.placeholder).toBeNull();
    expect(g.box).toEqual([10, 20, 14, 26]);
  });

  it('sin contorno hay un marcador con caja estimada, seleccionable', () => {
    const o = text([]);
    const g = buildGeom(o);
    expect(g.placeholder).not.toBeNull();
    expect(g.box![0]).toBeCloseTo(10, 9);
    expect(g.box![2]).toBeGreaterThan(10 + 10); // 5 caracteres más largos que 2 em
    expect(g.box![1]).toBeLessThan(20 - 10 * 1.2); // la segunda línea baja
    expect(selectByBox(ctxFor([o]), [0, 0, 300, 300], 'window')).toEqual(['t']);
  });
});
