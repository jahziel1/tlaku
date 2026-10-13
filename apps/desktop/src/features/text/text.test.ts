import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi, type MockApi } from '@/api/mock';
import { mockTextOutline } from '@/api/mock/text';
import type { DesignObject, Document, FontFamily, TextObject } from '@/api/types';
import { polylineBez, type BezPath } from '@/lib/bezpath';
import { createAppStore } from '@/store';
import { convertTextToPaths, textToVector } from './convert';
import { effectiveFont, filterFonts, matchFamily, pickDefaultFont } from './fonts';
import { OUTLINE_DEBOUNCE_MS, startOutlineSync, type OutlineSync } from './outlineSync';
import { choosePanelSide } from './placement';
import { isTextObject, paramsKey, resolveText, type TextDesignObject } from './signature';
import { useTextStatus } from './status';

const FONTS: FontFamily[] = [
  { family: 'Zapfino', styles: ['Regular'] },
  { family: 'Helvetica Neue', styles: ['Regular', 'Bold'] },
  { family: 'Arial', styles: ['Regular', 'Bold', 'Italic'] },
  { family: 'Bebas Neue', styles: ['Regular'] },
];

const textKind = (patch: Partial<TextObject> = {}): TextObject => ({
  text: 'Hola', font_family: 'Arial', bold: false, italic: false, size_mm: 10, letter_spacing_mm: 0,
  line_spacing: 1.2, align: 'left', on_path: null, outline: [], ...patch,
});

const textObj = (id: string, patch: Partial<TextObject> = {}, extra: Partial<DesignObject> = {}): DesignObject => ({
  id, layer: '#ff0000', transform: [1, 0, 0, 1, 20, 30], locked: false, hidden: false, group: null,
  kind: { type: 'text', ...textKind(patch) }, ...extra,
});

const lineObj = (id: string, len: number, dx = 0): DesignObject => ({
  id, layer: '#ff0000', transform: [1, 0, 0, 1, dx, 0], locked: false, hidden: false, group: null,
  kind: { type: 'vector', paths: [polylineBez([[0, 0], [len, 0]], false)] },
});

const doc = (objects: DesignObject[]): Document => ({ version: 2, objects, layers: [], device_id: null });
const asText = (o: DesignObject) => {
  if (!isTextObject(o)) throw new Error('no es texto');
  return o;
};
const square: BezPath = polylineBez([[0, 0], [1, 0], [1, 1]], true);

describe('firma de parámetros', () => {
  it('es la misma para los mismos parámetros e ignora el contorno', () => {
    const d = doc([]);
    const a = asText(textObj('a'));
    const b = asText(textObj('b', { outline: [square] }));
    expect(resolveText(a, d)).toEqual(resolveText(b, d));
  });

  it('cambia con cada parámetro que afecta al contorno', () => {
    const base = paramsKey(textKind());
    const variants: Partial<TextObject>[] = [
      { text: 'Adiós' }, { font_family: 'Bebas Neue' }, { bold: true }, { italic: true }, { size_mm: 12 },
      { letter_spacing_mm: 1 }, { line_spacing: 2 }, { align: 'center' },
      { on_path: { object_id: 'g', offset_mm: 0, flip: false } },
    ];
    for (const v of variants) expect(paramsKey(textKind(v)), JSON.stringify(v)).not.toBe(base);
    const key = paramsKey(textKind({ on_path: { object_id: 'g', offset_mm: 0, flip: false } }));
    expect(paramsKey(textKind({ on_path: { object_id: 'g', offset_mm: 3, flip: false } }))).not.toBe(key);
    expect(paramsKey(textKind({ on_path: { object_id: 'g', offset_mm: 0, flip: true } }))).not.toBe(key);
  });

  it('no distingue mayúsculas en la familia', () => {
    expect(paramsKey(textKind({ font_family: 'ARIAL ' }))).toBe(paramsKey(textKind()));
  });

  it('con trayecto incluye la guía en coordenadas locales del texto y la cambia al moverla', () => {
    const on_path = { object_id: 'g', offset_mm: 0, flip: false };
    const t = asText(textObj('t', { on_path }));
    const r1 = resolveText(t, doc([t, lineObj('g', 50)]));
    const r2 = resolveText(t, doc([t, lineObj('g', 50, 5)]));
    const r3 = resolveText(t, doc([t, lineObj('g', 60)]));
    if (!r1.ok || !r2.ok || !r3.ok) throw new Error('la guía debería resolverse');
    expect(new Set([r1.sig, r2.sig, r3.sig]).size).toBe(3);
    // El texto está en (20, 30): la guía local arranca en (-20, -30).
    expect(r1.path?.[0]).toEqual({ type: 'move_to', to: [-20, -30] });
    // Mover el texto también cambia la guía local.
    const moved = { ...t, transform: [1, 0, 0, 1, 25, 30] as TextDesignObject['transform'] };
    const r4 = resolveText(moved, doc([moved, lineObj('g', 50)]));
    expect(r4.ok && r4.sig).not.toBe(r1.sig);
  });

  it('falla si la guía no existe, no es vector o es el propio texto', () => {
    const t = asText(textObj('t', { on_path: { object_id: 'g', offset_mm: 0, flip: false } }));
    expect(resolveText(t, doc([t]))).toEqual({ ok: false, reason: 'target' });
    expect(resolveText(t, doc([t, textObj('g')]))).toEqual({ ok: false, reason: 'target' });
    const self = asText(textObj('s', { on_path: { object_id: 's', offset_mm: 0, flip: false } }));
    expect(resolveText(self, doc([self]))).toEqual({ ok: false, reason: 'target' });
  });
});

