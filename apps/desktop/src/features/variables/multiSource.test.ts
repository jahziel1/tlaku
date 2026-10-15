import { beforeEach, describe, expect, it } from 'vitest';
import { mockGenerateCode } from '@/api/mock/codes';
import { mockTextOutline } from '@/api/mock/text';
import type { CodeSpec, CsvTable, DataSource, DesignObject, Document, ImageProbe, TextObject, VariableText } from '@/api/types';
import { isTextObject } from '@/features/text/signature';
import { setLanguage } from '@/i18n';
import { polylineBez } from '@/lib/bezpath';
import { objectBounds } from '@/lib/geometry';
import { useStore } from '@/store';
import { advanceSettings, jobSpans } from './advance';
import { checkData } from './check';
import { placeCodeResult } from './codeVar';
import { copiesPerRow } from './copies';
import { displayContext, EMPTY_CSV, useVariablesUi } from './context';
import { DEFAULT_VARIABLE_TEXT, normalizeVariableText, variableTextOf } from './defaults';
import { csvBaseDir, fitImageObject, joinPath, resolveImagePath } from './imageVar';
import { resolveDocForOutput, VariableError } from './jobResolve';
import { makeContext, resolveTemplate } from './resolve';
import { editVariableText, renameSourceInDoc } from './settings';
import { addSource, makeMain, removeSource, renameInTemplate, rowFits } from './sources';
import { csvColumnsBySource, parseTemplate, serializeTemplate, shiftTemplate, sourcesUsed } from './syntax';
import { variableCodeOf, type VariableCodeObject, type VariableImageObject } from './varObjects';

const NOW = new Date(2025, 8, 3, 14, 5, 9);

const PEOPLE: CsvTable = {
  headers: ['Nombre', 'Cargo'],
  rows: [['Ana', 'Jefa'], ['Luis', 'Técnico'], ['Eva', 'Diseño'], ['Iván', 'Ventas']],
  delimiter: ',',
};
const PRODUCTS: CsvTable = {
  headers: ['Producto', 'Logo', 'Sku'],
  rows: [['Taza', 'logos/taza.png', 'T-1'], ['Llavero', '/abs/llavero.png', 'L-2']],
  delimiter: ',',
};

const src = (name: string, path: string, on_short: DataSource['on_short'] = 'repeat'): DataSource => ({ name, path, on_short });
const SOURCES = [src('personas', '/datos/personas.csv'), src('productos', '/datos/productos.csv')];

const vtOf = (patch: Partial<VariableText> = {}): VariableText => ({ ...DEFAULT_VARIABLE_TEXT, sources: SOURCES, csv_path: SOURCES[0]!.path, ...patch });
const ctxOf = (patch: Partial<VariableText> = {}, row = 0) =>
  makeContext({ ...vtOf(patch), csv_row: row }, PEOPLE, NOW, 'es', { productos: PRODUCTS });

const textKind = (text: string): TextObject => ({
  text, font_family: 'Arial', bold: false, italic: false, size_mm: 10, letter_spacing_mm: 0,
  line_spacing: 1.2, align: 'left', on_path: null, outline: [],
});
const base = (id: string, kind: DesignObject['kind'], extra: Partial<DesignObject> = {}): DesignObject => ({
  id, layer: '#0000ff', transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null, kind, ...extra,
});
const textObj = (id: string, text: string) => base(id, { type: 'text', ...textKind(text) });
const imageObj = (id: string, source: string, extra: Partial<DesignObject> = {}, fit: 'contain' | 'cover' = 'contain') =>
  base(id, { type: 'image', source_path: '/placeholder.png', width_px: 100, height_px: 100, size_mm: [20, 10], mask: [], variable: { source, fit } }, extra) as VariableImageObject;
const codeObj = (id: string, data: string, extra: Partial<DesignObject> = {}) =>
  base(
    id,
    { type: 'vector', paths: [polylineBez([[0, 0], [30, 0], [30, 30], [0, 30]], true)], code: variableCodeOf({ kind: 'qr', data, ecc: 'm', size_mm: 30, height_mm: 15, margin_mm: 2, human_readable: false, invert: false, font_family: '' }) },
    extra,
  ) as VariableCodeObject;
const doc = (objects: DesignObject[], vt: Partial<VariableText> = {}): Document => ({
  version: 2, objects, layers: [], device_id: null, variable_text: vtOf(vt),
});

beforeEach(() => {
  setLanguage('es');
  useVariablesUi.setState({ csv: EMPTY_CSV, extra: {} });
});

