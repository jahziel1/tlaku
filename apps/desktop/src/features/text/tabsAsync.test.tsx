// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getApi, setApi } from '@/api';
import type { LaserApi } from '@/api';
import { mockTextOutline } from '@/api/mock/text';
import type { DesignObject, Document, FontCapabilities, TextObject } from '@/api/types';
import { setLanguage } from '@/i18n';
import { createAppStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { cleanup, mount } from '@/test/dom';
import { clearCapabilitiesCache } from './capabilities';
import { OpenTypeSection } from './OpenTypeSection';
import { startOutlineSync, type OutlineSync } from './outlineSync';
import { isTextObject } from './signature';
import { useTextStatus } from './status';

/**
 * El texto avanzado (contornos, funciones OpenType y ejes) pide cosas al motor. Si el usuario cambia de
 * pestaña mientras tanto, la respuesta no puede acabar en el documento de otra ni ofrecer a un texto los
 * ejes de la fuente de otro.
 */

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (v: T) => void;
}
function deferred<T>(): Deferred<T> {
  const d = {} as Deferred<T>;
  d.promise = new Promise<T>((resolve) => void (d.resolve = resolve));
  return d;
}
const flush = () => act(async () => void (await new Promise((r) => setTimeout(r, 0))));

const textKind = (text: string, patch: Partial<TextObject> = {}): TextObject => ({
  text, font_family: 'Arial', bold: false, italic: false, size_mm: 10, letter_spacing_mm: 0,
  line_spacing: 1.2, align: 'left', on_path: null, outline: [], ...patch,
});
const textObj = (id: string, text: string): DesignObject => ({
  id, layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null, kind: { type: 'text', ...textKind(text) },
});
const docOf = (...objects: DesignObject[]): Document => ({ ...emptyDocument(), objects });

let original: LaserApi;
beforeEach(() => {
  setLanguage('es');
  original = getApi();
  clearCapabilitiesCache();
  useTextStatus.getState().reset();
});
afterEach(async () => {
  await cleanup();
  setApi(original);
});

describe('contorno del texto y pestañas', () => {
  let store: ReturnType<typeof createAppStore>;
  let sync: OutlineSync;
  /** Peticiones de contorno: cada una espera a que la prueba la conteste. */
  let calls: { text: string; reply: Deferred<ReturnType<typeof mockTextOutline>> }[];
  const s = () => store.getState();
  const outlineOf = (doc: Document, id: string): number => {
    const o = doc.objects.find((x) => x.id === id)!;
    return isTextObject(o) ? o.kind.outline.length : -1;
  };
  const answer = (i: number) => calls[i]!.reply.resolve(mockTextOutline(textKind(calls[i]!.text), null));

  beforeEach(() => {
    calls = [];
    store = createAppStore();
    sync = startOutlineSync({
      store,
      api: () => ({
        textOutline: ({ text }) => {
          const reply = deferred<ReturnType<typeof mockTextOutline>>();
          calls.push({ text: text.text, reply });
          return reply.promise;
        },
      }),
      fonts: () => Promise.resolve([{ family: 'Arial', styles: ['Regular'] }]),
      debounceMs: 0,
    });
  });
  afterEach(() => sync.stop());

  it('el contorno que llega con otra pestaña activa no se escribe en ella; al volver se pide de nuevo', async () => {
    s().openInTab(docOf(textObj('ta', 'Hola')), '/p/a.tlaku');
    const a = s().activeTabId;
    await vi.waitFor(() => expect(calls).toHaveLength(1));

    s().openInTab(docOf(textObj('tb', 'Adiós')), '/p/b.tlaku');
    await vi.waitFor(() => expect(calls).toHaveLength(2));
    expect(calls[1]!.text).toBe('Adiós');

    // Contesta la de A con B activa: ni B ni la foto de A cambian.
    answer(0);
    await flush();
    expect(outlineOf(s().doc, 'tb')).toBe(0);
    expect(outlineOf(s().tabs.find((e) => e.id === a)!.data!.doc, 'ta')).toBe(0);

    answer(1);
    await flush();
    expect(outlineOf(s().doc, 'tb')).toBeGreaterThan(0);

    // Al volver a A se vuelve a pedir su contorno y entra en A.
    s().activateTab(a);
    await vi.waitFor(() => expect(calls).toHaveLength(3));
    expect(calls[2]!.text).toBe('Hola');
    answer(2);
    await flush();
    expect(outlineOf(s().doc, 'ta')).toBeGreaterThan(0);
  });

  it('una pestaña duplicada (mismos ids) pide su propio contorno y no toma el de la otra', async () => {
    s().openInTab(docOf(textObj('t', 'Hola')), '/p/a.tlaku');
    const a = s().activeTabId;
    await vi.waitFor(() => expect(calls).toHaveLength(1));
    const copy = s().duplicateTab(a)!;
    s().activateTab(copy);
    answer(0);
    await flush();
    // La respuesta era de la pestaña original: la copia sigue sin contorno hasta tener el suyo.
    expect(outlineOf(s().tabs.find((e) => e.id === a)!.data!.doc, 't')).toBe(0);
    await vi.waitFor(() => expect(calls).toHaveLength(2));
    answer(1);
    await flush();
    expect(outlineOf(s().doc, 't')).toBeGreaterThan(0);
  });
});