describe('fuente por defecto', () => {
  it('prefiere una sans-serif conocida, en el orden de preferencia', () => {
    expect(pickDefaultFont(FONTS)).toBe('Arial');
    expect(pickDefaultFont(FONTS.filter((f) => f.family !== 'Arial'))).toBe('Helvetica Neue');
  });
  it('si no hay ninguna conocida usa la primera; sin fuentes, cadena vacía', () => {
    expect(pickDefaultFont([FONTS[0]!, FONTS[3]!])).toBe('Zapfino');
    expect(pickDefaultFont([])).toBe('');
  });
  it('respeta la familia instalada (sin distinguir mayúsculas) y sustituye la que falta', () => {
    expect(effectiveFont(FONTS, 'bebas neue')).toEqual({ family: 'Bebas Neue', fellBack: false });
    expect(effectiveFont(FONTS, 'Comic Sans')).toEqual({ family: 'Arial', fellBack: true });
    // Sin elegir ninguna, se usa la de por defecto sin avisar.
    expect(effectiveFont(FONTS, '  ')).toEqual({ family: 'Arial', fellBack: false });
    // Sin lista de fuentes se respeta lo pedido.
    expect(effectiveFont([], 'Comic Sans')).toEqual({ family: 'Comic Sans', fellBack: false });
  });
  it('busca por texto con las que empiezan igual primero', () => {
    expect(filterFonts(FONTS, 'e').map((f) => f.family)).toEqual(['Helvetica Neue', 'Bebas Neue']);
    expect(filterFonts(FONTS, 'neue').map((f) => f.family)).toEqual(['Helvetica Neue', 'Bebas Neue']);
    expect(filterFonts(FONTS, '')).toHaveLength(4);
    expect(matchFamily(FONTS, 'ARIAL')?.family).toBe('Arial');
  });
});

