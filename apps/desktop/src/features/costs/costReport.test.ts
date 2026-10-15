import { afterEach, describe, expect, it } from 'vitest';
import { setLanguage } from '@/i18n';
import { computeCost, DEFAULT_MATERIAL, DEFAULT_SALES, type CostInputs } from './costModel';
import { csvField, reportCsv, reportRows, reportText, type ReportContext } from './costReport';

const INPUTS: CostInputs = {
  job: { runS: 3600, laserOnS: 1800, bboxMm2: 10_000, realMm2: 6_000 },
  machine: { hourlyRate: 60, tubePrice: 1200, tubeLifeH: 2000, watts: 100, accessoryWatts: 20 },
  material: { ...DEFAULT_MATERIAL, sheetPrice: 120, wastePct: 0 },
  materialMode: 'bbox',
  sales: { ...DEFAULT_SALES, kwhPrice: 2.5, laborRate: 100, prepMin: 30, finishMin: 6, consumables: 2, margin: 30, commission: 10 },
  piecesInDesign: 1,
  quantity: 10,
  timeUsed: 'calibrated',
};

function ctx(over: Partial<ReportContext> = {}, inputs: CostInputs = INPUTS): ReportContext {
  return { inputs, result: computeCost(inputs), project: 'llaveros.tlaku', machine: 'Falcon2 Pro', timeUsed: 'calibrated', cutMm: 1234, digits: 2, tag: 'es-MX', date: '2026-10-15', ...over };
}

afterEach(() => setLanguage('es'));

describe('desglose en texto', () => {
  it('lleva el trabajo, el costo, el precio y el lote en español', () => {
    setLanguage('es');
    const text = reportText(ctx());
    expect(text.split('\n')[0]).toBe('Costos y precio por pieza');
    for (const s of [
      'Proyecto: llaveros.tlaku',
      'Máquina: Falcon2 Pro',
      'Tiempo por pasada: 1:00:00',
      'Longitud de corte: 123.4 cm',
      'Máquina (tiempo): $60.00',
      'Material: $5.00',
      'Costo por pieza: $82.60',
      'Precio sugerido por pieza: $119.32',
      'Precio del lote: $1,193.20',
    ]) {
      expect(text).toContain(s);
    }
    expect(text).not.toContain('Hojas necesarias'); // solo con el modo «por hoja»
    expect(text).toContain('Margen (el costo): 30.0 %');
  });

  it('el inglés usa otros textos y el formato de su región', () => {
    setLanguage('en');
    const text = reportText(ctx({ tag: 'en-US' }, { ...INPUTS, sales: { ...INPUTS.sales, currency: 'USD' } }));
    expect(text.split('\n')[0]).toBe('Cost and price per piece');
    expect(text).toContain('Suggested price per piece: $119.32');
    expect(text).toContain('Machine: Falcon2 Pro');
    expect(text).toContain('Margin (the cost): 30.0 %');
  });

  it('las notas del cálculo se incluyen', () => {
    setLanguage('es');
    expect(reportText(ctx())).toContain('- El material se calcula con el rectángulo');
    expect(reportText(ctx())).toContain('NOTAS');
  });

  it('con el modo por hoja dice cuántas hojas', () => {
    setLanguage('es');
    const sheet = { ...INPUTS, materialMode: 'sheet' as const, material: { ...INPUTS.material, piecesPerSheet: 6 } };
    expect(reportText(ctx({}, sheet))).toContain('Hojas necesarias: 2');
  });

  it('sin precio posible el desglose lo dice con un guion', () => {
    const bad = { ...INPUTS, sales: { ...INPUTS.sales, commission: 70, tax: 40 } };
    const text = reportText(ctx({}, bad));
    expect(text).toContain('Precio sugerido por pieza: —');
    expect(text).toContain('suman 100 % o más');
  });
});

describe('desglose en CSV', () => {
  it('encabezado, filas con punto decimal y sin separador de miles', () => {
    setLanguage('es');
    const csv = reportCsv(ctx());
    const lines = csv.trimEnd().split('\r\n');
    expect(lines[0]).toBe('Sección,Concepto,Valor,Unidad');
    expect(lines).toContain('Costo por pieza,Máquina (tiempo),60.00,MXN');
    expect(lines).toContain('Precio por pieza,Precio sugerido por pieza,119.32,MXN');
    expect(lines).toContain('Lote,Precio del lote,1193.20,MXN');
    expect(lines).toContain('Trabajo,Tiempo por pasada,3600,s');
    expect(lines).toContain('Precio por pieza,Margen (el costo),30.00,%');
    expect(csv.endsWith('\r\n')).toBe(true);
  });

  it('cada fila tiene cuatro campos aunque el proyecto lleve comas', () => {
    const csv = reportCsv(ctx({ project: 'caja, "grande".tlaku' }));
    expect(csv).toContain('"caja, ""grande"".tlaku"');
    const rows = csv.trimEnd().split('\r\n').filter((l) => !l.includes('"'));
    for (const r of rows) expect(r.split(',').length, r).toBe(4);
  });

  it('el inglés traduce el encabezado y los conceptos', () => {
    setLanguage('en');
    const lines = reportCsv(ctx({ tag: 'en-US' })).trimEnd().split('\r\n');
    expect(lines[0]).toBe('Section,Item,Value,Unit');
    expect(lines).toContain('Cost per piece,Machine (time),60.00,MXN');
  });

  it('las notas van como filas', () => {
    setLanguage('es');
    expect(reportCsv(ctx())).toMatch(/^Notas,"El material se calcula/m);
  });
});

describe('campos del CSV', () => {
  it('entrecomilla comas, comillas, saltos de línea y punto y coma', () => {
    expect(csvField('simple')).toBe('simple');
    expect(csvField('a,b')).toBe('"a,b"');
    expect(csvField('di "hola"')).toBe('"di ""hola"""');
    expect(csvField('una\nlínea')).toBe('"una\nlínea"');
    expect(csvField('a;b')).toBe('"a;b"');
  });

  it('un texto que una hoja de cálculo tomaría por fórmula se neutraliza; los números negativos no', () => {
    expect(csvField('=SUMA(A1)')).toBe("'=SUMA(A1)");
    expect(csvField('+52 55')).toBe("'+52 55");
    expect(csvField('@usuario')).toBe("'@usuario");
    expect(csvField('-12.50')).toBe('-12.50');
    expect(csvField('-pieza')).toBe("'-pieza");
  });
});

describe('filas', () => {
  it('los títulos de sección no llevan valor', () => {
    setLanguage('es');
    const heads = reportRows(ctx()).filter((r) => r.heading);
    expect(heads.map((h) => h.label)).toEqual(['Trabajo', 'Costo por pieza', 'Precio por pieza', 'Lote']);
    expect(heads.every((h) => h.raw === '' && h.display === '')).toBe(true);
  });

  it('las líneas de costo en cero no estorban', () => {
    setLanguage('es');
    const none = reportRows(ctx({}, { ...INPUTS, machine: { hourlyRate: 0, tubePrice: 0, tubeLifeH: 0, watts: 0, accessoryWatts: 0 } }));
    expect(none.some((r) => r.label === 'Máquina (tiempo)')).toBe(false);
  });
});
