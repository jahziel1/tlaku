import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getApi, setApi } from '@/api';
import type { LaserApi } from '@/api';
import { mockGenerateCode } from '@/api/mock/codes';
import type { CsvTable, DataSource, DesignObject, Document, VariableText } from '@/api/types';
import { polylineBez } from '@/lib/bezpath';
import { objectBounds } from '@/lib/geometry';
import { canvasDisplayObjects, onCanvasDisplayChange, resetCanvasDisplay } from './canvasDisplay';
import { EMPTY_CSV, useVariablesUi } from './context';
import { DEFAULT_VARIABLE_TEXT } from './defaults';
import { variableCodeOf, type VariableCodeObject, type VariableImageObject } from './varObjects';

const TABLE: CsvTable = {
  headers: ['Logo', 'Sku'],
  rows: [['logos/taza.png', 'T-1'], ['/abs/llavero.png', 'L-2']],
  delimiter: ',',
};
const source: DataSource = { name: 'datos', path: '/datos/p.csv', on_short: 'repeat' };

const vt = (row: number): VariableText => ({ ...DEFAULT_VARIABLE_TEXT, sources: [source], csv_path: source.path, csv_row: row });
const base = (id: string, kind: DesignObject['kind']): DesignObject => ({
  id, layer: '#0000ff', transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null, kind,
});
const image = (id: string, template: string): VariableImageObject =>
  base(id, { type: 'image', source_path: '/muestra.png', width_px: 100, height_px: 100, size_mm: [20, 10], mask: [], variable: { source: template, fit: 'contain' } }) as VariableImageObject;
const code = (id: string, data: string): VariableCodeObject =>
  base(id, {
    type: 'vector',
    paths: [polylineBez([[0, 0], [30, 0], [30, 30], [0, 30]], true)],
    code: variableCodeOf({ kind: 'qr', data, ecc: 'm', size_mm: 30, height_mm: 15, margin_mm: 2, human_readable: false, invert: false, font_family: '' }),
  }) as VariableCodeObject;
const doc = (objects: DesignObject[], row = 0): Document => ({ version: 2, objects, layers: [], device_id: null, variable_text: vt(row) });

const loadCsv = () => useVariablesUi.setState({ csv: { path: source.path, table: TABLE, error: null, loading: false }, extra: {}, showTemplate: false });
/** Deja que terminen las promesas de la API simulada. */
const flush = () => new Promise((r) => setTimeout(r, 0));

let original: LaserApi;
let probeImages: ReturnType<typeof vi.fn>;
let generateCode: ReturnType<typeof vi.fn>;

beforeEach(() => {
  resetCanvasDisplay();
  original = getApi();
  probeImages = vi.fn(async ({ paths }: { paths: string[] }) => paths.map((path) => ({ path, ok: !/falta/.test(path), width_px: 200, height_px: 100, error: null })));
  generateCode = vi.fn(async ({ spec }) => mockGenerateCode(spec));
  setApi({ ...original, probeImages, generateCode } as unknown as LaserApi);
  useVariablesUi.setState({ csv: EMPTY_CSV, extra: {}, showTemplate: false });
});

afterEach(() => setApi(original));