describe('sincronización del contorno', () => {
  let api: MockApi;
  let store: ReturnType<typeof createAppStore>;
  let sync: OutlineSync;
  let calls: Parameters<MockApi['textOutline']>[0][];

  const start = () => {
    sync = startOutlineSync({
      store,
      api: () => api,
      fonts: () => Promise.resolve(FONTS.concat({ family: 'Courier New', styles: ['Regular'] })),
      debounceMs: OUTLINE_DEBOUNCE_MS,
    });
  };
  const settle = (ms = OUTLINE_DEBOUNCE_MS + 50) => vi.advanceTimersByTimeAsync(ms);
  const setDoc = (objects: DesignObject[]) => store.getState().replaceDocument(doc(objects), null);
  const text = (id: string) => asText(store.getState().doc.objects.find((o) => o.id === id)!);

  beforeEach(() => {
    vi.useFakeTimers();
    useTextStatus.getState().reset();
    api = createMockApi({ latencyMs: 0 });
    calls = [];
    const real = api.textOutline.bind(api);
    api.textOutline = (a) => {
      calls.push(a);
      return real(a);
    };
    store = createAppStore();
  });
  afterEach(() => {
    sync?.stop();
    api.dispose();
    vi.useRealTimers();
  });

  it('genera el contorno de un texto con el contorno vacío, tras el debounce', async () => {
    setDoc([textObj('t')]);
    start();
    expect(useTextStatus.getState().byId.t?.busy).toBe(true);
    await vi.advanceTimersByTimeAsync(OUTLINE_DEBOUNCE_MS - 10);
    expect(calls).toHaveLength(0);
    await settle();
    expect(calls).toHaveLength(1);
    expect(calls[0]!.text.outline).toEqual([]);
    expect(text('t').kind.outline.length).toBe(4); // un rectángulo por letra
    expect(useTextStatus.getState().byId.t?.busy).toBe(false);
  });

  it('no ensucia el historial ni deja el documento sucio', async () => {
    setDoc([textObj('t')]);
    start();
    await settle();
    const s = store.getState();
    expect(s.past).toHaveLength(0);
    expect(s.future).toHaveLength(0);
    expect(s.dirty).toBe(false);
    expect(text('t').kind.outline.length).toBeGreaterThan(0);
  });

  it('un texto creado después (con un paso de historial) recibe contorno sin otro paso, y deshacer/rehacer no lo regenera', async () => {
    start();
    store.getState().addObjects([textObj('t')]);
    const steps = store.getState().past.length; // crear la capa y agregar el objeto
    await settle();
    expect(store.getState().past).toHaveLength(steps);
    expect(calls).toHaveLength(1);
    store.getState().undo();
    await settle();
    store.getState().redo();
    await settle();
    expect(calls).toHaveLength(1);
    expect(text('t').kind.outline.length).toBeGreaterThan(0);
    expect(store.getState().canUndo()).toBe(true);
  });

  it('agrupa las ediciones seguidas en una sola petición', async () => {
    setDoc([textObj('t')]);
    start();
    await settle();
    calls.length = 0;
    for (const text of ['Ho', 'Hol', 'Hola!']) {
      store.getState().applyDocChange('text.history.edit', (d) => ({
        ...d,
        objects: d.objects.map((o) => (isTextObject(o) ? { ...o, kind: { ...o.kind, text } } : o)),
      }));
      await vi.advanceTimersByTimeAsync(100);
    }
    expect(calls).toHaveLength(0);
    await settle();
    expect(calls).toHaveLength(1);
    expect(calls[0]!.text.text).toBe('Hola!');
    expect(text('t').kind.outline).toHaveLength(5);
  });

  it('usa el caché: sin cambios de parámetros no vuelve a pedir, y un contorno ya existente se da por bueno', async () => {
    setDoc([textObj('a', { outline: [square] }), textObj('b')]);
    start();
    await settle();
    expect(calls.map((c) => c.text.text)).toEqual(['Hola']); // solo b
    expect(text('a').kind.outline).toEqual([square]);
    // Mover o cambiar de capa no cambia el contorno.
    store.getState().moveObjects(['a', 'b'], 5, 5);
    store.getState().setObjectsLayer(['a'], '#00ff00');
    await settle();
    expect(calls).toHaveLength(1);
  });

  it('regenera cuando cambian los parámetros y no cuando solo cambia el contorno', async () => {
    setDoc([textObj('t')]);
    start();
    await settle();
    const first = text('t').kind.outline;
    store.getState().applyDocChange('text.history.edit', (d) => ({
      ...d,
      objects: d.objects.map((o) => (isTextObject(o) ? { ...o, kind: { ...o.kind, size_mm: 20 } } : o)),
    }));
    // Mientras tanto se sigue viendo el contorno anterior.
    expect(text('t').kind.outline).toBe(first);
    await settle();
    expect(calls).toHaveLength(2);
    expect(text('t').kind.outline).not.toBe(first);
    // Deshacer vuelve al contorno anterior, que ya está al día: no hay petición.
    store.getState().undo();
    await settle();
    expect(calls).toHaveLength(2);
    expect(text('t').kind.outline).toBe(first);
  });

  it('con fuente no instalada genera con la de por defecto y avisa', async () => {
    setDoc([textObj('t', { font_family: 'Fuente Inventada' })]);
    start();
    await settle();
    expect(calls[0]!.text.font_family).toBe('Arial');
    expect(text('t').kind.font_family).toBe('Fuente Inventada'); // el documento no se toca
    expect(useTextStatus.getState().byId.t?.warnings.join(' ')).toMatch(/Fuente Inventada.*Arial/);
  });

  it('muestra las advertencias del motor', async () => {
    setDoc([textObj('t', { font_family: 'Bebas Neue', italic: true })]);
    start();
    await settle();
    expect(useTextStatus.getState().byId.t?.warnings.join(' ')).toMatch(/cursiva/);
  });

  it('un error del motor se muestra y no se reintenta con los mismos parámetros', async () => {
    setDoc([textObj('t', { size_mm: 0 })]);
    start();
    await settle();
    expect(useTextStatus.getState().byId.t?.error).toMatch(/tamaño/);
    expect(useTextStatus.getState().byId.t?.busy).toBe(false);
    store.getState().moveObjects(['t'], 1, 1);
    await settle();
    expect(calls).toHaveLength(1);
  });

  it('texto en trayecto: pasa la guía en coordenadas locales y regenera al mover la guía', async () => {
    const on_path = { object_id: 'g', offset_mm: 0, flip: false };
    setDoc([lineObj('g', 100), textObj('t', { on_path })]);
    start();
    await settle();
    expect(calls).toHaveLength(1);
    expect(calls[0]!.path?.[0]).toEqual({ type: 'move_to', to: [-20, -30] });
    store.getState().moveObjects(['g'], 10, 0);
    await settle();
    expect(calls).toHaveLength(2);
    expect(calls[1]!.path?.[0]).toEqual({ type: 'move_to', to: [-10, -30] });
    // Cambiar algo ajeno no regenera.
    store.getState().moveObjects(['t', 'g'], 0, 0);
    await settle();
    expect(calls).toHaveLength(2);
  });

  it('texto en trayecto sin guía: avisa y no pide nada', async () => {
    setDoc([textObj('t', { on_path: { object_id: 'nada', offset_mm: 0, flip: false } })]);
    start();
    await settle();
    expect(calls).toHaveLength(0);
    expect(useTextStatus.getState().byId.t?.error).toBeTruthy();
  });

  it('descarta la respuesta de una petición superada por un cambio posterior', async () => {
    setDoc([textObj('t')]);
    api.textOutline = (a) => {
      calls.push(a);
      return new Promise((resolve) => setTimeout(() => resolve(mockTextOutline(a.text, a.path)), 400));
    };
    start();
    await vi.advanceTimersByTimeAsync(OUTLINE_DEBOUNCE_MS + 10); // petición en vuelo
    store.getState().applyDocChange('text.history.edit', (d) => ({
      ...d,
      objects: d.objects.map((o) => (isTextObject(o) ? { ...o, kind: { ...o.kind, text: 'Nuevo' } } : o)),
    }));
    await vi.advanceTimersByTimeAsync(500); // llega la vieja: no debe aplicarse
    expect(text('t').kind.outline).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(OUTLINE_DEBOUNCE_MS + 500);
    expect(text('t').kind.outline).toHaveLength(5);
  });

  it('un texto vacío termina con contorno vacío sin pedirlo en bucle', async () => {
    setDoc([textObj('t', { text: '  ' })]);
    start();
    await settle();
    await settle();
    expect(calls).toHaveLength(1);
    expect(text('t').kind.outline).toEqual([]);
    expect(useTextStatus.getState().byId.t?.busy).toBe(false);
  });
});

