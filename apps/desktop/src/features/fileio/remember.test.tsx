import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getApi, setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { importPaths } from '@/features/history/files';
import { DEFAULT_REMEMBERED_IMPORT } from '@/lib/importOptions';
import { useStore } from '@/store';
import { importWithOptions, type FileioIo } from './actions';
import { ImportOptionsDialog, kindsOf } from './ImportOptionsDialog';
import { initialImportOptions, rememberedFrom } from './remember';
import { DEFAULT_IMPORT_OPTIONS } from './types';

const s = () => useStore.getState();

function io(over: Partial<FileioIo> = {}): FileioIo {
  return {
    askExport: async () => null,
    askImport: async () => DEFAULT_IMPORT_OPTIONS,
    pickSavePath: async () => null,
    pickImportPaths: async () => ['/tmp/logo.svg'],
    ...over,
  };
}

beforeEach(() => {
  setApi(createMockApi({ latencyMs: 0 }));
  s().newDocument();
  s().resetPrefs();
});

describe('opciones de importación recordadas', () => {
  it('sin nada recordado el diálogo arranca con las de fábrica', () => {
    expect(initialImportOptions(null, ['#000000'])).toBe(DEFAULT_IMPORT_OPTIONS);
  });

  it('recuerda la resolución, la unidad del DXF, el agrupado y la capa; no la escala ni la página', () => {
    const picked = { ...DEFAULT_IMPORT_OPTIONS, scale_percent: 250, pdf_page: 4, dpi: 150, dxf_unit_mm: 25.4, group: true, target_layer: '#ff0000', gcode_by_power: false };
    const r = rememberedFrom(picked);
    expect(r).toEqual({ dpi: 150, dxf_unit_mm: 25.4, group: true, auto_group: false, target_layer: '#ff0000', gcode_by_power: false });
    const next = initialImportOptions(r, ['#000000', '#FF0000']);
    expect(next).toMatchObject({ scale_percent: 100, pdf_page: 1, dpi: 150, dxf_unit_mm: 25.4, group: true, gcode_by_power: false });
    expect(next.target_layer).toBe('#FF0000'); // con el color tal como lo tiene el documento
  });

  it('si la capa recordada ya no existe en el documento, vuelve a «una capa por color»', () => {
    expect(initialImportOptions({ ...DEFAULT_REMEMBERED_IMPORT, target_layer: '#123456' }, ['#000000']).target_layer).toBeNull();
  });

  it('el diálogo muestra la resolución recordada y la predeterminada de Preferencias', () => {
    const html = renderToStaticMarkup(
      <ImportOptionsDialog fileCount={1} kinds={kindsOf(['svg'])} layers={[]} initial={{ ...DEFAULT_IMPORT_OPTIONS, dpi: 600 }} defaultDpi={120} onDone={() => {}} />,
    );
    expect(html).toContain('Predeterminada (120)');
    expect(html).toContain('>600<');
  });
});

describe('resolución por omisión al importar', () => {
  it('con «predeterminada» en el diálogo se manda la resolución de Preferencias', async () => {
    s().setPref('importDpi', 150);
    const spy = vi.spyOn(getApi(), 'importFileWith');
    await importWithOptions(io());
    expect(spy.mock.calls[0]![0].options.dpi).toBe(150);
  });

  it('una resolución elegida en el diálogo manda sobre la de Preferencias', async () => {
    s().setPref('importDpi', 150);
    const spy = vi.spyOn(getApi(), 'importFileWith');
    await importWithOptions(io({ askImport: async () => ({ ...DEFAULT_IMPORT_OPTIONS, dpi: 300 }) }));
    expect(spy.mock.calls[0]![0].options.dpi).toBe(300);
  });

  it('con la de fábrica (96) no se manda nada: el motor ya usa 96', async () => {
    const spy = vi.spyOn(getApi(), 'importFileWith');
    await importWithOptions(io());
    expect(spy.mock.calls[0]![0].options.dpi).toBeNull();
  });

  it('abrir o arrastrar un archivo usa la resolución de Preferencias si no es la de fábrica', async () => {
    const plain = vi.spyOn(getApi(), 'importFile');
    const withOpts = vi.spyOn(getApi(), 'importFileWith');
    await importPaths(['/tmp/a.svg']);
    expect(plain).toHaveBeenCalledTimes(1);
    expect(withOpts).not.toHaveBeenCalled();
    s().setPref('importDpi', 72);
    await importPaths(['/tmp/b.svg']);
    expect(withOpts).toHaveBeenCalledTimes(1);
    expect(withOpts.mock.calls[0]![0].options).toMatchObject({ dpi: 72, scale_percent: 100, group: false });
    expect(withOpts.mock.calls[0]![0].options).not.toHaveProperty('auto_group');
  });
});
