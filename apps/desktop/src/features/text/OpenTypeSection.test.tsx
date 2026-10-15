// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { TextObject } from '@/api/types';
import { setLanguage } from '@/i18n';
import { audit, cleanup, click, mount } from '@/test/dom';
import { clearCapabilitiesCache } from './capabilities';
import { OpenTypeSection } from './OpenTypeSection';

const kind = (patch: Partial<TextObject> = {}): TextObject => ({
  text: 'Hola', font_family: 'Helvetica Neue', bold: false, italic: false, size_mm: 10, letter_spacing_mm: 0,
  line_spacing: 1.2, align: 'left', on_path: null, outline: [], ...patch,
});

type Patch = [Partial<TextObject>, string];

async function open(k: TextObject, opts: { lineFont?: boolean } = {}) {
  const changes: Patch[] = [];
  const view = await mount(
    <OpenTypeSection kind={k} family={k.font_family} lineFont={opts.lineFont ?? false} onChange={(p, f) => void changes.push([p, f])} />,
  );
  await click(view.host.querySelector('summary')!);
  return { view, changes };
}

/** Espera a que lleguen las capacidades (debounce + latencia del mock). */
const loaded = (host: HTMLElement) => vi.waitFor(() => expect(host.querySelector('.ot__list')).not.toBeNull(), { timeout: 3000 });
const pause = (ms: number) => act(async () => void (await new Promise((r) => setTimeout(r, ms))));
const rowOf = (host: HTMLElement, tag: string) => [...host.querySelectorAll('.ot-row')].find((r) => r.querySelector('code')?.textContent === tag)!;
const boxOf = (host: HTMLElement, tag: string) => rowOf(host, tag).querySelector('input') as HTMLInputElement;

beforeEach(() => {
  setLanguage('es');
  clearCapabilitiesCache();
  setApi(createMockApi({ latencyMs: 0 }));
});
afterEach(cleanup);

describe('OpenTypeSection', () => {
  it('cerrada no pide nada; al abrir lista solo lo que trae la fuente', async () => {
    const api = createMockApi({ latencyMs: 0 });
    const spy = vi.spyOn(api, 'fontCapabilities');
    setApi(api);
    const view = await mount(<OpenTypeSection kind={kind()} family="Helvetica Neue" lineFont={false} onChange={() => {}} />);
    await pause(300);
    expect(spy).not.toHaveBeenCalled();
    await click(view.host.querySelector('summary')!);
    await loaded(view.host);
    expect(spy).toHaveBeenCalledTimes(1);
    const names = [...view.host.querySelectorAll('.ot-row__name')].map((n) => n.textContent);
    expect(names).toEqual(expect.arrayContaining(['Ajuste de pares (kerning)', 'Ligaduras estándar']));
    // Helvetica Neue (simulada) no trae versalitas: no se ofrecen.
    expect(names).not.toContain('Versalitas');
    // Cada función lleva su vista previa apagada y encendida.
    expect(rowOf(view.host, 'kern').querySelectorAll('svg.ot-preview')).toHaveLength(2);
  });

  it('marcar una casilla manda solo la diferencia con la fuente', async () => {
    const { view, changes } = await open(kind());
    await loaded(view.host);
    expect(boxOf(view.host, 'ss01').checked).toBe(false);
    await click(boxOf(view.host, 'ss01'));
    expect(changes.at(-1)).toEqual([{ features: [{ tag: 'ss01', value: 1 }] }, 'ot-ss01']);
    // El kerning viene encendido: apagarlo sí se guarda.
    expect(boxOf(view.host, 'kern').checked).toBe(true);
    await click(boxOf(view.host, 'kern'));
    expect(changes.at(-1)).toEqual([{ features: [{ tag: 'kern', value: 0 }] }, 'ot-kern']);
  });

  it('refleja lo guardado en el objeto y permite quitar los cambios', async () => {
    const k = kind({ features: [{ tag: 'liga', value: 0 }, { tag: 'ss02', value: 1 }] });
    const { view, changes } = await open(k);
    await loaded(view.host);
    expect(boxOf(view.host, 'liga').checked).toBe(false);
    expect(boxOf(view.host, 'ss02').checked).toBe(true);
    expect(view.host.querySelector('.ot__count')?.textContent).toBe('2 activas');
    await click([...view.host.querySelectorAll('button')].find((b) => b.textContent === 'Quitar cambios')!);
    expect(changes.at(-1)).toEqual([{ features: undefined }, 'ot-reset']);
  });

  it('una fuente variable muestra un deslizador por eje y estilos con nombre', async () => {
    const { view, changes } = await open(kind());
    await loaded(view.host);
    const sliders = [...view.host.querySelectorAll('input[type=range]')] as HTMLInputElement[];
    expect(sliders.map((s) => s.id)).toEqual(['ot-axis-wght', 'ot-axis-wdth']);
    expect(sliders[0]!.min).toBe('100');
    expect(sliders[0]!.max).toBe('900');
    expect(view.host.querySelector('label[for=ot-axis-wght]')?.textContent).toContain('Peso: 400');
    // Mover el deslizador guarda el eje; el valor por omisión no deja rastro.
    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!;
      setter.call(sliders[0], '700');
      sliders[0]!.dispatchEvent(new Event('input', { bubbles: true }));
    });
    expect(changes.at(-1)).toEqual([{ variations: [{ tag: 'wght', value: 700 }] }, 'ot-axis-wght']);
    const select = view.host.querySelector('#ot-instance') as HTMLSelectElement;
    expect([...select.options].map((o) => o.value)).toEqual(['Regular', 'Bold Condensed']);
  });

  it('una fuente sin ejes no muestra deslizadores y una de una línea no pide nada', async () => {
    const plain = await open(kind({ font_family: 'Arial' }));
    await loaded(plain.view.host);
    expect(plain.view.host.querySelector('input[type=range]')).toBeNull();
    await cleanup();

    const api = createMockApi({ latencyMs: 0 });
    const spy = vi.spyOn(api, 'fontCapabilities');
    setApi(api);
    const line = await open(kind({ font_family: 'Hershey Roman Simplex' }), { lineFont: true });
    await pause(300);
    expect(spy).not.toHaveBeenCalled();
    expect(line.view.host.textContent).toContain('no tienen funciones OpenType');
  });

  it('muestra los textos en inglés y no tiene problemas de accesibilidad', async () => {
    setLanguage('en');
    const { view } = await open(kind());
    await loaded(view.host);
    expect(view.host.textContent).toContain('Font features and axes');
    expect(view.host.textContent).toContain('Standard ligatures');
    expect(view.host.textContent).toContain('Variable font');
    const result = await audit(view.host);
    expect(result.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.html).join(' | ')}`)).toEqual([]);
  });
});
