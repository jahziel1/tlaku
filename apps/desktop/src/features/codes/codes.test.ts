import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { mockGenerateCode } from '@/api/mock/codes';
import type { CodeSpec, Document } from '@/api/types';
import { codesCommands } from '@/commands/definitions/codes';
import { commands } from '@/commands';
import { es } from '@/i18n';
import { bezBounds } from '@/lib/bezpath';
import { createAppStore, useStore } from '@/store';
import { specToSend } from './CodesDialog';
import { useCodesDialog } from './codesStore';
import { DEFAULT_CODE_SPEC, KINDS, kindInfo } from './kinds';
import { applyCodeToDoc, codeCenter, codeObject } from './placement';

const spec = (patch: Partial<CodeSpec> = {}): CodeSpec => ({ ...DEFAULT_CODE_SPEC, data: 'hola', ...patch });
const emptyDoc = (): Document => ({ version: 2, objects: [], layers: [], device_id: null });

describe('tipos de código', () => {
  it('todos tienen etiqueta y ayuda traducidas, y los de barras son lineales', () => {
    expect(KINDS.map((k) => k.kind)).toEqual(['qr', 'data_matrix', 'code128', 'code39', 'ean13', 'ean8', 'upc_a', 'itf', 'codabar']);
    for (const k of KINDS) {
      expect(es[k.label], k.kind).toBeTruthy();
      expect(es[k.hint], k.kind).toBeTruthy();
    }
    expect(KINDS.filter((k) => !k.linear).map((k) => k.kind)).toEqual(['qr', 'data_matrix']);
    expect(kindInfo('ean13').linear).toBe(true);
  });
});

describe('lo que se pide al motor', () => {
  it('un QR ignora el texto legible, la fuente y el alto: no recalcula por ellos', () => {
    const a = specToSend(spec({ human_readable: true, height_mm: 9, ecc: 'h' }), 'Arial');
    expect(a.human_readable).toBe(false);
    expect(a.font_family).toBe('');
    expect(a.height_mm).toBe(15);
    expect(a.ecc).toBe('h');
  });

  it('un código de barras manda el texto legible con la fuente efectiva y no usa la corrección', () => {
    const a = specToSend(spec({ kind: 'code128', human_readable: true, height_mm: 9, ecc: 'h' }), 'Arial');
    expect(a).toMatchObject({ human_readable: true, font_family: 'Arial', height_mm: 9, ecc: 'm' });
    expect(specToSend(spec({ kind: 'code128' }), 'Arial').font_family).toBe('');
  });
});

describe('colocación', () => {
  it('el centro es el del área visible si cae en la cama; si no, el de la cama', () => {
    const view = { zoom: 2, panX: 100, panY: 500, width: 800, height: 400 };
    // centro de pantalla (400, 200) -> mm: ((400-100)/2, (500-200)/2) = (150, 150)
    expect(codeCenter(view, [400, 300])).toEqual([150, 150]);
    expect(codeCenter({ ...view, panX: -5000 }, [400, 300])).toEqual([200, 150]);
    expect(codeCenter({ ...view, zoom: 0 }, [400, 300])).toEqual([200, 150]);
  });

  it('el objeto queda centrado y con las formas tal cual', () => {
    const paths = mockGenerateCode(spec()).paths;
    const o = codeObject('code-1', { paths, size: [30, 30], center: [100, 80], layer: '#ff0000' });
    expect(o.transform).toEqual([1, 0, 0, 1, 85, 65]);
    expect(o.kind).toEqual({ type: 'vector', paths });
    const [[x0, y0], [x1, y1]] = bezBounds(paths)!;
    expect([x0 + 85, y0 + 65, x1 + 85, y1 + 65]).toEqual([85, 65, 115, 95]);
  });

  it('agrega la capa, asigna un id libre y no hace nada sin formas', () => {
    const paths = mockGenerateCode(spec()).paths;
    const r = applyCodeToDoc(emptyDoc(), { paths, size: [30, 30], center: [0, 0], layer: '#00ff00' });
    expect(r.id).toBe('code-1');
    expect(r.doc.objects).toHaveLength(1);
    expect(r.doc.layers.some((l) => l.color === '#00ff00')).toBe(true);
    const again = applyCodeToDoc(r.doc, { paths, size: [30, 30], center: [0, 0], layer: '#00ff00' });
    expect(again.id).not.toBe(r.id);
    expect(again.doc.objects).toHaveLength(2);
    expect(applyCodeToDoc(r.doc, { paths: [], size: [0, 0], center: [0, 0], layer: '#00ff00' })).toEqual({ doc: r.doc, id: null });
  });
});

