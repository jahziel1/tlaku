import { describe, expect, it } from 'vitest';
import { makeLayer } from '@/lib/layers';
import { axisValues, buildParams, defaultForm, fixedParams, gridLayout, labelWidth, paramsFor, setAxisParam, setMode, validateForm, valueText } from './testPlan';
import { shownIndexes } from './TestDialog';

describe('valores de un eje', () => {
  it('interpola de forma lineal e inclusiva', () => {
    expect(axisValues({ param: 'power', count: 5, from: 20, to: 100 })).toEqual([20, 40, 60, 80, 100]);
    expect(axisValues({ param: 'speed', count: 3, from: 1000, to: 500 })).toEqual([1000, 750, 500]);
  });

  it('con una sola celda usa "desde"', () => {
    expect(axisValues({ param: 'power', count: 1, from: 35, to: 90 })).toEqual([35]);
  });

  it('las pasadas se redondean a entero de al menos 1', () => {
    expect(axisValues({ param: 'passes', count: 4, from: 0.2, to: 3 })).toEqual([1, 1, 2, 3]);
  });
});

describe('formulario', () => {
  const active = makeLayer('#ff0000', 'Corte', { speed: 700, power: 80, passes: 3, air: true, angle: 12, interval: 0.2 });

  it('parte de la capa activa si la hay, o de los valores de fábrica', () => {
    expect(defaultForm(active).fromActiveLayer).toBe(true);
    expect(defaultForm(active).fixed).toMatchObject({ speed: 700, power: 80, passes: 3, interval: 0.2 });
    expect(defaultForm(null).fromActiveLayer).toBe(false);
    expect(defaultForm(null).rows.param).not.toBe(defaultForm(null).cols.param);
  });

  it('arma los parámetros para el motor con la base de la capa activa y los valores fijos', () => {
    const form = { ...defaultForm(active), fixed: { speed: 900, power: 50, passes: 2, interval: 0.1 } };
    const p = buildParams(form, active);
    expect(p).toMatchObject({ mode: 'line', cell: [10, 10], gap: 3, labels: true, origin: [5, 5] });
    expect(p.base).toMatchObject({ air: true, angle: 12, speed: 900, power: 50, passes: 2, mode: 'line', order: null });
    const plain = buildParams({ ...form, fromActiveLayer: false }, active);
    expect(plain.base.air).toBe(false);
  });

  it('cambiar el parámetro de un eje intercambia con el otro si chocan', () => {
    const f = defaultForm(null); // filas: potencia, columnas: velocidad
    const swapped = setAxisParam(f, 'rows', 'speed');
    expect(swapped.rows.param).toBe('speed');
    expect(swapped.cols.param).toBe('power');
    const other = setAxisParam(f, 'cols', 'passes');
    expect(other.cols).toMatchObject({ param: 'passes', count: 4 });
    expect(other.rows.param).toBe('power');
  });

  it('pasar a línea cambia un eje de intervalo por otro parámetro libre', () => {
    let f = setMode(defaultForm(null), 'fill');
    f = setAxisParam(f, 'cols', 'interval');
    const line = setMode(f, 'line');
    expect(line.mode).toBe('line');
    expect([line.rows.param, line.cols.param]).not.toContain('interval');
    expect(line.rows.param).not.toBe(line.cols.param);
  });

  it('el intervalo solo se ofrece en relleno', () => {
    expect(paramsFor('line')).not.toContain('interval');
    expect(paramsFor('fill')).toContain('interval');
  });

  it('los valores fijos son los parámetros que ningún eje varía', () => {
    const f = defaultForm(null);
    expect(fixedParams(f)).toEqual(['passes']);
    expect(fixedParams({ ...f, mode: 'fill' })).toEqual(['passes', 'interval']);
  });
});

describe('validar', () => {
  const ok = defaultForm(null);
  const problem = (patch: Partial<typeof ok>) => validateForm({ ...ok, ...patch })?.key ?? null;

  it('el formulario por defecto es válido', () => {
    expect(validateForm(ok)).toBeNull();
    expect(validateForm({ ...ok, mode: 'fill' })).toBeNull();
  });

  it('la cantidad va de 1 a 30 por eje', () => {
    expect(problem({ rows: { ...ok.rows, count: 0 } })).toBe('materials.test.err.count');
    expect(problem({ cols: { ...ok.cols, count: 31 } })).toBe('materials.test.err.count');
    expect(problem({ cols: { ...ok.cols, count: 30 } })).toBeNull();
    expect(problem({ cols: { ...ok.cols, count: 2.5 } })).toBe('materials.test.err.count');
  });

  it('la potencia va de 0 a 100 y la velocidad es positiva', () => {
    expect(problem({ rows: { ...ok.rows, to: 120 } })).toBe('materials.test.err.power');
    expect(problem({ rows: { ...ok.rows, from: -1 } })).toBe('materials.test.err.power');
    expect(problem({ cols: { ...ok.cols, from: 0 } })).toBe('materials.test.err.positive');
  });

  it('el intervalo en modo línea es un error', () => {
    expect(problem({ cols: { param: 'interval', count: 3, from: 0.1, to: 0.3 } })).toBe('materials.test.err.intervalLine');
    expect(problem({ mode: 'fill', cols: { param: 'interval', count: 3, from: 0.1, to: 0.3 } })).toBeNull();
  });

  it('exige parámetros distintos y tamaños positivos', () => {
    expect(problem({ cols: { ...ok.cols, param: 'power' } })).toBe('materials.test.err.sameParam');
    expect(problem({ cell: [0, 5] })).toBe('materials.test.err.cell');
    expect(problem({ gap: -1 })).toBe('materials.test.err.gap');
  });

  it('con rótulos la celda debe ser lo bastante ancha para el texto', () => {
    expect(problem({ cell: [3, 10], gap: 0 })).toBe('materials.test.err.labelWidth');
    expect(problem({ cell: [3, 10], gap: 0, labels: false })).toBeNull();
    expect(problem({ cell: [10, 2], gap: 0 })).toBe('materials.test.err.labelHeight');
  });
});

describe('geometría de la vista previa', () => {
  it('mide los rótulos como la fuente del motor (3 mm de alto)', () => {
    expect(labelWidth('12')).toBeCloseTo(3.75, 9);
    expect(labelWidth('1.5')).toBeCloseTo((2 + 1 + 0.6 + 1 + 2) * 0.75, 9);
  });

  it('da el tamaño total con y sin rótulos', () => {
    const f = { ...defaultForm(null), cell: [10, 10] as [number, number], gap: 2 };
    const plain = gridLayout({ ...f, labels: false });
    expect(plain).toMatchObject({ gridX: 0, width: 5 * 10 + 4 * 2, height: 5 * 10 + 4 * 2 });
    const labeled = gridLayout(f);
    expect(labeled.width).toBeGreaterThan(plain.width);
    expect(labeled.height).toBeCloseTo(plain.height + 2 + 3 + 2 + 3, 9);
  });

  it('formatea los valores como el motor', () => {
    expect(valueText('power', 37.5)).toBe('37.5');
    expect(valueText('power', 40)).toBe('40');
    expect(valueText('speed', 1200.4)).toBe('1200');
    expect(valueText('interval', 0.1)).toBe('0.1');
  });

  it('rotula todos los valores si son pocos y solo algunos si son muchos', () => {
    expect(shownIndexes(4)).toEqual([0, 1, 2, 3]);
    expect(shownIndexes(30)).toEqual([0, 10, 19, 29]);
  });
});
