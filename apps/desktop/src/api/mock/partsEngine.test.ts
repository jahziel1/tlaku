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
    expect((schema as { id: string }[]).map((g) => g.id)).toEqual(['box', 'tray', 'hinge', 'fit_test']);
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

  it('bandeja: cinco paneles; cajón: más un frente mayor; con caja: cinco piezas más', () => {
    const tray = mockGenerate('tray', {}, null, 5);
    expect(tray.error).toBeNull();
    expect(tray.parts.map((p) => p.key)).toEqual(['front', 'back', 'left', 'right', 'bottom']);
    expect(tray.parts[0]!.height).toBe(30);
    const drawer = mockGenerate('tray', { kind: 'drawer', front_overlap_mm: 8 }, null, 5);
    const plate = drawer.parts.find((p) => p.key === 'drawer_front')!;
    expect([plate.width, plate.height]).toEqual([136, 46]);
    const cased = mockGenerate('tray', { kind: 'drawer_case', drawer_clearance_mm: 1 }, null, 5);
    expect(cased.parts.map((p) => p.key)).toEqual(expect.arrayContaining(['case_back', 'case_top', 'case_bottom', 'case_left', 'case_right']));
    // Interior de la caja = cajón + 2 de juego; exterior suma el grosor.
    expect(cased.parts.find((p) => p.key === 'case_back')!.width).toBeCloseTo(120 + 2 + 6, 9);
  });

  it('los divisores avisan que el modo simulado no los dibuja', () => {
    const r = mockGenerate('box', { dividers_x: 2 }, null, 5);
    expect(r.error).toBeNull();
    expect(r.warnings.some((w) => w.includes('divisores'))).toBe(true);
    const bad = mockGenerate('box', { dividers_x: 11 }, null, 5);
    expect(bad.error?.field).toBe('dividers_x');
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
