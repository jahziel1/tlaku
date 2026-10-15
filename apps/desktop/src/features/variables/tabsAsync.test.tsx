// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getApi, setApi } from '@/api';
import type { LaserApi } from '@/api';
import type { CsvTable, DataSource, DesignObject, Document, ImageProbe, TextObject } from '@/api/types';
import { setLanguage } from '@/i18n';
import { useStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { resetTabs } from '@/test/tabs';
import { advanceAfterJob } from './advance';
import { canvasDisplayObjects, resetCanvasDisplay } from './canvasDisplay';
import { displayContext, EMPTY_CSV, useVariablesUi } from './context';
import { startCsvSync } from './csvSync';
import { DataCheck } from './DataCheck';
import { DEFAULT_VARIABLE_TEXT, variableTextOf } from './defaults';
import { resolveTemplate } from './resolve';
import { SourcesPanel } from './SourcesPanel';
import type { VariableImageObject } from './varObjects';

/**
 * Las funciones de datos variables conviven con las pestañas de documentos: cada documento tiene sus
 * propias fuentes CSV, y nada de lo asíncrono (leer un CSV, sondear imágenes, elegir un archivo,
 * «Revisar datos») puede acabar en otra pestaña si el usuario se va a mitad.
 */

// El selector de archivos real es un diálogo del sistema: aquí lo controla la prueba.
const picker = vi.hoisted(() => ({ resolve: null as null | ((path: string | null) => void) }));
vi.mock('./csvSync', async (original) => ({
  ...(await original<Record<string, unknown>>()),
  pickCsvFile: () => new Promise<string | null>((resolve) => void (picker.resolve = resolve)),
}));

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (v: T) => void;
  reject: (e: unknown) => void;
}
function deferred<T>(): Deferred<T> {
  const d = {} as Deferred<T>;
  d.promise = new Promise<T>((resolve, reject) => {
    d.resolve = resolve;
    d.reject = reject;
  });
  return d;
}
/** Deja que terminen las promesas ya resueltas. */
const flush = () => act(async () => void (await new Promise((r) => setTimeout(r, 0))));

const s = () => useStore.getState();
const table = (headers: string[], rows: string[][]): CsvTable => ({ headers, rows, delimiter: ',' });
const NOMBRES_A = table(['Nombre', 'Logo'], [['Ana', 'a/uno.png'], ['Luis', 'a/dos.png'], ['Eva', 'a/tres.png']]);
const NOMBRES_B = table(['Nombre', 'Logo'], [['Zoe', 'b/uno.png'], ['Max', 'b/dos.png']]);
const PRECIOS_A = table(['Precio'], [['10'], ['20']]);
const PRECIOS_B = table(['Precio'], [['99']]);

const src = (name: string, path: string): DataSource => ({ name, path, on_short: 'repeat' });
const textKind = (text: string): TextObject => ({
  text, font_family: 'Arial', bold: false, italic: false, size_mm: 10, letter_spacing_mm: 0,
  line_spacing: 1.2, align: 'left', on_path: null, outline: [],
});
const obj = (id: string, kind: DesignObject['kind']): DesignObject => ({
  id, layer: '#0000ff', transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null, kind,
});
const textObj = (id: string, text: string) => obj(id, { type: 'text', ...textKind(text) });
const imageObj = (id: string, template: string) =>
  obj(id, { type: 'image', source_path: '/muestra.png', width_px: 100, height_px: 100, size_mm: [20, 10], mask: [], variable: { source: template, fit: 'contain' } }) as VariableImageObject;

function docOf(sources: DataSource[], objects: DesignObject[] = [], row = 0): Document {
  return { ...emptyDocument(), objects, variable_text: { ...DEFAULT_VARIABLE_TEXT, sources, csv_path: sources[0]?.path ?? null, csv_row: row } };
}

let original: LaserApi;
let stopSync: (() => void) | undefined;
/** Lecturas pendientes por ruta: la prueba decide cuándo llega cada una. */
let reads: Map<string, Deferred<CsvTable>>;
let readCsv: ReturnType<typeof vi.fn>;

beforeEach(() => {
  setLanguage('es');
  original = getApi();
  reads = new Map();
  readCsv = vi.fn(({ path }: { path: string }) => {
    const d = deferred<CsvTable>();
    reads.set(path, d);
    return d.promise;
  });
  setApi({ ...original, readCsv } as unknown as LaserApi);
  resetTabs();
  useVariablesUi.setState({ csv: EMPTY_CSV, extra: {}, showTemplate: false });
  resetCanvasDisplay();
  picker.resolve = null;
});
afterEach(async () => {
  stopSync?.();
  stopSync = undefined;
  await cleanup();
  setApi(original);
  resetTabs();
  useVariablesUi.setState({ csv: EMPTY_CSV, extra: {} });
});