describe('sintaxis con fuente: {csv.fuente:Columna}', () => {
  it('lee la fuente y la columna, y la conserva al reescribir', () => {
    const [tok] = parseTemplate('{csv.productos:Producto@2}');
    expect(tok).toMatchObject({ kind: 'var', name: 'csv', source: 'productos', arg: 'Producto', offset: 2 });
    expect(serializeTemplate(parseTemplate('A {csv.productos:Sku} B {fila.productos}'))).toBe('A {csv.productos:Sku} B {fila.productos}');
    expect(parseTemplate('{csv:Nombre}')[0]).toMatchObject({ source: '' });
  });

  it('solo csv y fila admiten fuente; un nombre vacío o en otra variable es desconocido', () => {
    expect(parseTemplate('{fecha.x}')[0]).toMatchObject({ kind: 'bad', reason: 'unknown' });
    expect(parseTemplate('{serie.x:3}')[0]).toMatchObject({ kind: 'bad', reason: 'unknown' });
    expect(parseTemplate('{csv.:Nombre}')[0]).toMatchObject({ kind: 'bad', reason: 'unknown' });
  });

  it('agrupa las columnas por fuente (en minúsculas) y lista las fuentes usadas', () => {
    const tpl = '{csv:Nombre} {csv.Productos:Sku} {csv.productos:Logo} {fila.otra}';
    const by = csvColumnsBySource(tpl);
    expect(by.get('')).toEqual(['Nombre']);
    expect(by.get('productos')).toEqual(['Sku', 'Logo']);
    expect(sourcesUsed(tpl).sort()).toEqual(['', 'otra', 'productos']);
  });

  it('la copia k desplaza también las variables de fuentes secundarias', () => {
    expect(shiftTemplate('{csv.productos:Sku}', 2)).toBe('{csv.productos:Sku@2}');
  });
});

describe('resolución con varias fuentes', () => {
  it('cada variable elige su fuente y columna; sin fuente es la principal', () => {
    const c = ctxOf();
    expect(resolveTemplate('{csv:Nombre} / {csv.productos:Producto} / {csv.PRODUCTOS:sku}', c).text).toBe('Ana / Taza / T-1');
    expect(resolveTemplate('{csv.personas:Cargo}', c).text).toBe('Jefa');
  });

  it('la fuente más corta se repite desde el inicio cuando es la fila 3 de 2', () => {
    // La principal tiene 4 filas y la secundaria 2: la fila 3 (índice 2) de la principal lee la fila 1 de la otra.
    const c = ctxOf({}, 2);
    expect(resolveTemplate('{csv:Nombre} {csv.productos:Producto} {fila.productos}', c).text).toBe('Eva Taza 1');
    expect(resolveTemplate('{csv.productos:Producto@1}', c).text).toBe('Llavero');
    const r = resolveTemplate('{csv.productos:Producto}', ctxOf({}, 3));
    expect(r.text).toBe('Llavero');
  });

  it('con «bloquear» la fuente corta avisa con su nombre y la fila', () => {
    const c = ctxOf({ sources: [SOURCES[0]!, src('productos', '/datos/productos.csv', 'block')] }, 2);
    const r = resolveTemplate('{csv.productos:Producto}', c);
    expect(r.text).toBe('{csv.productos:Producto}');
    expect(r.issues).toEqual([{ code: 'sourceRowEnd', raw: '{csv.productos:Producto}', detail: 'productos 3' }]);
  });

  it('la fuente principal manda: pasada su última fila se acabaron los datos para todas', () => {
    const c = ctxOf({}, 4);
    expect(resolveTemplate('{csv.productos:Producto}', c).issues[0]).toMatchObject({ code: 'rowEnd', detail: '5' });
    // Una secundaria más larga no alarga el trabajo: sobran sus filas.
    const long: CsvTable = { ...PRODUCTS, rows: [...PRODUCTS.rows, ['A', 'b', 'c'], ['B', 'b', 'c'], ['C', 'b', 'c']] };
    const c2 = makeContext({ ...vtOf(), csv_row: 4 }, PEOPLE, NOW, 'es', { productos: long });
    expect(resolveTemplate('{csv.productos:Producto}', c2).issues[0]?.code).toBe('rowEnd');
  });

  it('avisa de fuente desconocida, sin leer y sin filas', () => {
    expect(resolveTemplate('{csv.nada:X}', ctxOf()).issues[0]).toMatchObject({ code: 'noSource', detail: 'nada' });
    const unread = makeContext(vtOf(), PEOPLE, NOW, 'es', {});
    expect(resolveTemplate('{csv.productos:Sku}', unread).issues[0]).toMatchObject({ code: 'noCsv', detail: 'productos' });
    const empty = makeContext(vtOf(), PEOPLE, NOW, 'es', { productos: { ...PRODUCTS, rows: [] } });
    expect(resolveTemplate('{csv.productos:Sku}', empty).issues[0]).toMatchObject({ code: 'emptySource' });
  });

  it('nombrar la principal por su nombre equivale a no nombrarla', () => {
    expect(resolveTemplate('{csv.personas:Nombre}', ctxOf()).text).toBe('Ana');
  });

  it('las celdas vacías se anotan aparte (no son error)', () => {
    const t: CsvTable = { headers: ['A'], rows: [[''], ['x']], delimiter: ',' };
    const c = makeContext({ ...DEFAULT_VARIABLE_TEXT, csv_path: 'a.csv' }, t, NOW, 'es');
    const r = resolveTemplate('[{csv:A}]', c);
    expect(r.text).toBe('[]');
    expect(r.issues).toEqual([]);
    expect(r.empties).toEqual([{ code: 'emptyCell', raw: '{csv:A}', detail: 'A', row: 1 }]);
  });
});