describe('funciones OpenType y ejes', () => {
  const caps = (axes: boolean): FontCapabilities => ({
    features: [],
    axes: axes ? [{ tag: 'wght', name: 'Weight', min: 100, default: 400, max: 900 }] : [],
    instances: [],
  });

  it('al pasar a un texto de otra fuente no se ofrecen los ejes de la anterior mientras llegan los suyos', async () => {
    const slow = deferred<FontCapabilities>();
    const fontCapabilities = vi.fn(({ text }: { text: TextObject }) => (text.font_family === 'Helvetica Neue' ? Promise.resolve(caps(true)) : slow.promise));
    setApi({ ...original, fontCapabilities } as unknown as LaserApi);
    const noop = () => undefined;
    const view = await mount(<OpenTypeSection kind={textKind('Hola', { font_family: 'Helvetica Neue' })} family="Helvetica Neue" lineFont={false} onChange={noop} />);
    await act(async () => view.host.querySelector('summary')!.click());
    await vi.waitFor(() => expect(view.host.querySelector('input[type=range]')).not.toBeNull(), { timeout: 3000 });

    // Se selecciona un texto de otra pestaña con otra fuente (su respuesta tarda).
    await view.rerender(<OpenTypeSection kind={textKind('Otro', { font_family: 'Arial' })} family="Arial" lineFont={false} onChange={noop} />);
    await act(async () => void (await new Promise((r) => setTimeout(r, 300))));
    expect(fontCapabilities).toHaveBeenCalledTimes(2);
    expect(view.host.querySelector('input[type=range]')).toBeNull();
    expect(view.host.querySelector('.ot__loading')).not.toBeNull();

    await act(async () => slow.resolve(caps(false)));
    await flush();
    expect(view.host.querySelector('.ot__loading')).toBeNull();
    expect(view.host.querySelector('input[type=range]')).toBeNull();
  });

  it('mover un eje de la misma fuente sí conserva los controles mientras llega la respuesta', async () => {
    const second = deferred<FontCapabilities>();
    let n = 0;
    const fontCapabilities = vi.fn(() => (++n === 1 ? Promise.resolve(caps(true)) : second.promise));
    setApi({ ...original, fontCapabilities } as unknown as LaserApi);
    const noop = () => undefined;
    const k = (wght: number) => textKind('Hola', { font_family: 'Helvetica Neue', variations: [{ tag: 'wght', value: wght }] });
    const view = await mount(<OpenTypeSection kind={k(400)} family="Helvetica Neue" lineFont={false} onChange={noop} />);
    await act(async () => view.host.querySelector('summary')!.click());
    await vi.waitFor(() => expect(view.host.querySelector('input[type=range]')).not.toBeNull(), { timeout: 3000 });
    await view.rerender(<OpenTypeSection kind={k(700)} family="Helvetica Neue" lineFont={false} onChange={noop} />);
    await act(async () => void (await new Promise((r) => setTimeout(r, 300))));
    expect(fontCapabilities).toHaveBeenCalledTimes(2);
    expect(view.host.querySelector('input[type=range]')).not.toBeNull();
  });
});
