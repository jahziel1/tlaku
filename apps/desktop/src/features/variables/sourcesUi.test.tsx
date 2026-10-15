// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { CsvTable, DesignObject, Document } from '@/api/types';
import { setLanguage } from '@/i18n';
import { useStore } from '@/store';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { EMPTY_CSV, useVariablesUi } from './context';
import { DEFAULT_VARIABLE_TEXT } from './defaults';
import { ImageVariableField } from './ImageVariableField';
import { SourcesPanel } from './SourcesPanel';
import { VariablesContent } from './VariablesDialog';

const PEOPLE: CsvTable = { headers: ['Nombre', 'Logo'], rows: [['Ana', 'a.png'], ['Luis', 'b.png'], ['Eva', 'c.png']], delimiter: ',' };
const ITEMS: CsvTable = { headers: ['Producto'], rows: [['Taza'], ['Llavero']], delimiter: ',' };

const image = (variable: boolean): DesignObject => ({
  id: 'img', layer: '#0000ff', transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null,
  kind: { type: 'image', source_path: '/x.png', width_px: 10, height_px: 10, size_mm: [20, 10], mask: [], ...(variable ? { variable: { source: '{csv:Logo}', fit: 'contain' as const } } : {}) },
});

function setup(objects: DesignObject[] = []): void {
  const doc: Document = {
    version: 2, objects, layers: [], device_id: null,
    variable_text: {
      ...DEFAULT_VARIABLE_TEXT,
      csv_path: '/d/personas.csv',
      sources: [
        { name: 'personas', path: '/d/personas.csv', on_short: 'repeat' },
        { name: 'productos', path: '/d/productos.csv', on_short: 'repeat' },
      ],
    },
  };
  useStore.getState().replaceDocument(doc, null);
  useVariablesUi.setState({
    csv: { path: '/d/personas.csv', table: PEOPLE, error: null, loading: false },
    extra: { productos: { path: '/d/productos.csv', table: ITEMS, error: null, loading: false } },
  });
}

beforeEach(() => setLanguage('es'));
afterEach(async () => {
  await cleanup();
  useVariablesUi.setState({ csv: EMPTY_CSV, extra: {} });
});

describe('panel de fuentes de datos', () => {
  it('lista cada fuente con su nombre, su archivo y el aviso de filas distintas', async () => {
    setup();
    const m = await mount(<SourcesPanel />);
    const items = m.host.querySelectorAll('.vt-source');
    expect(items).toHaveLength(2);
    expect(items[0]!.textContent).toContain('personas.csv');
    expect(items[1]!.textContent).toContain('productos.csv');
    // La secundaria tiene 2 filas y la principal 3: se repite.
    expect(items[1]!.textContent).toContain('se repite desde el inicio');
    expect(items[1]!.textContent).toContain('{csv.productos:…}');
    // Solo las secundarias tienen «si tiene menos filas» y «hacer principal».
    expect(items[0]!.querySelector('select')).toBeNull();
    expect(items[1]!.querySelector('select')).not.toBeNull();
    expect(describeViolations(await audit(m.host))).toEqual([]);
  });

  it('quitar la secundaria la saca del documento y la principal manda sobre la fila actual', async () => {
    setup();
    const m = await mount(<SourcesPanel />);
    const remove = m.host.querySelectorAll('.vt-source')[1]!.querySelector('button[aria-label="Quitar esta fuente"]')!;
    await click(remove);
    const vt = useStore.getState().doc.variable_text!;
    expect(vt.sources.map((s) => s.name)).toEqual(['personas']);
  });

  it('hacer principal cambia el CSV principal y vuelve a la primera fila', async () => {
    setup();
    useStore.getState().replaceDocument({ ...useStore.getState().doc, variable_text: { ...useStore.getState().doc.variable_text!, csv_row: 2 } }, null);
    const m = await mount(<SourcesPanel />);
    const btn = m.host.querySelectorAll('.vt-source')[1]!.querySelector('button[aria-label="Hacer principal"]')!;
    await click(btn);
    const vt = useStore.getState().doc.variable_text!;
    expect(vt.sources.map((s) => s.name)).toEqual(['productos', 'personas']);
    expect(vt.csv_path).toBe('/d/productos.csv');
    expect(vt.csv_row).toBe(0);
  });

  it('en inglés usa textos propios', async () => {
    setLanguage('en');
    setup();
    const m = await mount(<SourcesPanel />);
    expect(m.host.textContent).toContain('Data sources (CSV)');
    expect(m.host.textContent).toContain('repeats from the start');
  });
});