describe('documentos anteriores y normalización', () => {
  it('un documento con un solo CSV (csv_path) pasa a tener una fuente principal', () => {
    const vt = variableTextOf({ variable_text: { ...DEFAULT_VARIABLE_TEXT, csv_path: '/x/Mis etiquetas.csv', csv_row: 3 } });
    expect(vt.sources).toEqual([{ name: 'Misetiquetas', path: '/x/Mis etiquetas.csv', on_short: 'repeat' }]);
    expect(vt.csv_path).toBe('/x/Mis etiquetas.csv');
    expect(vt.csv_row).toBe(3);
  });

  it('un documento sin ajustes de variables no tiene fuentes', () => {
    expect(variableTextOf({}).sources).toEqual([]);
    expect(variableTextOf({}).csv_path).toBeNull();
  });

  it('los nombres se limpian y no se repiten; csv_path sigue a la principal; es idempotente', () => {
    const n = normalizeVariableText({ ...DEFAULT_VARIABLE_TEXT, csv_path: '/viejo.csv', sources: [src(' Lista A! ', '/a.csv'), src('listaa', '/b.csv'), src('?', '/c.csv')] });
    expect(n.sources.map((s) => s.name)).toEqual(['ListaA', 'listaa2', 'datos3']);
    expect(n.csv_path).toBe('/a.csv');
    expect(normalizeVariableText(n)).toBe(n);
  });

  it('un contexto armado con el CSV único antiguo resuelve igual que antes', () => {
    const c = makeContext({ ...DEFAULT_VARIABLE_TEXT, csv_path: 'a.csv', csv_row: 1 }, PEOPLE, NOW, 'es');
    expect(resolveTemplate('{csv:Nombre} {fila}', c).text).toBe('Luis 2');
  });
});

describe('operaciones sobre la lista de fuentes', () => {
  it('agregar usa el nombre del archivo y evita repetidos', () => {
    const a = addSource([], '/x/Lista de precios.csv');
    expect(a[0]).toEqual({ name: 'Listadeprecios', path: '/x/Lista de precios.csv', on_short: 'repeat' });
    expect(addSource(a, '/otro/listadeprecios.csv')[1]!.name).toBe('listadeprecios2');
  });

  it('hacer principal mueve la fuente al inicio y quitar no deja huecos', () => {
    const s = [src('a', '/a'), src('b', '/b'), src('c', '/c')];
    expect(makeMain(s, 2).map((x) => x.name)).toEqual(['c', 'a', 'b']);
    expect(removeSource(s, 0).map((x) => x.name)).toEqual(['b', 'c']);
  });

  it('avisa cómo se llevan las filas de las demás fuentes con la principal', () => {
    const s = [src('a', '/a'), src('b', '/b'), src('c', '/c', 'block'), src('d', '/d'), src('e', '/e')];
    const t = (n: number): CsvTable => ({ headers: ['x'], rows: Array.from({ length: n }, () => ['1']), delimiter: ',' });
    const fits = rowFits(s, [t(4), t(4), t(2), t(9), t(0)]);
    expect(fits[0]).toBeNull();
    expect(fits.slice(1).map((f) => f?.kind)).toEqual(['same', 'block', 'extra', 'empty']);
    expect(fits[2]).toEqual({ kind: 'block', rows: 2, main: 4 });
    expect(fits[3]).toEqual({ kind: 'extra', rows: 9, main: 4 });
  });

  it('renombrar una fuente reescribe las plantillas que la nombran, en textos, imágenes y códigos', () => {
    expect(renameInTemplate('{csv.productos:Sku} {Fila.Productos} {csv.otra:X} {csv:Y}', 'productos', 'items')).toBe('{csv.items:Sku} {Fila.items} {csv.otra:X} {csv:Y}');
    const d = doc([textObj('t', '{csv.productos:Producto}'), imageObj('i', '{csv.productos:Logo}'), codeObj('c', 'https://x/{csv.productos:Sku}')]);
    const r = renameSourceInDoc(d, 1, 'Items 2!');
    expect(r.variable_text?.sources[1]!.name).toBe('Items2');
    const [t, i, c] = r.objects as [DesignObject, VariableImageObject, VariableCodeObject];
    expect(isTextObject(t) && t.kind.text).toBe('{csv.Items2:Producto}');
    expect(i.kind.variable.source).toBe('{csv.Items2:Logo}');
    expect(c.kind.code.data).toBe('https://x/{csv.Items2:Sku}');
  });

  it('el nombre no puede chocar con el de otra fuente', () => {
    const r = renameSourceInDoc(doc([]), 1, 'PERSONAS');
    expect(r.variable_text?.sources[1]!.name).toBe('PERSONAS2');
  });
});

