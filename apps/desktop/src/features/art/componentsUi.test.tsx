// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { DesignObject } from '@/api/types';
import { commands } from '@/commands';
import { buildContextMenu } from '@/features/contextmenu/model';
import { usePrompts } from '@/features/history/prompt';
import { setLanguage } from '@/i18n';
import { polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { useStore } from '@/store';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { useArtPanel } from './artStore';
import { ComponentsTab } from './ComponentsTab';
import { loadUserComponents, saveSelectionAsComponent } from './componentActions';
import { SaveComponentHost } from './SaveComponentDialog';

const square = (id: string): DesignObject => ({
  id,
  layer: '#ff0000',
  transform: [1, 0, 0, 1, 100, 100],
  kind: { type: 'vector', paths: [polylineBez([[0, 0], [10, 0], [10, 10], [0, 10]], true)] },
  locked: false,
  hidden: false,
  group: null,
});

const s = () => useStore.getState();

beforeEach(async () => {
  setLanguage('es');
  setApi(createMockApi({ latencyMs: 0 }));
  useArtPanel.setState({ components: [], saving: null, placement: 'auto', error: null, category: 'componentes', query: '' });
  useStore.setState({ notices: [] });
  s().newDocument();
  await s().loadDevices();
  s().applyImport({ objects: [square('a'), square('b')], new_layers: [makeLayer('#ff0000', 'Corte', { speed: 777 })], warnings: [] });
});

afterEach(async () => {
  await cleanup();
  setLanguage('es');
});

const buttons = () => [...document.querySelectorAll<HTMLButtonElement>('button')];
const byLabel = (text: string) => buttons().find((b) => (b.getAttribute('aria-label') ?? b.textContent ?? '').includes(text));

describe('Mis componentes: menú y comando', () => {
  it('el menú contextual de una selección ofrece «Guardar como componente…» y el del fondo no', () => {
    s().select(['a']);
    const hooks = { editText: () => undefined, textToPaths: () => undefined, has: (id: string) => commands.has(id) };
    const onObject = buildContextMenu(s(), true, hooks);
    expect(onObject.some((e) => e.kind === 'command' && e.id === 'tools.saveComponent')).toBe(true);
    const onBackground = buildContextMenu(s(), false, hooks);
    expect(onBackground.some((e) => e.kind === 'command' && e.id === 'tools.saveComponent')).toBe(false);
  });

  it('sin selección el comando no está disponible; con selección abre el diálogo', async () => {
    s().select([]);
    expect(commands.availability('tools.saveComponent')).not.toBe(true);
    s().select(['a', 'b']);
    expect(commands.availability('tools.saveComponent')).toBe(true);
    await commands.run('tools.saveComponent');
    expect(useArtPanel.getState().saving).toEqual(['a', 'b']);
  });
});

describe('Mis componentes: interfaz', () => {
  it('el diálogo pide un nombre y guarda la selección con sus ajustes', async () => {
    useArtPanel.getState().setSaving(['a', 'b']);
    await mount(<SaveComponentHost />);
    const input = document.querySelector<HTMLInputElement>('input.field__input')!;
    expect(input.value).toBe('Componente');
    expect(document.body.textContent).toContain('2 objeto(s)');
    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!;
      setter.call(input, 'Hoja 60x60');
      input.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await click(byLabel('Guardar')!);
    // Deja que termine el guardado asíncrono.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });
    expect(useArtPanel.getState().saving).toBeNull();
    const [saved] = useArtPanel.getState().components;
    expect(saved?.name).toBe('Hoja 60x60');
    expect(saved?.data.layers[0]).toMatchObject({ speed: 777 });
  });

  it('lista cada componente con miniatura, nombre y tamaño; clic inserta; borrar pide confirmación', async () => {
    await saveSelectionAsComponent('Hoja 60x60', ['a', 'b']);
    s().newDocument();
    s().select([]);
    await mount(<ComponentsTab query="" />);
    const text = document.body.textContent ?? '';
    expect(text).toContain('Hoja 60x60');
    expect(text).toContain('10 × 10 mm');
    expect(text).toContain('2 objetos');
    expect(document.querySelector('img.art-comp__img')?.getAttribute('src')).toContain('data:image/svg+xml');

    // Sin selección, guardar no está disponible.
    expect(byLabel('Guardar selección como componente')!.disabled).toBe(true);

    // Un clic (sin arrastrar) inserta en su posición original.
    const main = document.querySelector<HTMLButtonElement>('button.art-comp__main')!;
    await act(async () => {
      main.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });
    expect(s().doc.objects).toHaveLength(2);
    expect(s().doc.objects[0]!.transform[4]).toBe(100);
    expect(s().doc.layers.find((l) => l.color === '#ff0000')).toMatchObject({ speed: 777 });

    // Borrar: confirma antes.
    await click(byLabel('Borrar «Hoja 60x60»')!);
    expect(usePrompts.getState().current?.message).toContain('Hoja 60x60');
    await act(async () => {
      usePrompts.getState().answer('delete');
      await new Promise((r) => setTimeout(r, 10));
    });
    expect(useArtPanel.getState().components).toEqual([]);
    expect(document.body.textContent).toContain('Todavía no guardaste componentes');
  });

  it('renombrar cambia el nombre en la lista', async () => {
    await saveSelectionAsComponent('uno', ['a']);
    await mount(<ComponentsTab query="" />);
    await click(byLabel('Renombrar «uno»')!);
    const input = document.querySelector<HTMLInputElement>('input.art-comp__rename')!;
    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!;
      setter.call(input, 'panel');
      input.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await act(async () => {
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
      await new Promise((r) => setTimeout(r, 10));
    });
    await loadUserComponents();
    expect(useArtPanel.getState().components.map((c) => c.name)).toEqual(['panel']);
  });

  it('es accesible', async () => {
    await saveSelectionAsComponent('Hoja', ['a', 'b']);
    s().select(['a']);
    await mount(<ComponentsTab query="" />);
    const r = await audit();
    expect(describeViolations(r)).toEqual([]);
  });
});