describe('imágenes variables en el lienzo', () => {
  it('sin objetos variables devuelve el mismo arreglo', () => {
    loadCsv();
    const d = doc([base('r', { type: 'vector', paths: [], code: null } as never)]);
    expect(canvasDisplayObjects(d, d.objects)).toBe(d.objects);
  });

  it('muestra la muestra mientras el archivo se sondea y después la imagen de la fila actual', async () => {
    loadCsv();
    const img = image('i', '{csv:Logo}');
    const d = doc([img]);
    const notified = vi.fn();
    const off = onCanvasDisplayChange(notified);
    expect(canvasDisplayObjects(d, d.objects)).toBe(d.objects);
    await flush();
    expect(notified).toHaveBeenCalled();
    off();
    const shown = canvasDisplayObjects(d, d.objects);
    expect(shown).not.toBe(d.objects);
    const o = shown[0]!;
    expect(o.kind.type === 'image' && o.kind.source_path).toBe('/datos/logos/taza.png');
    // Encajada en su caja (20 x 10) con la proporción 2:1 del archivo.
    expect(o.kind.type === 'image' && o.kind.size_mm).toEqual([20, 10]);
    // El documento no se toca.
    expect(d.objects[0]).toBe(img);
    expect(img.kind.type === 'image' && img.kind.source_path).toBe('/muestra.png');
  });

  it('es la misma respuesta (caché) mientras nada cambie y no vuelve a sondear', async () => {
    loadCsv();
    const d = doc([image('i', '{csv:Logo}')]);
    canvasDisplayObjects(d, d.objects);
    await flush();
    const a = canvasDisplayObjects(d, d.objects);
    expect(canvasDisplayObjects(d, d.objects)).toBe(a);
    // Un documento nuevo con el mismo objeto reutiliza la copia ya armada.
    const d2 = { ...d, objects: [...d.objects] };
    expect(canvasDisplayObjects(d2, d2.objects)[0]).toBe(a[0]);
    expect(probeImages).toHaveBeenCalledTimes(1);
  });

  it('al cambiar de fila cambia la imagen; una fila ya vista no se pide otra vez', async () => {
    loadCsv();
    const img = image('i', '{csv:Logo}');
    canvasDisplayObjects(doc([img], 0), [img]);
    await flush();
    const row1 = doc([img], 1);
    canvasDisplayObjects(row1, row1.objects);
    await flush();
    const shown = canvasDisplayObjects(row1, row1.objects)[0]!;
    expect(shown.kind.type === 'image' && shown.kind.source_path).toBe('/abs/llavero.png');
    const row0 = doc([img], 0);
    expect((canvasDisplayObjects(row0, row0.objects)[0]!.kind as { source_path: string }).source_path).toBe('/datos/logos/taza.png');
    expect(probeImages).toHaveBeenCalledTimes(2);
  });

  it('un archivo ausente deja la muestra, y se vuelve a mirar al cambiar los datos', async () => {
    loadCsv();
    const img = image('i', 'logos/falta.png');
    const d = doc([img]);
    canvasDisplayObjects(d, d.objects);
    await flush();
    expect(canvasDisplayObjects(d, d.objects)[0]).toBe(img);
    useVariablesUi.setState({ csv: { path: source.path, table: { ...TABLE }, error: null, loading: false } });
    canvasDisplayObjects(d, d.objects);
    await flush();
    expect(probeImages).toHaveBeenCalledTimes(2);
  });

  it('viendo la plantilla se ve la muestra', async () => {
    loadCsv();
    const d = doc([image('i', '{csv:Logo}')]);
    canvasDisplayObjects(d, d.objects);
    await flush();
    useVariablesUi.setState({ showTemplate: true });
    expect(canvasDisplayObjects(d, d.objects)).toBe(d.objects);
  });

  it('sin el CSV leído deja la muestra', () => {
    const d = doc([image('i', '{csv:Logo}')]);
    expect(canvasDisplayObjects(d, d.objects)).toBe(d.objects);
    expect(probeImages).not.toHaveBeenCalled();
  });
});

describe('códigos variables en el lienzo', () => {
  it('regenera el código con la fila actual, centrado donde estaba, sin tocar el documento', async () => {
    loadCsv();
    const qr = code('c', 'https://x.test/{csv:Sku}');
    const d = doc([qr]);
    canvasDisplayObjects(d, d.objects);
    await flush();
    const shown = canvasDisplayObjects(d, d.objects);
    expect(shown[0]).not.toBe(qr);
    expect(generateCode).toHaveBeenCalledTimes(1);
    expect(generateCode.mock.calls[0]![0].spec.data).toBe('https://x.test/T-1');
    const before = objectBounds(qr)!;
    const after = objectBounds(shown[0]!)!;
    expect((before[0][0] + before[1][0]) / 2).toBeCloseTo((after[0][0] + after[1][0]) / 2, 6);
    expect(d.objects[0]).toBe(qr);
    // Otra fila: otro contenido; la primera no se vuelve a generar.
    const row1 = doc([qr], 1);
    canvasDisplayObjects(row1, row1.objects);
    await flush();
    canvasDisplayObjects(row1, row1.objects);
    expect(generateCode.mock.calls.map((c) => c[0].spec.data)).toEqual(['https://x.test/T-1', 'https://x.test/L-2']);
    const row0 = doc([qr], 0);
    canvasDisplayObjects(row0, row0.objects);
    expect(generateCode).toHaveBeenCalledTimes(2);
  });

  it('un código sin variables no se regenera', () => {
    loadCsv();
    const d = doc([code('c', 'https://x.test/fijo')]);
    expect(canvasDisplayObjects(d, d.objects)).toBe(d.objects);
    expect(generateCode).not.toHaveBeenCalled();
  });

  it('si el generador falla, deja la muestra', async () => {
    loadCsv();
    generateCode.mockRejectedValue(new Error('no'));
    const d = doc([code('c', '{csv:Sku}')]);
    canvasDisplayObjects(d, d.objects);
    await flush();
    expect(canvasDisplayObjects(d, d.objects)).toBe(d.objects);
  });
});