describe('vista previa por fila', () => {
  it('el lienzo ve la fila actual de todas las fuentes y cambia al ir a otra fila', () => {
    useStore.getState().replaceDocument(doc([textObj('t', '{csv:Nombre} - {csv.productos:Producto}')], { csv_row: 0 }), null);
    useVariablesUi.setState({
      csv: { path: '/datos/personas.csv', table: PEOPLE, error: null, loading: false },
      extra: { productos: { path: '/datos/productos.csv', table: PRODUCTS, error: null, loading: false } },
    });
    const show = () => resolveTemplate('{csv:Nombre} - {csv.productos:Producto}', displayContext(useStore.getState().doc)).text;
    expect(show()).toBe('Ana - Taza');
    editVariableText({ csv_row: 1 }, 'row');
    expect(show()).toBe('Luis - Llavero');
    editVariableText({ csv_row: 3 }, 'row');
    expect(show()).toBe('Iván - Llavero'); // la secundaria de 2 filas vuelve a empezar: 3 % 2 = 1
    editVariableText({ csv_row: 2 }, 'row');
    expect(show()).toBe('Eva - Taza');
  });

  it('una fuente secundaria aún sin leer no se confunde con la de otra ruta', () => {
    useStore.getState().replaceDocument(doc([]), null);
    useVariablesUi.setState({
      csv: { path: '/datos/personas.csv', table: PEOPLE, error: null, loading: false },
      extra: { productos: { path: '/otra/ruta.csv', table: PRODUCTS, error: null, loading: false } },
    });
    expect(resolveTemplate('{csv.productos:Producto}', displayContext(useStore.getState().doc)).issues[0]?.code).toBe('noCsv');
  });
});

describe('avance con imágenes y códigos', () => {
  it('un trabajo con copias de imagen y código gasta tantas filas como el mayor desplazamiento', () => {
    const d = doc([imageObj('i', '{csv:Logo@4}'), codeObj('c', '{csv.productos:Sku@2}'), textObj('t', '{csv:Nombre}')]);
    expect(jobSpans(d)).toEqual({ serial: 1, row: 5 });
    expect(advanceSettings(variableTextOf(d), jobSpans(d), 100).csv_row).toBe(5);
  });
});

describe('copias por fila con imágenes y códigos', () => {
  it('cada copia toma una fila distinta también en la ruta de la imagen y el contenido del código', () => {
    const d = doc([imageObj('i', '{csv.productos:Logo}'), codeObj('c', 'https://x/{csv:Nombre}'), textObj('t', '{csv:Nombre}')]);
    const r = copiesPerRow(d, ['i', 'c', 't'], { count: 3, cols: 3, gap: 5 });
    expect(r.created).toHaveLength(6);
    const images = r.doc.objects.filter((o): o is VariableImageObject => o.kind.type === 'image').map((o) => o.kind.variable.source);
    expect(images).toEqual(['{csv.productos:Logo}', '{csv.productos:Logo@1}', '{csv.productos:Logo@2}']);
    const codes = r.doc.objects.filter((o): o is VariableCodeObject => o.kind.type === 'vector').map((o) => o.kind.code.data);
    expect(codes).toEqual(['https://x/{csv:Nombre}', 'https://x/{csv:Nombre@1}', 'https://x/{csv:Nombre@2}']);
  });
});

describe('rutas de imagen', () => {
  it('une carpetas, resuelve .. y respeta Windows', () => {
    expect(joinPath('/datos/lote', 'logos/a.png')).toBe('/datos/lote/logos/a.png');
    expect(joinPath('/datos/lote', '../comun/./a.png')).toBe('/datos/comun/a.png');
    expect(joinPath('C:\\datos\\lote', 'logos\\a.png')).toBe('C:\\datos\\lote\\logos\\a.png');
    expect(joinPath('/', 'a.png')).toBe('/a.png');
  });

  it('una ruta absoluta se deja; una relativa parte de la carpeta del CSV; sin carpeta queda relativa', () => {
    expect(resolveImagePath('/abs/x.png', '/datos')).toBe('/abs/x.png');
    expect(resolveImagePath('C:\\x\\y.png', '/datos')).toBe('C:\\x\\y.png');
    expect(resolveImagePath('  "logos/x.png" ', '/datos')).toBe('/datos/logos/x.png');
    expect(resolveImagePath('x.png', null)).toBe('x.png');
    expect(resolveImagePath('   ', '/datos')).toBe('');
  });

  it('la carpeta base es la del CSV de la columna que usa la plantilla', () => {
    const c = ctxOf();
    expect(csvBaseDir('{csv:Nombre}', c)).toBe('/datos');
    expect(csvBaseDir('{csv.productos:Logo}', makeContext(vtOf({ sources: [SOURCES[0]!, src('productos', '/otro/lugar/p.csv')] }), PEOPLE, NOW, 'es', {}))).toBe('/otro/lugar');
    expect(csvBaseDir('fijo.png', c)).toBeNull();
  });
});

