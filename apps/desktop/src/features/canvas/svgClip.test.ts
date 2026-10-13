import { describe, expect, it } from 'vitest';
import { makeLayer } from '@/lib/layers';
import { objectBounds } from '@/lib/geometry';
import { looksLikeSvg, objectsToSvg, parseColor, parsePathData, parseTransform, svgToObjects } from './svgClip';
import { lineObj, rectObj } from './testUtil';

const size = (o: ReturnType<typeof rectObj>) => {
  const b = objectBounds(o)!;
  return [b[1][0] - b[0][0], b[1][1] - b[0][1]];
};

describe('escribir SVG', () => {
  it('mide en mm, con Y hacia abajo y la esquina del conjunto en el origen', () => {
    const svg = objectsToSvg([rectObj('a', 10, 20, 30, 25)], [makeLayer('#ff0000', 'Rojo')])!;
    expect(svg).toContain('width="20mm"');
    expect(svg).toContain('height="5mm"');
    expect(svg).toContain('viewBox="0 0 20 5"');
    expect(svg).toContain('stroke="#ff0000"');
    expect(svg).toContain('fill="none"');
    expect(svg).toContain('M0 5L20 5L20 0L0 0Z'); // Y se invierte
  });

  it('las capas de relleno salen rellenas', () => {
    const svg = objectsToSvg([rectObj('a', 0, 0, 5, 5)], [makeLayer('#ff0000', 'Rojo', { mode: 'fill' })])!;
    expect(svg).toContain('fill="#ff0000"');
    expect(svg).toContain('evenodd');
  });

  it('omite lo oculto, las imágenes y los textos sin contorno; sin nada, null', () => {
    expect(objectsToSvg([rectObj('h', 0, 0, 1, 1, { hidden: true })], [])).toBeNull();
    expect(objectsToSvg([], [])).toBeNull();
  });

  it('lo que se escribe se vuelve a leer con las mismas medidas y capas', () => {
    const svg = objectsToSvg([rectObj('a', 10, 20, 30, 25), lineObj('b', [0, 0], [50, 50], { layer: '#00ff00' })], [makeLayer('#ff0000', 'R'), makeLayer('#00ff00', 'V')])!;
    const back = svgToObjects(svg, '#000000')!;
    expect(back.objects).toHaveLength(2);
    expect(back.width).toBeCloseTo(50, 3);
    expect(back.height).toBeCloseTo(50, 3);
    expect(back.objects.map((o) => o.layer).sort()).toEqual(['#00ff00', '#ff0000']);
    const rect = back.objects.find((o) => o.layer === '#ff0000')!;
    expect(size(rect)).toEqual([20, 5].map((v) => expect.closeTo(v, 4)));
  });
});

describe('reconocer SVG', () => {
  it('con prólogo, comentarios o solo la etiqueta', () => {
    expect(looksLikeSvg('<svg xmlns="http://www.w3.org/2000/svg"></svg>')).toBe(true);
    expect(looksLikeSvg('<?xml version="1.0"?>\n<!-- hecho con otra app -->\n<svg width="1"/>')).toBe(true);
    expect(looksLikeSvg('hola <svg>')).toBe(false);
    expect(looksLikeSvg('<html><svg/></html>')).toBe(false);
    expect(looksLikeSvg('')).toBe(false);
  });
});

describe('colores y transformaciones', () => {
  it('colores CSS comunes', () => {
    expect(parseColor('#F00')).toBe('#ff0000');
    expect(parseColor('#00FF00')).toBe('#00ff00');
    expect(parseColor('rgb(0, 0, 255)')).toBe('#0000ff');
    expect(parseColor('rgb(100%, 0%, 0%)')).toBe('#ff0000');
    expect(parseColor('Blue')).toBe('#0000ff');
    expect(parseColor('none')).toBeNull();
    expect(parseColor('url(#grad)')).toBeNull();
    expect(parseColor(undefined)).toBeNull();
  });

  it('transform: traslación, escala, giro y encadenado', () => {
    expect(parseTransform('translate(5 7)')).toEqual([1, 0, 0, 1, 5, 7]);
    expect(parseTransform('scale(2)')).toEqual([2, 0, 0, 2, 0, 0]);
    expect(parseTransform('matrix(1 2 3 4 5 6)')).toEqual([1, 2, 3, 4, 5, 6]);
    const r = parseTransform('rotate(90)');
    expect(r[0]).toBeCloseTo(0, 9);
    expect(r[1]).toBeCloseTo(1, 9);
    expect(parseTransform('translate(10,0) scale(2)')).toEqual([2, 0, 0, 2, 10, 0]);
    expect(parseTransform(undefined)).toEqual([1, 0, 0, 1, 0, 0]);
  });
});