describe('diálogo de texto variable con varias fuentes', () => {
  it('muestra vista previa por fila, selector de fuente y revisión de datos sin violaciones de accesibilidad', async () => {
    setup([
      { id: 't', layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null,
        kind: { type: 'text', text: '{csv:Nombre} {csv.productos:Producto}', font_family: 'Arial', bold: false, italic: false, size_mm: 10, letter_spacing_mm: 0, line_spacing: 1.2, align: 'left', on_path: null, outline: [] } },
      image(true),
    ]);
    const m = await mount(<VariablesContent titleId="t" />);
    expect(m.host.textContent).toContain('Vista previa por fila');
    expect(m.host.textContent).toContain('Fila 1 de 3');
    // El texto se ve con la fila actual de las dos fuentes.
    expect(m.host.textContent).toContain('Ana Taza');
    // La imagen variable aparece en la lista con su ruta resuelta.
    expect(m.host.textContent).toContain('a.png');
    expect(m.host.querySelector('.vt-viewsel select')).not.toBeNull();
    expect(describeViolations(await audit(m.host))).toEqual([]);
    // Siguiente fila.
    const next = [...m.host.querySelectorAll('button')].find((b) => b.textContent === 'Fila siguiente')!;
    await click(next);
    expect(useStore.getState().doc.variable_text?.csv_row).toBe(1);
    expect(m.host.textContent).toContain('Luis Llavero');
  });
});

describe('propiedades de la imagen variable', () => {
  it('no se dibuja sin una imagen seleccionada', async () => {
    setup([image(false)]);
    useStore.getState().select([]);
    const m = await mount(<ImageVariableField />);
    expect(m.host.innerHTML).toBe('');
  });

  it('marcar la imagen como variable propone la columna de imágenes y desmarcar la deja fija', async () => {
    setup([image(false)]);
    useStore.getState().select(['img']);
    const m = await mount(<ImageVariableField />);
    const check = m.host.querySelector('input[type="checkbox"]') as HTMLInputElement;
    expect(check.checked).toBe(false);
    await click(check);
    const o = useStore.getState().doc.objects[0]!;
    expect(o.kind.type === 'image' && o.kind.variable).toEqual({ source: '{csv:Logo}', fit: 'contain' });
    expect(m.host.textContent).toContain('Ruta del archivo');
    expect(describeViolations(await audit(m.host))).toEqual([]);
    await click(m.host.querySelector('input[type="checkbox"]')!);
    const back = useStore.getState().doc.objects[0]!;
    expect(back.kind.type === 'image' && back.kind.variable).toBeUndefined();
    // Un solo paso de deshacer vuelve a marcarla.
    useStore.getState().undo();
    const again = useStore.getState().doc.objects[0]!;
    expect(again.kind.type === 'image' && again.kind.variable).toBeTruthy();
  });

  it('elegir una columna de otra fuente escribe {csv.fuente:Columna}', async () => {
    setup([image(true)]);
    useStore.getState().select(['img']);
    const m = await mount(<ImageVariableField />);
    const selects = m.host.querySelectorAll('select');
    const col = selects[0] as HTMLSelectElement;
    const opt = [...col.options].find((o) => o.textContent?.includes('{csv.productos:Producto}'))!;
    await import('react').then(({ act }) =>
      act(async () => {
        const setter = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value')!.set!;
        setter.call(col, opt.value);
        col.dispatchEvent(new Event('change', { bubbles: true }));
      }),
    );
    const o = useStore.getState().doc.objects[0]!;
    expect(o.kind.type === 'image' && o.kind.variable?.source).toBe('{csv.productos:Producto}');
  });
});