/** Abre `a` y luego `b` (queda activa `b`). Devuelve los ids de pestaña. */
function openTwo(a: Document, b: Document): { a: string; b: string } {
  s().openInTab(a, '/p/a.tlaku');
  const ida = s().activeTabId;
  s().openInTab(b, '/p/b.tlaku');
  return { a: ida, b: s().activeTabId };
}

describe('tablas CSV por pestaña', () => {
  it('cada pestaña conserva su tabla y su fila; al volver no se vuelve a leer nada', async () => {
    stopSync = startCsvSync();
    const { a, b } = openTwo(docOf([src('datos', '/d/a.csv')], [textObj('t', '{csv:Nombre}')], 2), docOf([src('datos', '/d/b.csv')], [textObj('t', '{csv:Nombre}')], 1));
    await flush();
    reads.get('/d/a.csv')!.resolve(NOMBRES_A);
    reads.get('/d/b.csv')!.resolve(NOMBRES_B);
    await flush();
    expect(readCsv).toHaveBeenCalledTimes(2);

    // Pestaña B (activa): su tabla y su fila.
    expect(useVariablesUi.getState().csv).toMatchObject({ path: '/d/b.csv', table: NOMBRES_B, loading: false });
    expect(resolveTemplate('{csv:Nombre}', displayContext(s().doc)).text).toBe('Max');

    s().activateTab(a);
    expect(useVariablesUi.getState().csv).toMatchObject({ path: '/d/a.csv', table: NOMBRES_A });
    expect(variableTextOf(s().doc).csv_row).toBe(2);
    expect(resolveTemplate('{csv:Nombre}', displayContext(s().doc)).text).toBe('Eva');

    s().activateTab(b);
    expect(resolveTemplate('{csv:Nombre}', displayContext(s().doc)).text).toBe('Max');
    expect(readCsv).toHaveBeenCalledTimes(2); // la tabla viaja con la pestaña
  });

  it('una lectura que termina con otra pestaña activa se guarda en la suya y no toca a la activa', async () => {
    stopSync = startCsvSync();
    const { a, b } = openTwo(docOf([src('datos', '/d/a.csv')]), docOf([src('datos', '/d/b.csv')]));
    await flush();
    // Llega la de A mientras B está activa (y aún cargando la suya).
    reads.get('/d/a.csv')!.resolve(NOMBRES_A);
    await flush();
    expect(useVariablesUi.getState().csv).toMatchObject({ path: '/d/b.csv', table: null, loading: true });
    reads.get('/d/b.csv')!.resolve(NOMBRES_B);
    await flush();
    expect(useVariablesUi.getState().csv.table).toBe(NOMBRES_B);

    s().activateTab(a);
    expect(useVariablesUi.getState().csv).toMatchObject({ path: '/d/a.csv', table: NOMBRES_A, loading: false });
    expect(s().activeTabId).toBe(a);
    expect(readCsv).toHaveBeenCalledTimes(2);
    void b;
  });

  it('un error de lectura de otra pestaña no aparece en la activa', async () => {
    stopSync = startCsvSync();
    const { a } = openTwo(docOf([src('datos', '/d/a.csv')]), docOf([src('datos', '/d/b.csv')]));
    await flush();
    reads.get('/d/b.csv')!.resolve(NOMBRES_B);
    reads.get('/d/a.csv')!.reject(new Error('no existe a.csv'));
    await flush();
    expect(useVariablesUi.getState().csv).toMatchObject({ path: '/d/b.csv', error: null, table: NOMBRES_B });
    s().activateTab(a);
    expect(useVariablesUi.getState().csv).toMatchObject({ path: '/d/a.csv', error: 'no existe a.csv', table: null, loading: false });
  });

  it('si la pestaña se cierra durante la lectura, el resultado se descarta', async () => {
    stopSync = startCsvSync();
    const { a } = openTwo(docOf([src('datos', '/d/a.csv')]), docOf([src('datos', '/d/b.csv')]));
    await flush();
    s().removeTab(a);
    reads.get('/d/a.csv')!.resolve(NOMBRES_A);
    reads.get('/d/b.csv')!.resolve(NOMBRES_B);
    await flush();
    expect(useVariablesUi.getState().csv).toMatchObject({ path: '/d/b.csv', table: NOMBRES_B });
    expect(s().tabs).toHaveLength(1);
  });

  it('las fuentes secundarias del mismo nombre son de cada pestaña', async () => {
    stopSync = startCsvSync();
    const docA = docOf([src('personas', '/d/a.csv'), src('precios', '/d/pa.csv')]);
    const docB = docOf([src('personas', '/d/b.csv'), src('precios', '/d/pb.csv')]);
    const { a } = openTwo(docA, docB);
    await flush();
    // Lecturas de A y B, en orden cruzado, mientras B está activa.
    reads.get('/d/pa.csv')!.resolve(PRECIOS_A);
    reads.get('/d/a.csv')!.resolve(NOMBRES_A);
    await flush();
    expect(useVariablesUi.getState().extra.precios).toMatchObject({ path: '/d/pb.csv', table: null, loading: true });
    reads.get('/d/pb.csv')!.resolve(PRECIOS_B);
    reads.get('/d/b.csv')!.resolve(NOMBRES_B);
    await flush();
    expect(useVariablesUi.getState().extra.precios!.table).toBe(PRECIOS_B);

    s().activateTab(a);
    expect(useVariablesUi.getState().extra.precios).toMatchObject({ path: '/d/pa.csv', table: PRECIOS_A });
    expect(resolveTemplate('{csv.precios:Precio}', displayContext(s().doc)).text).toBe('10');
    expect(readCsv).toHaveBeenCalledTimes(4);
  });

  it('una pestaña nueva empieza sin tablas, aunque la anterior las tuviera', async () => {
    stopSync = startCsvSync();
    s().openInTab(docOf([src('datos', '/d/a.csv')]), '/p/a.tlaku');
    await flush();
    reads.get('/d/a.csv')!.resolve(NOMBRES_A);
    await flush();
    s().newTab();
    expect(useVariablesUi.getState().csv).toEqual(EMPTY_CSV);
    expect(useVariablesUi.getState().extra).toEqual({});
  });

  it('duplicar la pestaña conserva las tablas leídas (mismas fuentes) sin releerlas', async () => {
    stopSync = startCsvSync();
    s().openInTab(docOf([src('datos', '/d/a.csv')]), '/p/a.tlaku');
    const a = s().activeTabId;
    await flush();
    reads.get('/d/a.csv')!.resolve(NOMBRES_A);
    await flush();
    const copy = s().duplicateTab(a)!;
    s().activateTab(copy);
    expect(useVariablesUi.getState().csv.table).toBe(NOMBRES_A);
    expect(readCsv).toHaveBeenCalledTimes(1);
  });
});