describe('encaje de la imagen variable en su caja', () => {
  const box = (extra: Partial<DesignObject> = {}) => imageObj('i', '{csv:Logo}', { transform: [1, 0, 0, 1, 100, 50], ...extra });

  it('«entera»: cabe dentro y queda centrada (imagen cuadrada en una caja de 20 x 10)', () => {
    const o = fitImageObject(box(), '/a.png', { width: 50, height: 50 }) as VariableImageObject;
    expect(o.kind.size_mm[0]).toBeCloseTo(10);
    expect(o.kind.size_mm[1]).toBeCloseTo(10);
    expect(o.transform[4]).toBeCloseTo(105); // (20 - 10) / 2 a la derecha
    expect(o.transform[5]).toBeCloseTo(50);
    expect(o.kind).toMatchObject({ source_path: '/a.png', width_px: 50, height_px: 50, mask: [] });
    expect(o.layer).toBe('#0000ff');
  });

  it('«entera» con una imagen más ancha: llena el ancho y se centra arriba y abajo', () => {
    const o = fitImageObject(box(), '/a.png', { width: 400, height: 100 }) as VariableImageObject;
    expect(o.kind.size_mm[0]).toBeCloseTo(20);
    expect(o.kind.size_mm[1]).toBeCloseTo(5);
    expect(o.transform[5]).toBeCloseTo(52.5);
  });

  it('«llenar»: cubre la caja y la máscara recorta lo que sobra', () => {
    const o = fitImageObject(box(), '/a.png', { width: 50, height: 50 }, 'cover') as VariableImageObject;
    expect(o.kind.size_mm[0]).toBeCloseTo(20);
    expect(o.kind.size_mm[1]).toBeCloseTo(20);
    expect(o.transform[5]).toBeCloseTo(45); // sobran 10 mm: 5 por arriba y 5 por abajo
    expect(o.kind.mask).toHaveLength(1);
    // La máscara es la caja original vista desde la imagen nueva: x de 0 a 20, y de 5 a 15.
    const pts = (o.kind.mask ?? [])[0]!.flatMap((seg) => ('to' in seg ? [seg.to] : []));
    const xs = pts.map((p) => p[0]);
    const ys = pts.map((p) => p[1]);
    expect(Math.min(...xs)).toBeCloseTo(0);
    expect(Math.max(...xs)).toBeCloseTo(20);
    expect(Math.min(...ys)).toBeCloseTo(5);
    expect(Math.max(...ys)).toBeCloseTo(15);
  });

  it('respeta la escala del transform (la caja se mide ya escalada) y el giro', () => {
    // Caja local 20 x 10 escalada x2: 40 x 20 mm en el documento.
    const o = fitImageObject(box({ transform: [2, 0, 0, 2, 0, 0] }), '/a.png', { width: 100, height: 100 }) as VariableImageObject;
    expect(o.kind.size_mm[0] * 2).toBeCloseTo(20);
    expect(o.kind.size_mm[1] * 2).toBeCloseTo(20);
    // Girada 90 grados: el desplazamiento al centro sigue el eje girado.
    const r = fitImageObject(box({ transform: [0, 1, -1, 0, 0, 0] }), '/a.png', { width: 50, height: 50 }) as VariableImageObject;
    expect(r.transform[4]).toBeCloseTo(0);
    expect(r.transform[5]).toBeCloseTo(5); // dx = 5 local → y' = b·dx
  });

  it('una caja o una imagen sin tamaño no revienta', () => {
    const o = fitImageObject(box(), '/a.png', { width: 0, height: 0 });
    expect(o.kind).toMatchObject({ source_path: '/a.png' });
  });
});

describe('colocación del código regenerado', () => {
  it('queda centrado donde estaba aunque el nuevo ancho sea otro', () => {
    const o = codeObj('c', '{csv:Nombre}', { transform: [1, 0, 0, 1, 100, 100] });
    const before = objectBounds(o)!;
    const cx = (before[0][0] + before[1][0]) / 2;
    const cy = (before[0][1] + before[1][1]) / 2;
    const result = mockGenerateCode({ kind: 'code128', data: 'ABCDEFGHIJ', ecc: 'm', size_mm: 60, height_mm: 10, margin_mm: 0, human_readable: false, invert: false, font_family: '' });
    const placed = placeCodeResult(o, result);
    const after = objectBounds(placed)!;
    expect((after[0][0] + after[1][0]) / 2).toBeCloseTo(cx);
    expect((after[0][1] + after[1][1]) / 2).toBeCloseTo(cy);
    expect(placed.kind).toMatchObject({ type: 'vector' });
    expect((placed.kind as VariableCodeObject['kind']).code.data).toBe('{csv:Nombre}');
  });
});