describe('datos de trayecto', () => {
  it('absolutos y relativos, H y V, cierre', () => {
    const p = parsePathData('M10 10 h20 v10 H10 z');
    expect(p.map((s) => s.type)).toEqual(['move_to', 'line_to', 'line_to', 'line_to', 'close']);
    expect(p[1]).toEqual({ type: 'line_to', to: [30, 10] });
    expect(p[2]).toEqual({ type: 'line_to', to: [30, 20] });
    expect(p[3]).toEqual({ type: 'line_to', to: [10, 20] });
  });

  it('pares extra tras M son rectas; números pegados y exponentes', () => {
    const p = parsePathData('M0,0 5,5 1e1-2');
    expect(p).toHaveLength(3);
    expect(p[2]).toEqual({ type: 'line_to', to: [10, -2] });
  });

  it('curvas: C, S (reflejo del control anterior), Q y T', () => {
    const p = parsePathData('M0 0 C0 10 10 10 10 0 S20 -10 20 0');
    expect(p[2]).toEqual({ type: 'cubic_to', ctrl1: [10, -10], ctrl2: [20, -10], to: [20, 0] });
    const q = parsePathData('M0 0 Q5 5 10 0 T20 0');
    expect(q[2]).toEqual({ type: 'quad_to', ctrl: [15, -5], to: [20, 0] });
  });

  it('un arco de media circunferencia queda sobre el círculo', () => {
    const p = parsePathData('M0 0 A10 10 0 0 1 20 0');
    const cubics = p.filter((s) => s.type === 'cubic_to');
    expect(cubics.length).toBeGreaterThanOrEqual(2);
    const last = p.at(-1)!;
    expect(last.type === 'cubic_to' && last.to).toEqual([20, 0]);
    for (const s of cubics) {
      if (s.type !== 'cubic_to') continue;
      expect(Math.hypot(s.to[0] - 10, s.to[1])).toBeCloseTo(10, 6);
    }
  });

  it('banderas de arco pegadas', () => {
    const p = parsePathData('M0 0a5 5 0 1010 0');
    expect(p.at(-1)).toMatchObject({ type: 'cubic_to', to: [10, 0] });
  });

  it('comandos desconocidos cortan sin romper', () => {
    expect(parsePathData('M0 0 L5 5 X9 9 L1 1')).toHaveLength(2);
    expect(parsePathData('')).toEqual([]);
  });
});

