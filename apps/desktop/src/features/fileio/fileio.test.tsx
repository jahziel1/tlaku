import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { commands, MENUS } from '@/commands';
import { useStore } from '@/store';
import { buildMenus } from '@/features/shell/menuModel';
import { exportAs, exportEnabled, fmtBytes, importWithOptions, printDesign, suggestedName, type FileioIo } from './actions';
import { ExportDialog, defaultRemembered, type ExportChoice } from './ExportDialog';
import { ImportOptionsDialog, kindsOf } from './ImportOptionsDialog';
import { DEFAULT_IMPORT_OPTIONS, DEFAULT_PDF, DEFAULT_PNG, DEFAULT_SVG, type ExportFormat } from './types';

const s = () => useStore.getState();

/** Preguntas que contestan siempre lo mismo, para probar el flujo sin diálogos. */
function io(over: Partial<FileioIo> = {}): FileioIo {
  return {
    askExport: async (format, _print, _n) => ({ scope: 'all', request: { format } as ExportChoice['request'] }),
    askImport: async () => ({ ...DEFAULT_IMPORT_OPTIONS, scale_percent: 50 }),
    pickSavePath: async (format, name) => `/tmp/${name.replace(/\.[^.]+$/, '')}.${format}`,
    pickImportPaths: async () => ['/tmp/diseno.pdf'],
    ...over,
  };
}

beforeEach(() => {
  setApi(createMockApi({ latencyMs: 0 }));
  s().newDocument();
  for (const n of s().notices) s().dismissNotice(n.id);
});

describe('utilidades', () => {
  it('formatea tamaños y nombres sugeridos', () => {
    expect(fmtBytes(512)).toBe('512 B');
    expect(fmtBytes(2048)).toBe('2 KB');
    expect(fmtBytes(1536 * 1024)).toBe('1,5 MB');
    expect(suggestedName('/a/b/placa.tlaku', 'svg')).toBe('placa.svg');
    expect(suggestedName(null, 'pdf')).toBe('Sin título.pdf');
  });

  it('qué opciones de importación tocan a cada formato', () => {
    expect(kindsOf(['svg'])).toMatchObject({ dpi: true, dxf: false, pdf: false, gcode: false, vector: true });
    expect(kindsOf(['dxf', 'ai'])).toMatchObject({ dxf: true, pdf: true, dpi: false });
    expect(kindsOf(['nc'])).toMatchObject({ gcode: true });
    // Una imagen sola no tiene capa destino de vectores.
    expect(kindsOf(['webp', 'tif'])).toMatchObject({ dpi: true, vector: false });
  });
});

describe('exportar', () => {
  it('sin objetos no se puede exportar', async () => {
    expect(exportEnabled(s())).toBe('fileio.reason.empty');
    const ask = vi.fn();
    expect(await exportAs('svg', io({ askExport: ask }))).toBe(false);
    expect(ask).not.toHaveBeenCalled();
  });

  it.each<ExportFormat>(['svg', 'dxf', 'png', 'pdf', 'ai'])('exporta %s: manda el documento, la ruta con extensión y avisa', async (format) => {
    s().applyImport(await (await import('@/api')).getApi().importFile({ path: 'a.svg' }));
    expect(exportEnabled(s())).toBe(true);
    const spy = vi.spyOn((await import('@/api')).getApi(), 'exportDesign');
    expect(await exportAs(format, io())).toBe(true);
    const call = spy.mock.calls[0]![0];
    expect(call.selection).toBeNull();
    expect(call.path).toMatch(new RegExp(`\\.${format}$`));
    expect(call.request.format).toBe(format);
    expect(call.doc).toBe(s().doc);
    expect(s().notices.at(-1)).toMatchObject({ level: 'success' });
    expect(s().notices.at(-1)!.text).toContain('Exportado a');
  });

  it('solo la selección manda sus ids; cancelar no exporta ni avisa', async () => {
    const { getApi } = await import('@/api');
    const ids = s().applyImport(await getApi().importFile({ path: 'a.svg' }));
    s().select([ids[0]!]);
    const spy = vi.spyOn(getApi(), 'exportDesign');
    const only = io({ askExport: async (format) => ({ scope: 'selection', request: { format } as ExportChoice['request'] }) });
    expect(await exportAs('dxf', only)).toBe(true);
    expect(spy.mock.calls[0]![0].selection).toEqual([ids[0]]);
    spy.mockClear();
    const n = s().notices.length;
    expect(await exportAs('dxf', io({ askExport: async () => null }))).toBe(false);
    expect(await exportAs('dxf', io({ pickSavePath: async () => null }))).toBe(false);
    expect(spy).not.toHaveBeenCalled();
    expect(s().notices).toHaveLength(n);
  });

  it('los errores del motor salen como aviso de error', async () => {
    const { getApi } = await import('@/api');
    s().applyImport(await getApi().importFile({ path: 'a.svg' }));
    vi.spyOn(getApi(), 'exportDesign').mockRejectedValue('disco lleno');
    expect(await exportAs('png', io())).toBe(false);
    expect(s().notices.at(-1)).toMatchObject({ level: 'error' });
    expect(s().notices.at(-1)!.text).toContain('disco lleno');
  });

  it('las advertencias del exportador salen en un aviso persistente con su detalle', async () => {
    const { getApi } = await import('@/api');
    s().applyImport(await getApi().importFile({ path: 'a.svg' }));
    vi.spyOn(getApi(), 'exportDesign').mockResolvedValue({ path: '/x.svg', bytes: 10, size_mm: [1, 1], warnings: ['Falta una imagen'] });
    await exportAs('svg', io());
    const n = s().notices.at(-1)!;
    expect(n).toMatchObject({ level: 'warning', sticky: true });
    expect(n.details).toEqual(['Falta una imagen']);
  });

  it('imprimir usa las opciones del PDF', async () => {
    const { getApi } = await import('@/api');
    s().applyImport(await getApi().importFile({ path: 'a.svg' }));
    const spy = vi.spyOn(getApi(), 'printDesign');
    const ask = io({ askExport: async () => ({ scope: 'all', request: { format: 'pdf', page: 'a4' } }) });
    expect(await printDesign(ask)).toBe(true);
    expect(spy.mock.calls[0]![0].options).toEqual({ page: 'a4' });
    expect(s().notices.at(-1)!.text).toContain('escala 1:1');
  });
});

