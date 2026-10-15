import { describe, expect, it } from 'vitest';
import type { PathDto } from '@/bindings/PathDto';
import schema from './partsSchema.json';
import { mockGenerate, rectMinusRects } from './partsEngine';

const area = (p: PathDto) => p.points.reduce((s, [x, y], i) => {
  const [x2, y2] = p.points[(i + 1) % p.points.length]!;
  return s + (x * y2 - x2 * y) / 2;
}, 0);

describe('simulación de los generadores (la verdad es el motor Rust)', () => {
  it('el formulario viene del mismo JSON que el motor', () => {
    expect((schema as { id: string }[]).map((g) => g.id)).toEqual(['box', 'hinge', 'fit_test', 'joints', 'rounded_box']);
  });

  it('caja de 100×80×60: seis paneles con las medidas pedidas', () => {
    const r = mockGenerate('box', { width_mm: 100, depth_mm: 80, height_mm: 60 }, null, 5);
    expect(r.error).toBeNull();
    const size = Object.fromEntries(r.parts.map((p) => [p.key, [p.width, p.height]]));
    expect(size).toEqual({ front: [100, 60], back: [100, 60], left: [80, 60], right: [80, 60], bottom: [100, 80], top: [100, 80] });
    // Los cortes de cada panel caben en su caja.
    for (const p of r.parts) for (const c of p.cuts) for (const [x, y] of c.points) {
      expect(x).toBeGreaterThanOrEqual(p.x - 1e-9);
      expect(x).toBeLessThanOrEqual(p.x + p.width + 1e-9);
      expect(y).toBeGreaterThanOrEqual(p.y - 1e-9);
      expect(y).toBeLessThanOrEqual(p.y + p.height + 1e-9);
    }
  });

  it('los dientes del frente y de la base son complementarios (mismas celdas)', () => {
    // 100 mm con dientes de 10: 11 celdas; el frente tiene 6 dientes abajo y la base 5.
    const r = mockGenerate('box', {}, null, 5);
    const front = r.parts.find((p) => p.key === 'front')!;
    const bottom = r.parts.find((p) => p.key === 'bottom')!;
    // Vértices de la arista inferior del frente a z = 0 (su y local = 0): inicios de cada diente.
    const toothStarts = (part: typeof front, y: number) => front === part
      ? part.cuts[0]!.points.filter((q) => Math.abs(q[1] - part.y - y) < 1e-9).length / 2
      : part.cuts[0]!.points.filter((q) => Math.abs(q[1] - part.y - y) < 1e-9).length / 2;
    expect(toothStarts(front, 0)).toBe(6);
    // La base tiene el borde que da al frente arriba (v = 80): 5 dientes.
    expect(toothStarts(bottom, 80)).toBe(5);
  });

  it('dimensiones interiores suman dos grosores', () => {
    const r = mockGenerate('box', { width_mm: 90, depth_mm: 70, height_mm: 40, dims: 'inner' }, null, 5);
    expect(r.parts.find((p) => p.key === 'front')!.width).toBe(96);
    expect(r.parts.find((p) => p.key === 'left')!.width).toBe(76);
    expect(r.parts.find((p) => p.key === 'front')!.height).toBe(46);
  });

  it('acomoda en filas dentro del ancho dado y sin tocarse', () => {
    const r = mockGenerate('box', {}, 300, 5);
    expect(r.width).toBeLessThanOrEqual(300);
    for (const [i, a] of r.parts.entries()) for (const b of r.parts.slice(i + 1)) {
      const apart = a.x + a.width + 5 <= b.x + 1e-9 || b.x + b.width + 5 <= a.x + 1e-9 || a.y + a.height + 5 <= b.y + 1e-9 || b.y + b.height + 5 <= a.y + 1e-9;
      expect(apart, `${a.key} y ${b.key}`).toBe(true);
    }
  });

  it('los rótulos solo salen si se piden', () => {
    expect(mockGenerate('box', {}, null, 5).parts.every((p) => p.engraves.length === 0)).toBe(true);
    expect(mockGenerate('box', { labels: true }, null, 5).parts.every((p) => p.engraves.length > 0)).toBe(true);
  });

  it('un valor fuera de rango vuelve como error del campo, no como excepción', () => {
    const r = mockGenerate('box', { thickness_mm: 0 }, null, 5);
    expect(r.error?.field).toBe('thickness_mm');
    expect(r.parts).toEqual([]);
    expect(mockGenerate('box', { width_mm: 8 }, null, 5).error?.field).toBe('width_mm');
    expect(() => mockGenerate('nope', {}, null, 5)).toThrow();
  });

  it('bisagra: cortes dentro del margen y contorno del tamaño pedido', () => {
    for (const pattern of ['straight', 'wave', 'zigzag']) for (const orientation of ['horizontal', 'vertical']) {
      const r = mockGenerate('hinge', { pattern, orientation }, null, 5);
      expect(r.error).toBeNull();
      const part = r.parts[0]!;
      expect([part.width, part.height]).toEqual([100, 50]);
      const slits = part.cuts.filter((c) => !c.closed);
      expect(slits.length).toBeGreaterThan(10);
      for (const s of slits) for (const [x, y] of s.points) {
        expect(x).toBeGreaterThanOrEqual(3 - 1e-9);
        expect(x).toBeLessThanOrEqual(97 + 1e-9);
        expect(y).toBeGreaterThanOrEqual(3 - 1e-9);
        expect(y).toBeLessThanOrEqual(47 + 1e-9);
      }
    }
  });

  it('bisagra: ladrillo y rombo salen como rectas con un aviso de aproximación', () => {
    for (const pattern of ['brick', 'diamond']) {
      const r = mockGenerate('hinge', { pattern }, null, 5);
      expect(r.error).toBeNull();
      expect(r.parts[0]!.cuts.filter((c) => !c.closed).length).toBeGreaterThan(10);
      expect(r.warnings.some((w) => w.includes('aproximada'))).toBe(true);
    }
  });

  it('cola de milano: las áreas de las dos placas suman el rectángulo y la holgura abre la ranura', () => {
    const r = mockGenerate('joints', {}, null, 5);
    expect(r.error).toBeNull();
    const [tails, sockets] = r.parts;
    expect(tails!.height).toBe(60);
    expect(sockets!.height).toBe(60);
    // 40 de placa por lado: la placa de colas mide 40 + profundidad de la cola.
    expect(tails!.width).toBeCloseTo(48, 9);
    const total = area(tails!.cuts[0]!) + area(sockets!.cuts[0]!);
    expect(total).toBeCloseTo(2 * 40 * 60, 6);
    const loose = mockGenerate('joints', { fit_mm: 0.2 }, null, 5);
    expect(area(loose.parts[0]!.cuts[0]!) + area(loose.parts[1]!.cuts[0]!)).toBeLessThan(total - 1e-6);
    expect(mockGenerate('joints', { count: 8, neck_mm: 12 }, null, 5).error?.field).toBe('count');
  });

  it('pestaña-ranura: la ranura es más ancha que la pestaña justo la holgura', () => {
    const r = mockGenerate('joints', { kind: 'tab_slot', fit_mm: 0.1 }, null, 5);
    expect(r.error).toBeNull();
    const holes = r.parts[1]!.cuts.filter((c) => area(c) < 0);
    expect(holes).toHaveLength(3);
    const w = (c: PathDto) => Math.max(...c.points.map((p) => p[0])) - Math.min(...c.points.map((p) => p[0]));
    const h = (c: PathDto) => Math.max(...c.points.map((p) => p[1])) - Math.min(...c.points.map((p) => p[1]));
    expect(w(holes[0]!)).toBeCloseTo(10.1, 9);
    expect(h(holes[0]!)).toBeCloseTo(3.1, 9);
    expect(mockGenerate('joints', { kind: 'tab_slot', tab_mm: 30 }, null, 5).error?.field).toBe('tab_mm');
  });

  it('caja redondeada (aproximada): lateral del largo de la línea media y aviso', () => {
    const r = mockGenerate('rounded_box', {}, null, 5);
    expect(r.error).toBeNull();
    expect(r.parts.map((p) => p.key)).toEqual(['base', 'lid', 'wall']);
    const rm = 15 - 1.5;
    const per = 2 * (97 + 67) - 8 * rm + 2 * Math.PI * rm;
    expect(r.parts[2]!.width).toBeCloseTo(per, 6);
    expect(r.warnings.some((w) => w.includes('aproximada'))).toBe(true);
    expect(mockGenerate('rounded_box', { lid: false }, null, 5).parts.map((p) => p.key)).toEqual(['base', 'wall']);
    expect(mockGenerate('rounded_box', { radius_mm: 4 }, null, 5).error?.field).toBe('radius_mm');
  });

  it('prueba de encaje: ranuras de grosor ± pasos (trece por omisión: ±0,30 mm)', () => {
    expect(mockGenerate('fit_test', {}, null, 5).parts[0]!.cuts.filter((c) => area(c) < 0)).toHaveLength(13);
    const r = mockGenerate('fit_test', { count: 4 }, null, 5);
    const holes = r.parts[0]!.cuts.filter((c) => area(c) < 0);
    expect(holes).toHaveLength(9);
    const widths = holes.map((h) => Math.max(...h.points.map((p) => p[0])) - Math.min(...h.points.map((p) => p[0]))).sort((a, b) => a - b);
    widths.forEach((w, i) => expect(w).toBeCloseTo(3 + (i - 4) * 0.05, 9));
  });

  it('rectMinusRects: una muesca de esquina da un contorno en L', () => {
    const r = rectMinusRects(10, 10, [{ x0: 0, y0: 0, x1: 3, y1: 4 }]);
    expect(r).toHaveLength(1);
    expect(r[0]!.points).toHaveLength(6);
    expect(area(r[0]!)).toBeCloseTo(88, 9);
  });
});