describe('leer SVG de otras aplicaciones', () => {
  const doc = (inner: string, attrs = 'width="100mm" height="50mm" viewBox="0 0 100 50"') =>
    `<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" ${attrs}>${inner}</svg>`;

  it('no SVG, o sin formas, da null', () => {
    expect(svgToObjects('hola', '#000000')).toBeNull();
    expect(svgToObjects(doc(''), '#000000')).toBeNull();
  });

  it('formas básicas: cada una es un objeto, con Y hacia arriba y el conjunto en el origen', () => {
    const r = svgToObjects(
      doc('<rect x="10" y="10" width="20" height="10" stroke="red" fill="none"/><circle cx="60" cy="25" r="5" stroke="blue" fill="none"/><line x1="0" y1="0" x2="10" y2="0" stroke="#0f0"/>'),
      '#000000',
    )!;
    expect(r.objects).toHaveLength(3);
    expect(r.objects.map((o) => o.layer)).toEqual(['#ff0000', '#0000ff', '#00ff00']);
    // El conjunto mide de x=0 a 65 y de y=0 a 30 (en SVG), en mm porque viewBox = tamaño.
    expect(r.width).toBeCloseTo(65, 4);
    expect(r.height).toBeCloseTo(30, 4);
    const b = objectBounds(r.objects[0]!)!;
    expect(b[0][0]).toBeCloseTo(10, 4);
    expect(b[0][1]).toBeCloseTo(10, 4); // el rectángulo estaba a 10 del tope de 30 de alto -> a 10 del fondo (30-20)
    const line = objectBounds(r.objects[2]!)!;
    expect(line[0][1]).toBeCloseTo(30, 4); // la línea en y=0 de SVG queda arriba
  });

  it('las unidades escalan a mm: viewBox 0 0 100 50 en 200 mm de ancho duplica', () => {
    const r = svgToObjects(doc('<rect width="10" height="10" stroke="red"/>', 'width="200mm" height="100mm" viewBox="0 0 100 50"'), '#000000')!;
    expect(r.width).toBeCloseTo(20, 6);
  });

  it('sin tamaño, 1 unidad es 1 px a 96 ppp', () => {
    const r = svgToObjects(doc('<rect width="96" height="96" stroke="red"/>', ''), '#000000')!;
    expect(r.width).toBeCloseTo(25.4, 6);
  });

  it('las transformaciones de grupo y los estilos se heredan', () => {
    const r = svgToObjects(
      doc('<g transform="translate(10 0)" style="stroke:#ff0000;fill:none"><path d="M0 0 L10 0"/></g><g transform="translate(0 20)"><path d="M0 0 L10 0" stroke="blue" fill="none"/></g>'),
      '#000000',
    )!;
    expect(r.objects[0]!.layer).toBe('#ff0000');
    const b = objectBounds(r.objects[0]!)!;
    expect(b[1][0] - b[0][0]).toBeCloseTo(10, 6);
    // El segundo grupo está 20 mm más abajo en SVG -> 20 mm más abajo en el documento.
    const b2 = objectBounds(r.objects[1]!)!;
    expect(b[0][1] - b2[0][1]).toBeCloseTo(20, 6);
  });

  it('sin trazo usa el relleno como color de capa y la marca como relleno', () => {
    const r = svgToObjects(doc('<path d="M0 0 L10 0 L10 10 Z" fill="#336699"/>'), '#000000')!;
    expect(r.objects[0]!.layer).toBe('#336699');
    expect(r.layers).toEqual([{ color: '#336699', fill: true }]);
  });

  it('lo que no tiene color propio va a la capa de respaldo', () => {
    const r = svgToObjects(doc('<path d="M0 0 L10 0" stroke="currentColor" fill="none"/>'), '#abcdef')!;
    expect(r.objects[0]!.layer).toBe('#abcdef');
  });

  it('omite defs, ocultos y lo no vectorial, y lo cuenta', () => {
    const r = svgToObjects(
      doc('<defs><path id="x" d="M0 0 L5 5" stroke="red"/></defs><path d="M0 0 L10 10" stroke="red" display="none"/><text x="1" y="1">hola</text><image href="a.png"/><path d="M0 0 L10 10" stroke="red"/>'),
      '#000000',
    )!;
    expect(r.objects).toHaveLength(1);
    expect(r.skipped).toBe(2);
  });

  it('polígonos, polilíneas y rectángulos redondeados', () => {
    const r = svgToObjects(
      doc('<polygon points="0,0 10,0 10,10" stroke="red"/><polyline points="0 0 5 5" stroke="red"/><rect width="20" height="10" rx="2" stroke="red"/>'),
      '#000000',
    )!;
    expect(r.objects).toHaveLength(3);
    expect(r.objects[0]!.kind.type === 'vector' && r.objects[0]!.kind.paths[0]!.at(-1)).toEqual({ type: 'close' });
    expect(r.objects[2]!.kind.type === 'vector' && r.objects[2]!.kind.paths[0]!.some((s) => s.type === 'cubic_to')).toBe(true);
  });

  it('entidades y comillas simples en atributos', () => {
    const r = svgToObjects(doc("<path d='M0 0 L10 0' stroke='red' style='fill:none;stroke:&#35;00ff00'/>"), '#000000')!;
    expect(r.objects[0]!.layer).toBe('#00ff00');
  });
});