describe('resolución al enviar: varias fuentes, imágenes y códigos', () => {
  const probeFor = (missing: string[] = []) => async ({ paths }: { paths: string[] }): Promise<ImageProbe[]> =>
    paths.map((path) => (missing.includes(path) ? { path, ok: false, width_px: 0, height_px: 0, error: 'no existe' } : { path, ok: true, width_px: 200, height_px: 100, error: null }));

  const fake = (opts: { missing?: string[]; people?: CsvTable; products?: CsvTable } = {}) => {
    const reads: { path: string; required?: string[] }[] = [];
    const codes: CodeSpec[] = [];
    const probes: string[][] = [];
    const warnings: string[][] = [];
    const deps = {
      api: () => ({
        textOutline: async ({ text, path }: { text: TextObject; path: unknown }) => mockTextOutline(text, path as never),
        readCsv: async (a: { path: string; required?: string[] }) => {
          reads.push(a);
          return a.path.includes('productos') ? (opts.products ?? PRODUCTS) : (opts.people ?? PEOPLE);
        },
        probeImages: async (a: { paths: string[] }) => {
          probes.push(a.paths);
          return probeFor(opts.missing)(a);
        },
        generateCode: async ({ spec }: { spec: CodeSpec }) => {
          codes.push(spec);
          return mockGenerateCode(spec);
        },
      }),
      fonts: async () => [],
      now: () => NOW,
      warn: (m: string[]) => warnings.push(m),
    };
    return { deps, reads, codes, probes, warnings };
  };

  it('lee cada fuente que se usa con las columnas que le piden y resuelve cada texto con su fila', async () => {
    const f = fake();
    const d = doc([textObj('t', '{csv:Nombre} {csv.productos:Producto}')], { csv_row: 1 });
    const out = await resolveDocForOutput(d, f.deps);
    const t = out.objects[0]!;
    expect(t.kind.type === 'text' && t.kind.outline.length).toBeGreaterThan(0);
    expect(f.reads).toEqual([
      { path: '/datos/personas.csv', required: ['Nombre'] },
      { path: '/datos/productos.csv', required: ['Producto'] },
    ]);
  });

  it('no lee una fuente que ningún objeto usa', async () => {
    const f = fake();
    await resolveDocForOutput(doc([textObj('t', '{csv:Nombre}')]), f.deps);
    expect(f.reads.map((r) => r.path)).toEqual(['/datos/personas.csv']);
  });

  it('un CSV secundario que falla nombra la fuente', async () => {
    const f = fake();
    f.deps.api = () => ({
      ...fake().deps.api(),
      readCsv: async (a: { path: string }) => {
        if (a.path.includes('productos')) throw new Error('no se pudo abrir');
        return PEOPLE;
      },
    });
    await expect(resolveDocForOutput(doc([textObj('t', '{csv.productos:Producto}')]), f.deps)).rejects.toThrow(/«productos».*no se pudo abrir/);
  });

  it('reemplaza la imagen por el archivo de la fila: ruta relativa a la carpeta del CSV, absoluta tal cual', async () => {
    const f = fake();
    const d = doc([imageObj('a', '{csv.productos:Logo}'), imageObj('b', '{csv.productos:Logo@1}')], { csv_row: 0 });
    const out = await resolveDocForOutput(d, f.deps);
    const [a, b] = out.objects as VariableImageObject[];
    expect(a!.kind.source_path).toBe('/datos/logos/taza.png');
    expect(b!.kind.source_path).toBe('/abs/llavero.png');
    // Imagen de 200 x 100 px en una caja de 20 x 10: ocupa toda la caja.
    expect(a!.kind.size_mm[0]).toBeCloseTo(20);
    expect(a!.kind.width_px).toBe(200);
    expect(f.probes).toEqual([['/datos/logos/taza.png', '/abs/llavero.png']]);
    // El documento original conserva su imagen de muestra y su plantilla.
    expect((d.objects[0] as VariableImageObject).kind.source_path).toBe('/placeholder.png');
  });

  it('conserva la capa de la imagen (su modo de imagen) y el modo de ajuste elegido', async () => {
    const f = fake();
    const d = doc([imageObj('a', '{csv.productos:Logo}', { layer: '#00ff00' }, 'cover')]);
    const [a] = (await resolveDocForOutput(d, f.deps)).objects as VariableImageObject[];
    expect(a!.layer).toBe('#00ff00');
    expect(a!.kind.mask).toHaveLength(0); // 200x100 llena la caja 20x10 sin sobrante
    // Una imagen cuadrada en la caja de 20 x 10 sobra por arriba y por abajo: se recorta.
    const square = fake();
    square.deps.api = () => ({ ...fake().deps.api(), probeImages: async ({ paths }: { paths: string[] }) => paths.map((path) => ({ path, ok: true, width_px: 100, height_px: 100, error: null })) });
    const [b] = (await resolveDocForOutput(doc([imageObj('a', '{csv.productos:Logo}', {}, 'cover')]), square.deps)).objects as VariableImageObject[];
    expect(b!.kind.mask).toHaveLength(1);
  });

  it('un archivo que falta bloquea el envío y dice cuál, en qué fila y de qué plantilla', async () => {
    const f = fake({ missing: ['/datos/logos/taza.png'] });
    const d = doc([imageObj('a', '{csv.productos:Logo}')], { csv_row: 0 });
    const err = await resolveDocForOutput(d, f.deps).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(VariableError);
    const e = err as VariableError;
    expect(e.message).toContain('/datos/logos/taza.png');
    expect(e.message).toContain('fila 1');
    expect(e.issues[0]).toMatchObject({ code: 'imageMissing', row: 1 });
  });

  it('una ruta vacía en la celda también bloquea, con la fila', async () => {
    const f = fake({ products: { ...PRODUCTS, rows: [['Taza', '', 'T-1']] } });
    const err = (await resolveDocForOutput(doc([imageObj('a', '{csv.productos:Logo}')]), f.deps).catch((e: unknown) => e)) as VariableError;
    expect(err.issues[0]).toMatchObject({ code: 'imageEmpty', row: 1 });
  });

  it('junta todos los problemas de una vez (imagen que falta y código vacío)', async () => {
    const f = fake({ missing: ['/datos/logos/taza.png'], people: { ...PEOPLE, rows: [['', 'x']] } });
    const d = doc([imageObj('a', '{csv.productos:Logo}'), codeObj('c', '{csv:Nombre}'), textObj('t', '{csv:Nombre}')]);
    const err = (await resolveDocForOutput(d, f.deps).catch((e: unknown) => e)) as VariableError;
    expect(err.issues.map((i) => i.code).sort()).toEqual(['codeEmpty', 'imageMissing']);
  });

  it('regenera el código con el contenido de la fila y no toca el original', async () => {
    const f = fake();
    const d = doc([codeObj('c', 'https://tienda.example/{csv.productos:Sku}'), codeObj('c2', 'https://tienda.example/{csv.productos:Sku@1}')]);
    const out = await resolveDocForOutput(d, f.deps);
    expect(f.codes.map((c) => c.data)).toEqual(['https://tienda.example/T-1', 'https://tienda.example/L-2']);
    expect(f.codes[0]).toMatchObject({ kind: 'qr', ecc: 'm', size_mm: 30 });
    const [c, c2] = out.objects as VariableCodeObject[];
    expect(c!.kind.paths).not.toEqual(c2!.kind.paths);
    expect(c!.kind.code.data).toBe('https://tienda.example/{csv.productos:Sku}');
    expect((d.objects[0] as VariableCodeObject).kind.paths).toHaveLength(1);
  });

  it('el mismo contenido se genera una sola vez', async () => {
    const f = fake();
    await resolveDocForOutput(doc([codeObj('a', 'X-{csv:Nombre}'), codeObj('b', 'X-{csv:Nombre}')]), f.deps);
    expect(f.codes).toHaveLength(1);
  });

  it('el generador que rechaza el contenido bloquea con su mensaje y la fila', async () => {
    const f = fake();
    f.deps.api = () => ({
      ...fake().deps.api(),
      generateCode: async () => {
        throw new Error('EAN-13 necesita 12 dígitos');
      },
    });
    const err = (await resolveDocForOutput(doc([codeObj('a', '{csv:Nombre}')], { csv_row: 2 }), f.deps).catch((e: unknown) => e)) as VariableError;
    expect(err.issues[0]).toMatchObject({ code: 'codeFailed', row: 3, detail: 'EAN-13 necesita 12 dígitos' });
    expect(err.message).toContain('EAN-13');
  });

  it('un código con llaves sueltas (sin variables) no es variable y no se toca', async () => {
    const f = fake();
    const d = doc([codeObj('a', '{"id": 1}')]);
    expect(await resolveDocForOutput(d, f.deps)).toBe(d);
    expect(f.codes).toHaveLength(0);
  });

  it('avisa (sin bloquear) de las celdas vacías de los textos, con su fila', async () => {
    const f = fake({ people: { ...PEOPLE, rows: [['', 'x'], ['Luis', '']] } });
    const out = await resolveDocForOutput(doc([textObj('a', '{csv:Nombre}'), textObj('b', '{csv:Cargo@1}')]), f.deps);
    expect(out.objects).toHaveLength(2);
    expect(f.warnings).toHaveLength(1);
    expect(f.warnings[0]![0]).toContain('{csv:Nombre} (fila 1)');
    expect(f.warnings[0]![0]).toContain('{csv:Cargo@1} (fila 2)');
  });

  it('una cuadrícula «una copia por fila» con imagen y código resuelve una fila por copia', async () => {
    const f = fake();
    const d = doc([imageObj('i', '{csv.productos:Logo}'), codeObj('c', '{csv.productos:Sku}')]);
    const grid = copiesPerRow(d, ['i', 'c'], { count: 2, cols: 2, gap: 4 }).doc;
    const out = await resolveDocForOutput(grid, f.deps);
    const imgs = out.objects.filter((o): o is VariableImageObject => o.kind.type === 'image').map((o) => o.kind.source_path);
    expect(imgs).toEqual(['/datos/logos/taza.png', '/abs/llavero.png']);
    expect(f.codes.map((c) => c.data)).toEqual(['T-1', 'L-2']);
  });

  it('sin nada variable devuelve el mismo documento sin pedir nada', async () => {
    const f = fake();
    const d: Document = { version: 2, objects: [textObj('t', 'Fijo')], layers: [], device_id: null };
    expect(await resolveDocForOutput(d, f.deps)).toBe(d);
    expect(f.reads).toHaveLength(0);
  });
});

