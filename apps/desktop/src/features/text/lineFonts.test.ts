import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { mockTextOutline } from '@/api/mock/text';
import type { FontFamily, TextObject } from '@/api/types';
import { bezBounds, polylineBez } from '@/lib/bezpath';
import { useStore } from '@/store';
import { filterFonts, isLineFamily, isLineFont, loadFonts, resetFontsCache } from './fonts';
import { importShxFonts } from './shx';
import { paramsKey } from './signature';

const textKind = (patch: Partial<TextObject> = {}): TextObject => ({
  text: 'Hola', font_family: 'Arial', bold: false, italic: false, size_mm: 10, letter_spacing_mm: 0,
  line_spacing: 1.2, align: 'left', on_path: null, outline: [], ...patch,
});

const FONTS: FontFamily[] = [
  { family: 'Arial', styles: ['Regular'] },
  { family: 'Bebas Neue', styles: ['Regular'] },
  { family: 'Hershey Roman Simplex', styles: ['Regular'], single_line: true },
  { family: 'Hershey Script Simplex', styles: ['Regular'], single_line: true },
];

describe('curvado y soldado en la firma', () => {
  it('cambian la firma, pero ausente es lo mismo que 0 / falso', () => {
    const base = paramsKey(textKind());
    expect(paramsKey(textKind({ bend_deg: 45 }))).not.toBe(base);
    expect(paramsKey(textKind({ bend_deg: -45 }))).not.toBe(paramsKey(textKind({ bend_deg: 45 })));
    expect(paramsKey(textKind({ welded: true }))).not.toBe(base);
    expect(paramsKey(textKind({ bend_deg: 0, welded: false }))).toBe(base);
    expect(paramsKey(textKind({ bend_deg: undefined, welded: undefined }))).toBe(base);
  });
});

describe('fuentes de una línea en la lista', () => {
  it('van después de las del sistema, cada grupo con su propio orden de búsqueda', () => {
    expect(filterFonts(FONTS, '').map((f) => f.family)).toEqual(['Arial', 'Bebas Neue', 'Hershey Roman Simplex', 'Hershey Script Simplex']);
    // Con búsqueda cada grupo conserva su orden (las del sistema siempre primero).
    expect(filterFonts(FONTS, 's').map((f) => f.family)).toEqual(['Bebas Neue', 'Hershey Roman Simplex', 'Hershey Script Simplex']);
    expect(filterFonts([...FONTS].reverse(), 'e').map((f) => f.family)).toEqual(['Bebas Neue', 'Hershey Script Simplex', 'Hershey Roman Simplex']);
    expect(filterFonts(FONTS, 'script').map((f) => f.family)).toEqual(['Hershey Script Simplex']);
    expect(filterFonts(FONTS, 'hershey').every(isLineFont)).toBe(true);
  });

  it('reconoce si la familia pedida es de una línea', () => {
    expect(isLineFamily(FONTS, 'hershey roman simplex')).toBe(true);
    expect(isLineFamily(FONTS, 'Arial')).toBe(false);
    expect(isLineFamily(FONTS, 'No existe')).toBe(false);
    expect(isLineFamily([], 'Hershey Roman Simplex')).toBe(false);
  });
});