describe('convertir a trayectos', () => {
  it('reemplaza el texto por un vector con el mismo id, transform, capa y grupo', () => {
    const t = textObj('t', { outline: [square] }, { group: 'g1', layer: '#00ff00', transform: [2, 0, 0, 2, 4, 5] });
    const other = lineObj('l', 5);
    const out = textToVector(doc([t, other]), 't');
    const v = out.objects[0]!;
    expect(v).toMatchObject({ id: 't', group: 'g1', layer: '#00ff00', transform: [2, 0, 0, 2, 4, 5] });
    expect(v.kind).toEqual({ type: 'vector', paths: [square] });
    expect(out.objects[1]).toBe(other);
  });

  it('no cambia nada sin contorno, con id ajeno o si no es un texto', () => {
    const d = doc([textObj('t'), lineObj('l', 5)]);
    expect(textToVector(d, 't')).toBe(d);
    expect(textToVector(d, 'l')).toBe(d);
    expect(textToVector(d, 'nada')).toBe(d);
  });

  it('con applyDocChange deja un solo paso de deshacer que devuelve el texto', async () => {
    const { useStore } = await import('@/store');
    const t = textObj('t', { outline: [square] });
    useStore.getState().replaceDocument(doc([t]), null);
    expect(convertTextToPaths('t')).toBe(true);
    expect(useStore.getState().past).toHaveLength(1);
    expect(useStore.getState().doc.objects[0]!.kind.type).toBe('vector');
    expect(convertTextToPaths('t')).toBe(false);
    useStore.getState().undo();
    expect(useStore.getState().doc.objects[0]).toBe(t);
  });
});