describe('avance de fila al terminar un trabajo', () => {
  it('usa las filas del CSV de la pestaña de origen aunque haya otra activa', async () => {
    stopSync = startCsvSync();
    // Una copia por fila (@4): el trabajo gasta 5 filas. A tiene 3 filas y B, 2: el tope es el de A.
    const label = textObj('t', '{csv:Nombre@4}');
    const { a } = openTwo(
      docOf([src('datos', '/d/a.csv')], [label], 0),
      docOf([src('datos', '/d/b.csv')], [label], 0),
    );
    await flush();
    reads.get('/d/a.csv')!.resolve(NOMBRES_A);
    reads.get('/d/b.csv')!.resolve(NOMBRES_B);
    await flush();
    const notices = s().notices.length;

    // Como al terminar el trabajo: se aplica en la pestaña de origen (A), con B activa.
    s().runInTab(a, () => advanceAfterJob(useStore));
    expect(s().notices.length).toBe(notices + 1); // el aviso de «se acabó el CSV» (al llegar al tope de A)
    s().activateTab(a);
    expect(variableTextOf(s().doc).csv_row).toBe(3); // tope: las 3 filas de A (con las 2 de B habría sido 2)
    s().activateTab(s().tabs.find((e) => e.id !== a)!.id);
    expect(variableTextOf(s().doc).csv_row).toBe(0);
  });
});

describe('elegir un archivo CSV', () => {
  it('el archivo elegido se agrega a la pestaña donde se abrió el selector', async () => {
    stopSync = startCsvSync();
    s().openInTab(docOf([]), '/p/a.tlaku');
    const a = s().activeTabId;
    s().openInTab(docOf([]), '/p/b.tlaku');
    const b = s().activeTabId;
    s().activateTab(a);
    const m = await mount(<SourcesPanel />);
    await click([...m.host.querySelectorAll('button')].find((x) => /Elegir/.test(x.textContent ?? ''))!);
    expect(picker.resolve).not.toBeNull();

    act(() => s().activateTab(b)); // el usuario se va a B con el selector abierto
    await act(async () => picker.resolve!('/d/elegido.csv'));
    await flush();

    expect(s().activeTabId).toBe(b);
    expect(variableTextOf(s().doc).sources).toEqual([]); // B no recibió nada
    expect(s().dirty).toBe(false);
    s().activateTab(a);
    expect(variableTextOf(s().doc).sources.map((x) => x.path)).toEqual(['/d/elegido.csv']);
    expect(s().dirty).toBe(true);
    expect(describeViolations(await audit(m.host))).toEqual([]);
  });

  it('si la pestaña se cerró durante el selector, avisa y no escribe en otra', async () => {
    s().openInTab(docOf([]), '/p/a.tlaku');
    const a = s().activeTabId;
    s().openInTab(docOf([]), '/p/b.tlaku');
    s().activateTab(a);
    const m = await mount(<SourcesPanel />);
    await click([...m.host.querySelectorAll('button')].find((x) => /Elegir/.test(x.textContent ?? ''))!);
    const other = s().tabs.find((e) => e.id !== a)!.id;
    act(() => s().activateTab(other));
    act(() => void s().removeTab(a));
    await act(async () => picker.resolve!('/d/elegido.csv'));
    await flush();
    expect(variableTextOf(s().doc).sources).toEqual([]);
    expect(s().notices.at(-1)?.level).toBe('warning');
  });
});

