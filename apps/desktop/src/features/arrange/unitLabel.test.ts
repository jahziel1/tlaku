import { describe, expect, it } from 'vitest';
import type { DesignObject } from '@/api/types';
import { ellipseBez, polylineBez } from '@/lib/bezpath';
import { describeObject, describeUnit, unitLabel, unitThumbnail } from './unitLabel';

const base = { layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null } as const;

function poly(id: string, points: [number, number][], closed: boolean, extra: Partial<DesignObject> = {}): DesignObject {
  return { id, ...base, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [polylineBez(points, closed)] }, ...extra };
}

const rect = (id: string, w: number, h: number, extra: Partial<DesignObject> = {}) => poly(id, [[0, 0], [w, 0], [w, h], [0, h]], true, extra);

describe('nombre legible de cada forma', () => {
  it('rectángulo y cuadrado con su tamaño, con transform incluido', () => {
    expect(unitLabel({ objects: [rect('obj-placa', 40, 20)] })).toBe('Rectángulo 40×20 mm');
    expect(unitLabel({ objects: [rect('x', 20, 20)] })).toBe('Cuadrado 20 mm');
    // El tamaño es el que ocupa en el documento: escalado ×2 en X.
    expect(unitLabel({ objects: [rect('x', 10, 20, { transform: [2, 0, 0, 1, 5, 5] })] })).toBe('Cuadrado 20 mm');
    expect(unitLabel({ objects: [rect('x', 12.5, 5)] })).toBe('Rectángulo 12,5×5 mm');
  });

  it('un rectángulo girado sigue siendo rectángulo', () => {
    const girado = poly('r', [[0, 0], [30, 30], [20, 40], [-10, 10]], true);
    expect(describeObject(girado).kind).toBe('rect');
  });

  it('círculo y elipse', () => {
    const circulo: DesignObject = { id: 'c', ...base, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [ellipseBez(0, 0, 15, 15)] } };
    const elipse: DesignObject = { id: 'e', ...base, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [ellipseBez(0, 0, 20, 10)] } };
    expect(unitLabel({ objects: [circulo] })).toBe('Círculo ⌀ 30 mm');
    expect(unitLabel({ objects: [elipse] })).toBe('Elipse 40×20 mm');
  });

  it('polígonos, formas cerradas y trazos abiertos', () => {
    expect(unitLabel({ objects: [poly('t', [[0, 0], [10, 0], [5, 8]], true)] })).toBe('Polígono de 3 lados (10×8 mm)');
    const muchos = poly('m', Array.from({ length: 20 }, (_, i) => [Math.cos(i) * (5 + (i % 3)), Math.sin(i) * (5 + (i % 3))] as [number, number]), true);
    expect(describeObject(muchos).kind).toBe('closed');
    expect(unitLabel({ objects: [poly('l', [[0, 0], [30, 40]], false)] })).toBe('Línea de 50 mm');
    expect(describeObject(poly('o', [[0, 0], [10, 0], [10, 10]], false)).kind).toBe('open');
  });

  it('varios trazos en un objeto se describen como forma de n trazos', () => {
    const marco: DesignObject = {
      id: 'm',
      ...base,
      transform: [1, 0, 0, 1, 0, 0],
      kind: { type: 'vector', paths: [polylineBez([[0, 0], [20, 0], [20, 20], [0, 20]], true), polylineBez([[5, 5], [15, 5], [15, 15], [5, 15]], true)] },
    };
    expect(unitLabel({ objects: [marco] })).toBe('Forma de 2 trazos (20×20 mm)');
  });

  it('texto con su contenido (primera línea, recortada), imagen y grupo', () => {
    const texto: DesignObject = {
      id: 't',
      ...base,
      transform: [1, 0, 0, 1, 0, 0],
      kind: { type: 'text', text: 'Hola\nmundo', font_family: 'Sans', bold: false, italic: false, size_mm: 10, letter_spacing_mm: 0, line_spacing: 1.2, align: 'left', on_path: null, outline: [] },
    };
    expect(unitLabel({ objects: [texto] })).toBe('Texto «Hola»');
    const largo = { ...texto, kind: { ...texto.kind, text: 'Un texto bastante más largo que el límite' } } as DesignObject;
    expect(unitLabel({ objects: [largo] })).toMatch(/^Texto «.{23}…»$/);
    expect(unitLabel({ objects: [{ ...texto, kind: { ...texto.kind, text: '  ' } } as DesignObject] })).toBe('Texto sin contenido');
    const imagen: DesignObject = { id: 'i', ...base, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'image', source_path: 'x.png', width_px: 10, height_px: 10, size_mm: [30, 20] } };
    expect(unitLabel({ objects: [imagen] })).toBe('Imagen 30×20 mm');
    expect(unitLabel({ objects: [rect('a', 1, 2), rect('b', 3, 4), imagen] })).toBe('Grupo (3)');
    expect(describeUnit({ objects: [] }).kind).toBe('empty');
  });

  it('nunca muestra el id interno', () => {
    for (const o of [rect('obj-placa', 40, 20), poly('obj-linea', [[0, 0], [1, 1]], false)]) {
      expect(unitLabel({ objects: [o] })).not.toContain('obj-');
    }
  });
});

describe('miniatura', () => {
  it('dibuja formas simples con Y hacia abajo y la mayor dimensión escalada', () => {
    const th = unitThumbnail({ objects: [poly('t', [[0, 0], [10, 0], [5, 20]], true)] })!;
    expect(th).not.toBeNull();
    // 20 mm de alto -> 100 de dibujo más 2 × 6 de margen; 10 mm de ancho -> 50 + 12.
    expect(th.height).toBeCloseTo(112);
    expect(th.width).toBeCloseTo(62);
    // El vértice de arriba (y = 20) queda arriba en pantalla (y pequeña).
    expect(th.d).toContain('M6.00 106.00');
    expect(th.d).toContain('L31.00 6.00');
    expect(th.d.endsWith('Z')).toBe(true);
  });

  it('un grupo se dibuja entero y una línea recta tiene caja', () => {
    const g = unitThumbnail({ objects: [rect('a', 10, 10), rect('b', 10, 10, { transform: [1, 0, 0, 1, 30, 0] })] })!;
    expect(g.width).toBeGreaterThan(g.height);
    expect(g.d.split('M')).toHaveLength(3);
    const l = unitThumbnail({ objects: [poly('l', [[0, 0], [10, 0]], false)] })!;
    expect(l.height).toBeGreaterThan(0);
  });

  it('no hay miniatura para imágenes, textos ni formas muy detalladas', () => {
    const imagen: DesignObject = { id: 'i', ...base, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'image', source_path: 'x.png', width_px: 1, height_px: 1, size_mm: [3, 2] } };
    expect(unitThumbnail({ objects: [imagen] })).toBeNull();
    const denso = poly('d', Array.from({ length: 700 }, (_, i) => [i, (i * 7) % 13] as [number, number]), false);
    expect(unitThumbnail({ objects: [denso] })).toBeNull();
    expect(unitThumbnail({ objects: [] })).toBeNull();
    expect(unitThumbnail({ objects: [poly('p', [[1, 1], [1, 1]], false)] })).toBeNull();
  });
});