describe('mock de códigos', () => {
  it('respeta el tamaño pedido y, al invertir, suma el margen y trae el fondo', () => {
    const a = mockGenerateCode(spec({ size_mm: 40 }));
    expect(a.width_mm).toBe(40);
    expect(a.height_mm).toBe(40);
    const inv = mockGenerateCode(spec({ size_mm: 40, invert: true, margin_mm: 3 }));
    expect(inv.width_mm).toBe(46);
    expect(inv.paths.length).toBe(a.paths.length + 1);
  });

  it('los de barras usan el alto y dejan sitio al texto legible', () => {
    const a = mockGenerateCode(spec({ kind: 'code128', size_mm: 50, height_mm: 10 }));
    expect(a.height_mm).toBe(10);
    expect(mockGenerateCode(spec({ kind: 'code128', size_mm: 50, height_mm: 10, human_readable: true })).height_mm).toBeGreaterThan(10);
  });

  it('avisa de los módulos muy pequeños y falla sin contenido o con datos inválidos', () => {
    expect(mockGenerateCode(spec({ size_mm: 3 })).warnings.join(' ')).toMatch(/0,25/);
    expect(() => mockGenerateCode(spec({ data: '' }))).toThrow(/contenido/);
    expect(() => mockGenerateCode(spec({ kind: 'ean13', data: '123' }))).toThrow(/EAN-13/);
  });

  it('la API simulada responde por generateCode', async () => {
    const api: MockApi = createMockApi({ latencyMs: 0 });
    setApi(api);
    const r = await api.generateCode({ spec: spec() });
    expect(r.paths.length).toBeGreaterThan(10);
    await expect(api.generateCode({ spec: spec({ data: '' }) })).rejects.toThrow();
    api.dispose();
  });
});

describe('comando e interfaz', () => {
  beforeEach(() => useCodesDialog.getState().close());
  afterEach(() => useCodesDialog.getState().close());

  it('"Insertar código…" está en Herramientas y abre el diálogo con el contenido vacío', () => {
    expect(commands.get('tools.insertCode')).toBeDefined();
    expect(codesCommands.every((c) => c.category === 'tools')).toBe(true);
    useCodesDialog.getState().patch({ data: 'anterior', size_mm: 44 });
    const cmd = codesCommands.find((c) => c.id === 'tools.insertCode')!;
    cmd.run(createAppStore().getState());
    const s = useCodesDialog.getState();
    expect(s.open).toBe(true);
    // Se recuerdan los ajustes, no el contenido.
    expect(s.spec.data).toBe('');
    expect(s.spec.size_mm).toBe(44);
  });

  it('el diálogo muestra todos los campos del QR', async () => {
    useStore.setState({ doc: emptyDoc() });
    const { CodesContent } = await import('./CodesDialog');
    useCodesDialog.getState().show();
    const html = renderToString(createElement(CodesContent, { titleId: 't' }));
    for (const label of ['Insertar código', 'Tipo de código', 'Contenido', 'Corrección de errores', 'Lado del código', 'Margen', 'Invertir (grabar el fondo)', 'Cancelar', 'Insertar']) {
      expect(html, label).toContain(label);
    }
    // Sin contenido no se puede insertar y se explica por qué.
    expect(html).toContain('disabled=""');
    expect(html).toContain('Escribe el contenido del código.');
    // El texto legible y el alto son de los códigos de barras.
    expect(html).not.toContain('Texto legible debajo');
    expect(html).not.toContain('Alto de las barras');
  });
});