describe('revisar datos antes de enviar', () => {
  const probe = async ({ paths }: { paths: string[] }): Promise<ImageProbe[]> =>
    paths.map((path) => ({ path, ok: !path.includes('falta'), width_px: 10, height_px: 10, error: null }));

  it('lista las filas con celdas vacías, imágenes que no existen y códigos sin contenido', async () => {
    const people: CsvTable = {
      headers: ['Nombre', 'Logo', 'Id'],
      rows: [['Ana', 'a.png', '1'], ['', 'b.png', '2'], ['Eva', 'falta.png', ''], ['Iván', '', '4']],
      delimiter: ',',
    };
    const d = doc([textObj('t', '{csv:Nombre}'), imageObj('i', '{csv:Logo}'), codeObj('c', 'https://x/{csv:Id}')], { sources: [SOURCES[0]!] });
    const r = await checkData(d, { main: people, extra: {} }, { probeImages: probe }, NOW);
    expect(r.rows).toBe(4);
    expect(r.general).toEqual([]);
    expect(r.problems.map((p) => p.row)).toEqual([2, 3, 4]);
    expect(r.problems[0]!.messages.join(' ')).toContain('Nombre');
    expect(r.problems[1]!.messages.join(' ')).toContain('falta.png');
    expect(r.problems[2]!.messages.join(' ')).toContain('Logo');
    expect(r.problemRows).toBe(3);
  });

  it('una columna que no existe se avisa una sola vez, no en cada fila', async () => {
    const d = doc([textObj('t', '{csv:Edad}')], { sources: [SOURCES[0]!] });
    const r = await checkData(d, { main: PEOPLE, extra: {} }, { probeImages: probe }, NOW);
    expect(r.problemRows).toBe(0);
    expect(r.general).toHaveLength(1);
    expect(r.general[0]).toContain('Edad');
  });

  it('revisa una fuente secundaria que se repite sin falsos avisos de «se acabó»', async () => {
    const d = doc([textObj('t', '{csv:Nombre} {csv.productos:Producto}')]);
    const r = await checkData(d, { main: PEOPLE, extra: { productos: PRODUCTS } }, { probeImages: probe }, NOW);
    expect(r.problemRows).toBe(0);
    expect(r.general).toEqual([]);
  });

  it('con «bloquear», las filas que pasan de la fuente corta se listan', async () => {
    const d = doc([textObj('t', '{csv.productos:Producto}')], { sources: [SOURCES[0]!, src('productos', '/datos/productos.csv', 'block')] });
    const r = await checkData(d, { main: PEOPLE, extra: { productos: PRODUCTS } }, { probeImages: probe }, NOW);
    expect(r.problems.map((p) => p.row)).toEqual([3, 4]);
  });

  it('una cuadrícula con desplazamientos @k no multiplica los avisos', async () => {
    const people: CsvTable = { headers: ['Nombre'], rows: [[''], ['x'], ['y']], delimiter: ',' };
    const d = doc([textObj('a', '{csv:Nombre}'), textObj('b', '{csv:Nombre@1}'), textObj('c', '{csv:Nombre@2}')], { sources: [SOURCES[0]!] });
    const r = await checkData(d, { main: people, extra: {} }, { probeImages: probe }, NOW);
    expect(r.problems).toHaveLength(1);
    expect(r.problems[0]!.messages).toHaveLength(1);
  });

  it('sin CSV leído o sin objetos variables no revisa nada', async () => {
    const empty = await checkData(doc([textObj('t', 'Fijo')]), { main: PEOPLE, extra: {} }, { probeImages: probe }, NOW);
    expect(empty).toEqual({ rows: 0, problems: [], problemRows: 0, general: [] });
    const unread = await checkData(doc([textObj('t', '{csv:Nombre}')]), { main: null, extra: {} }, { probeImages: probe }, NOW);
    expect(unread.rows).toBe(0);
  });
});