describe('«Revisar datos»', () => {
  const LOGOS = (state: 'a' | 'b') => docOf([src('datos', `/d/${state}.csv`)], [imageObj('img', '{csv:Logo}')]);

  it('el resultado es de la pestaña donde se pidió y no se muestra en otra', async () => {
    const probes = deferred<ImageProbe[]>();
    const probeImages = vi.fn(() => probes.promise);
    setApi({ ...original, readCsv, probeImages } as unknown as LaserApi);
    const { a, b } = openTwo(LOGOS('a'), LOGOS('b'));
    s().activateTab(a);
    useVariablesUi.setState({ csv: { path: '/d/a.csv', table: NOMBRES_A, error: null, loading: false }, extra: {} });
    const m = await mount(<DataCheck onPickRow={() => undefined} />);
    const run = [...m.host.querySelectorAll('button')].find((x) => x.textContent === 'Revisar todas las filas')!;
    expect(run.disabled).toBe(false);
    await click(run);
    expect(probeImages).toHaveBeenCalledTimes(1);

    act(() => s().activateTab(b)); // se va a mitad de la revisión
    useVariablesUi.setState({ csv: { path: '/d/b.csv', table: NOMBRES_B, error: null, loading: false } });
    await act(async () => probes.resolve(['a/uno.png', 'a/dos.png', 'a/tres.png'].map((path) => ({ path, ok: false, width_px: 0, height_px: 0, error: 'no existe' }))));
    await flush();
    // En B no hay resultado ni «revisando».
    expect(m.host.querySelector('.vt-check-list')).toBeNull();
    expect(m.host.textContent).not.toMatch(/Revisando/);

    act(() => s().activateTab(a));
    expect(m.host.querySelector('.vt-check-list')).not.toBeNull();
  });
});

describe('lienzo: imágenes variables de cada pestaña', () => {
  it('con la sonda en vuelo al cambiar de pestaña, cada una ve su propia imagen', async () => {
    const pending = new Map<string, Deferred<ImageProbe[]>>();
    const probeImages = vi.fn(({ paths }: { paths: string[] }) => {
      const d = deferred<ImageProbe[]>();
      pending.set(paths[0]!, d);
      return d.promise;
    });
    const probe = (suffix: string) => [...pending.entries()].find(([p]) => p.endsWith(suffix))!;
    setApi({ ...original, probeImages } as unknown as LaserApi);
    const ok = (path: string, w: number): ImageProbe[] => [{ path, ok: true, width_px: w, height_px: 100, error: null }];
    const imgDoc = (name: 'a' | 'b') => docOf([src('datos', `/d/${name}.csv`)], [imageObj('img', '{csv:Logo}')]);

    s().openInTab(imgDoc('a'), '/p/a.tlaku');
    const a = s().activeTabId;
    useVariablesUi.setState({ csv: { path: '/d/a.csv', table: NOMBRES_A, error: null, loading: false } });
    const shownA = () => (canvasDisplayObjects(s().doc, s().doc.objects)[0] as VariableImageObject).kind.source_path;
    expect(shownA()).toBe('/muestra.png'); // sonda pendiente: se ve la muestra

    s().openInTab(imgDoc('b'), '/p/b.tlaku');
    useVariablesUi.setState({ csv: { path: '/d/b.csv', table: NOMBRES_B, error: null, loading: false } });
    expect(shownA()).toBe('/muestra.png');

    // Llega la sonda de A con B activa: B sigue viendo su muestra, no la imagen de A.
    const [pathA, probeA] = probe('a/uno.png');
    probeA.resolve(ok(pathA, 200));
    await flush();
    expect(shownA()).toBe('/muestra.png');
    const [pathB, probeB] = probe('b/uno.png');
    probeB.resolve(ok(pathB, 400));
    await flush();
    expect(shownA()).toBe(pathB);

    s().activateTab(a);
    expect(shownA()).toBe(pathA);
    expect(pathA).not.toBe(pathB);
  });
});