describe('colocación del panel', () => {
  const box = (left: number, top: number, right: number, bottom: number) => ({ left, top, right, bottom });
  it('va a la derecha por defecto y a la izquierda si la selección llega hasta allí', () => {
    expect(choosePanelSide(null, 1000)).toBe('right');
    expect(choosePanelSide(box(100, 100, 300, 200), 1000)).toBe('right');
    expect(choosePanelSide(box(600, 100, 900, 200), 1000)).toBe('left');
    // Una selección debajo del panel no estorba.
    expect(choosePanelSide(box(600, 700, 900, 800), 1000)).toBe('right');
    // Si cubre todo, se queda a la derecha.
    expect(choosePanelSide(box(0, 0, 1000, 600), 1000)).toBe('right');
  });
});

describe('mock de texto', () => {
  it('devuelve un rectángulo por letra visible, respeta líneas y alineación', () => {
    const r = mockTextOutline(textKind({ text: 'ab\nc', align: 'center' }), null);
    expect(r.paths).toHaveLength(3);
    expect(r.warnings).toEqual([]);
  });
  it('avisa de cursiva sin soporte y falla con fuente desconocida o sin guía', () => {
    expect(mockTextOutline(textKind({ font_family: 'Bebas Neue', italic: true }), null).warnings[0]).toMatch(/cursiva/);
    expect(() => mockTextOutline(textKind({ font_family: 'Nada' }), null)).toThrow(/no está instalada/);
    expect(() => mockTextOutline(textKind({ on_path: { object_id: 'g', offset_mm: 0, flip: false } }), null)).toThrow(/trayectoria/);
  });
  it('sobre un trayecto avisa de lo que no cabe', () => {
    const on_path = { object_id: 'g', offset_mm: 0, flip: false };
    const r = mockTextOutline(textKind({ text: 'abcdefghij', on_path }), polylineBez([[0, 0], [20, 0]], false));
    expect(r.paths.length).toBeLessThan(10);
    expect(r.warnings.join(' ')).toMatch(/no cabe/);
  });
});

describe('panel', () => {
  it('dibuja todos los campos del texto', async () => {
    const { createElement } = await import('react');
    const { renderToString } = await import('react-dom/server');
    const { TextEditor } = await import('./TextPanel');
    // En el servidor zustand lee el estado inicial: basta para ver la estructura del panel.
    const html = renderToString(createElement(TextEditor, { obj: asText(textObj('t')) }));
    for (const label of ['Propiedades del texto', 'Contenido', 'Fuente', 'Negrita', 'Cursiva', 'Tamaño', 'Espaciado', 'Interlineado', 'Sobre trayecto', 'Convertir a trayectos', 'Hola']) {
      expect(html, label).toContain(label);
    }
    // Sin contorno el botón se explica en vez de quedar gris sin motivo.
    expect(html).toContain('aria-disabled="true"');
    expect(html).toContain('todavía');
  });
});