describe('importar con opciones', () => {
  it('pasa las opciones, selecciona lo importado y avisa de las advertencias', async () => {
    const { getApi } = await import('@/api');
    const spy = vi.spyOn(getApi(), 'importFileWith');
    const n = await importWithOptions(io());
    expect(n).toBeGreaterThan(0);
    expect(spy.mock.calls[0]![0]).toMatchObject({ path: '/tmp/diseno.pdf', options: { scale_percent: 50 } });
    expect(s().selectedIds).toHaveLength(n);
    const warn = s().notices.find((x) => x.level === 'warning');
    expect(warn?.details?.[0]).toContain('imagen');
  });

  it('un formato no admitido o cancelar el diálogo no importa nada', async () => {
    expect(await importWithOptions(io({ pickImportPaths: async () => ['/tmp/a.docx'] }))).toBe(0);
    expect(s().notices.at(-1)).toMatchObject({ level: 'error' });
    expect(await importWithOptions(io({ askImport: async () => null }))).toBe(0);
    expect(s().doc.objects).toHaveLength(0);
  });

  it('el simulador aplica escala, grupo y capa destino', async () => {
    const { getApi } = await import('@/api');
    const base = await getApi().importFile({ path: 'a.svg' });
    const r = await getApi().importFileWith({ path: 'a.svg', options: { scale_percent: 200, group: true, target_layer: '#00ff00' } });
    expect(r.objects[0]!.transform[0]).toBe(base.objects[0]!.transform[0] * 2);
    expect(new Set(r.objects.map((o) => o.group))).toEqual(new Set(['grp-mock']));
    expect(r.objects.filter((o) => o.kind.type !== 'image').every((o) => o.layer === '#00ff00')).toBe(true);
  });
});

describe('comandos y menús', () => {
  it('registra los comandos con su prefijo y los pone en el menú Archivo', () => {
    const ids = ['importOptions', 'exportSvg', 'exportDxf', 'exportPng', 'exportPdf', 'exportAi', 'print'].map((n) => `fileio.${n}`);
    for (const id of ids) expect(commands.get(id), id).toBeDefined();
    const file = MENUS.find((m) => m.id === 'file')!;
    for (const id of ids) expect(file.items).toContain(id);
    const shell = buildMenus(commands).find((m) => m.id === 'file')!;
    expect(shell.items.filter((i) => i.kind === 'command' && i.id.startsWith('fileio.'))).toHaveLength(ids.length);
  });

  it('exportar y imprimir están deshabilitados sin objetos y dicen por qué', () => {
    expect(commands.availability('fileio.exportSvg')).toBe('fileio.reason.empty');
    expect(commands.availability('fileio.print')).toBe('fileio.reason.empty');
    expect(commands.availability('fileio.importOptions')).toBe(true);
  });
});

describe('diálogos', () => {
  const noop = () => undefined;

  it('exportación: opciones propias de cada formato y alcance solo con selección', () => {
    const html = (format: ExportFormat, n = 0) =>
      renderToString(<ExportDialog format={format} selectionCount={n} initial={defaultRemembered()} onDone={noop} />);
    expect(html('svg')).toContain('Exportar SVG');
    expect(html('svg')).toContain('Incrustar');
    expect(html('svg')).not.toContain('Solo la selección');
    expect(html('svg', 3)).toContain('Solo la selección (3)');
    expect(html('dxf')).toContain('Curvas exactas (SPLINE)');
    expect(html('png')).toContain('Resolución (ppp)');
    expect(html('pdf')).toContain('escala 1:1');
    expect(renderToString(<ExportDialog format="pdf" print selectionCount={0} initial={defaultRemembered()} onDone={noop} />)).toContain('Imprimir');
  });

  it('importación: solo muestra lo que corresponde a los archivos', () => {
    const html = (exts: string[]) =>
      renderToString(<ImportOptionsDialog fileCount={exts.length} kinds={kindsOf(exts)} layers={[{ color: '#ff0000', name: 'Corte' }]} onDone={noop} />);
    const svg = html(['svg']);
    expect(svg).toContain('Opciones de importación');
    expect(svg).toContain('Resolución (ppp)');
    expect(svg).toContain('Corte');
    expect(svg).not.toContain('Unidad de un DXF');
    expect(html(['dxf'])).toContain('Unidad de un DXF');
    expect(html(['pdf'])).toContain('Página del PDF');
    expect(html(['nc'])).toContain('Una capa por potencia');
    expect(html(['tga'])).not.toContain('Capa destino');
  });

  it('los valores por defecto de la interfaz coinciden con los del motor', () => {
    // Si cambian en Rust (tlaku-export lib.rs), cambian aquí.
    expect(DEFAULT_SVG).toMatchObject({ images: 'embed', stroke_mm: 0.1, margin_mm: 0, keep_position: false });
    expect(DEFAULT_PNG).toMatchObject({ dpi: 300, margin_mm: 2, transparent: false, fill_layers: true });
    expect(DEFAULT_PDF).toMatchObject({ page: 'fit', margin_mm: 5, images: true });
  });
});