describe('mock de texto con una línea, curvado y SHX', () => {
  it('una fuente Hershey da trazos abiertos y no tiene negrita', () => {
    const r = mockTextOutline(textKind({ font_family: 'Hershey Roman Simplex', text: 'ab c', bold: true }), null);
    expect(r.paths).toHaveLength(3);
    expect(r.paths.every((p) => p.every((s) => s.type !== 'close'))).toBe(true);
    expect(r.warnings.join(' ')).toMatch(/negrita/);
    // La mayúscula mide 0,72 del em.
    expect(bezBounds(r.paths)![1][1]).toBeCloseTo(7.2, 6);
  });

  it('el curvado mueve los extremos y 0 no cambia nada', () => {
    const flat = mockTextOutline(textKind({ text: 'abcd', align: 'center' }), null);
    const same = mockTextOutline(textKind({ text: 'abcd', align: 'center', bend_deg: 0 }), null);
    expect(same.paths).toEqual(flat.paths);
    const up = bezBounds(mockTextOutline(textKind({ text: 'abcd', align: 'center', bend_deg: 120 }), null).paths)!;
    const down = bezBounds(mockTextOutline(textKind({ text: 'abcd', align: 'center', bend_deg: -120 }), null).paths)!;
    const base = bezBounds(flat.paths)!;
    // Positivo baja los extremos; negativo los sube.
    expect(up[0][1]).toBeLessThan(base[0][1] - 1);
    expect(down[1][1]).toBeGreaterThan(base[1][1] + 1);
  });

  it('sobre un trayecto se ignora la curvatura', () => {
    const on_path = { object_id: 'g', offset_mm: 0, flip: false };
    const guide = polylineBez([[0, 0], [100, 0]], false);
    const a = mockTextOutline(textKind({ text: 'ab', on_path }), guide);
    const b = mockTextOutline(textKind({ text: 'ab', on_path, bend_deg: 90 }), guide);
    expect(b.paths).toEqual(a.paths);
  });
});

describe('importar fuentes SHX', () => {
  let api: MockApi;
  beforeEach(() => {
    api = createMockApi({ latencyMs: 0 });
    setApi(api);
    resetFontsCache();
    useStore.setState({ notices: [] });
  });
  afterEach(() => api.dispose());

  it('el mock lista las fuentes de una línea al final y suma las importadas', async () => {
    const before = await api.listFonts();
    const first = before.findIndex(isLineFont);
    expect(first).toBeGreaterThan(0);
    expect(before.slice(first).every(isLineFont)).toBe(true);
    const names = await api.importShxFonts({ paths: ['/tmp/mi fuente.shx', 'C:\\fuentes\\otra.SHX'] });
    expect(names).toEqual(['mi fuente', 'otra']);
    expect((await api.listFonts()).map((f) => f.family)).toEqual(expect.arrayContaining(['mi fuente', 'otra']));
    await expect(api.importShxFonts({ paths: [] })).rejects.toThrow();
  });

  it('el flujo de la interfaz importa, refresca la lista y avisa', async () => {
    await loadFonts();
    const names = await importShxFonts();
    expect(names).toEqual(['romans']);
    expect((await loadFonts()).some((f) => f.family === 'romans' && f.single_line)).toBe(true);
    const notices = useStore.getState().notices;
    expect(notices.at(-1)?.level).toBe('success');
    expect(notices.at(-1)?.text).toContain('romans');
  });

  it('si el motor falla avisa del error y no devuelve nombres', async () => {
    api.importShxFonts = () => Promise.reject(new Error('El archivo no es una fuente SHX de AutoCAD'));
    expect(await importShxFonts()).toEqual([]);
    const last = useStore.getState().notices.at(-1)!;
    expect(last.level).toBe('error');
    expect(last.text).toContain('SHX');
  });
});

describe('panel con curvado, soldado y SHX', () => {
  it('muestra los campos nuevos', async () => {
    const { createElement } = await import('react');
    const { renderToString } = await import('react-dom/server');
    const { TextEditor } = await import('./TextPanel');
    const obj = {
      id: 't', layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0] as [number, number, number, number, number, number], locked: false, hidden: false, group: null,
      kind: { type: 'text' as const, ...textKind({ bend_deg: 45, welded: true }) },
    };
    const html = renderToString(createElement(TextEditor, { obj }));
    for (const label of ['Curvatura', 'Soldar letras', 'Importar fuente SHX…', 'value="45"']) {
      expect(html, label).toContain(label);
    }
    // Con una fuente Hershey se cita a sus autores.
    const hershey = renderToString(createElement(TextEditor, { obj: { ...obj, kind: { ...obj.kind, font_family: 'Hershey Cursive' } } }));
    expect(hershey).toContain('Fuentes Hershey: creadas');
    expect(html).not.toContain('Fuentes Hershey: creadas');
    // El soldado está activado.
    expect(html).toMatch(/type="checkbox"[^>]*checked/);
  });
});
