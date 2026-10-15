// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createMockApi } from '@/api/mock';
import { setApi } from '@/api';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import type { Document } from '@/api/types';
import { getLanguage, setLanguage } from '@/i18n';
import { tabViews, useStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { audit, cleanup, click, describeViolations, mount, press } from '@/test/dom';
import { resetTabs } from '@/test/tabs';
import { TabBar, TabPanel, TAB_PANEL_ID } from './TabBar';

const s = () => useStore.getState();
const docWith = (ids: string[]): Document => ({ ...emptyDocument(), objects: sampleObjects().filter((o) => ids.includes(o.id)), layers: sampleLayers() });

const tabs = () => [...document.querySelectorAll<HTMLElement>('[role="tab"]')];
const labels = () => tabs().map((t) => t.querySelector('.tab__name')!.textContent);

beforeEach(() => {
  setLanguage('es');
  setApi(createMockApi({ latencyMs: 0 }));
  resetTabs();
  s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
  s().openInTab(docWith(['obj-foto']), '/p/b.tlaku');
  s().newTab();
});

afterEach(async () => {
  await cleanup();
  setLanguage('es');
});

const ui = (
  <>
    <TabBar />
    <TabPanel>
      <p>lienzo</p>
    </TabPanel>
  </>
);

describe('barra de pestañas: estructura y accesibilidad', () => {
  it('es un tablist con una pestaña por documento, la activa seleccionada y una sola parada de Tab', async () => {
    await mount(ui);
    const list = document.querySelector('[role="tablist"]')!;
    expect(list.getAttribute('aria-label')).toBe('Documentos abiertos');
    expect(labels()).toEqual(['a.tlaku', 'b.tlaku', 'Sin título 1']);
    expect(tabs().map((t) => t.getAttribute('aria-selected'))).toEqual(['false', 'false', 'true']);
    expect(tabs().map((t) => t.tabIndex)).toEqual([-1, -1, 0]);
    // Cada pestaña controla el panel del diseño, que se rotula con la activa.
    const panel = document.getElementById(TAB_PANEL_ID)!;
    expect(panel.getAttribute('role')).toBe('tabpanel');
    expect(tabs().every((t) => t.getAttribute('aria-controls') === TAB_PANEL_ID)).toBe(true);
    expect(panel.getAttribute('aria-labelledby')).toBe(tabs()[2]!.id);
  });

  it('pasa axe (también con una pestaña sucia, otra con el trabajo en curso y el menú abierto)', async () => {
    const m = await mount(ui);
    s().activateTab(s().tabs[0]!.id);
    s().deleteObjects(['obj-placa']);
    s().noteJobOrigin({ tabId: s().tabs[1]!.id, name: 'b.tlaku', path: '/p/b.tlaku', preview: null });
    useStore.setState({ jobState: 'running' });
    await act(async () => {});
    expect(describeViolations(await audit(m.host))).toEqual([]);
    await act(async () => {
      tabs()[1]!.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 10, clientY: 10 }));
    });
    expect(document.querySelector('[role="menu"]')).not.toBeNull();
    expect(describeViolations(await audit(document.body))).toEqual([]);
    useStore.setState({ jobState: 'idle' });
    s().noteJobOrigin(null);
  });

  it('marca los cambios sin guardar (punto y texto para lectores) y el trabajo en curso (icono con texto)', async () => {
    await mount(ui);
    s().activateTab(s().tabs[0]!.id);
    s().deleteObjects(['obj-placa']);
    s().noteJobOrigin({ tabId: s().tabs[1]!.id, name: 'b.tlaku', path: '/p/b.tlaku', preview: null });
    useStore.setState({ jobState: 'running' });
    await act(async () => {});
    const [a, b, c] = tabs();
    expect(a!.querySelector('.tab__dirty')).not.toBeNull();
    expect(a!.textContent).toContain('con cambios sin guardar');
    expect(b!.querySelector('.tab__job')).not.toBeNull();
    expect(b!.textContent).toContain('Trabajo en curso desde esta pestaña');
    expect(c!.querySelector('.tab__dirty')).toBeNull();
    expect(c!.querySelector('.tab__job')).toBeNull();
    // Con el trabajo terminado la marca desaparece.
    useStore.setState({ jobState: 'finished' });
    await act(async () => {});
    expect(document.querySelector('.tab__job')).toBeNull();
    useStore.setState({ jobState: 'idle' });
    s().noteJobOrigin(null);
  });

  it('los textos salen en inglés con el idioma en inglés', async () => {
    setLanguage('en');
    await mount(ui);
    expect(document.querySelector('[role="tablist"]')!.getAttribute('aria-label')).toBe('Open documents');
    expect(labels()[2]).toBe('Untitled 1');
    expect(document.querySelector('.tabbar__new')!.getAttribute('aria-label')).toBe('New tab');
    expect(getLanguage()).toBe('en');
  });
});

describe('barra de pestañas: teclado y ratón', () => {
  it('← y → mueven el foco y activan (dando la vuelta); Inicio y Fin van a los extremos', async () => {
    await mount(ui);
    tabs()[2]!.focus();
    await press('ArrowRight');
    expect(s().activeTabId).toBe(s().tabs[0]!.id);
    expect(document.activeElement).toBe(tabs()[0]);
    await press('ArrowLeft');
    expect(s().activeTabId).toBe(s().tabs[2]!.id);
    await press('Home');
    expect(s().activeTabId).toBe(s().tabs[0]!.id);
    await press('End');
    expect(s().activeTabId).toBe(s().tabs[2]!.id);
    expect(document.activeElement).toBe(tabs()[2]);
  });

  it('clic activa; el botón + abre una pestaña nueva; la X cierra la pestaña', async () => {
    await mount(ui);
    await click(tabs()[0]!);
    expect(s().activeTabId).toBe(s().tabs[0]!.id);
    await click(document.querySelector('.tabbar__new')!);
    expect(s().tabs).toHaveLength(4);
    expect(labels()[3]).toBe('Sin título 2');
    await click(tabs()[3]!.querySelector('.tab__close')!);
    expect(s().tabs).toHaveLength(3);
  });

  it('Supr cierra la pestaña enfocada; con cambios pregunta antes', async () => {
    const { usePrompts } = await import('@/features/history/prompt');
    await mount(ui);
    tabs()[2]!.focus();
    await press('Delete');
    expect(s().tabs).toHaveLength(2);
    s().activateTab(s().tabs[0]!.id);
    s().deleteObjects(['obj-placa']);
    await act(async () => {});
    tabs()[0]!.focus();
    await press('Delete');
    await act(async () => {});
    expect(usePrompts.getState().current?.title).toBe('Cambios sin guardar');
    await act(async () => usePrompts.getState().answer('cancel'));
    expect(s().tabs).toHaveLength(2);
  });

  it('arrastrar una pestaña sobre otra las reordena; un simple clic no', async () => {
    await mount(ui);
    const slots = [...document.querySelectorAll<HTMLElement>('[data-tab-slot]')];
    slots.forEach((el, i) => {
      el.getBoundingClientRect = () => ({ left: i * 100, right: i * 100 + 100, top: 0, bottom: 30, width: 100, height: 30, x: i * 100, y: 0, toJSON: () => ({}) });
    });
    const first = s().tabs[0]!.id;
    const down = new MouseEvent('pointerdown', { bubbles: true, button: 0, clientX: 50 });
    await act(async () => tabs()[0]!.dispatchEvent(down));
    await act(async () => window.dispatchEvent(new MouseEvent('pointermove', { clientX: 52 })));
    expect(s().tabs[0]!.id).toBe(first); // por debajo del umbral: sigue siendo un clic
    await act(async () => window.dispatchEvent(new MouseEvent('pointermove', { clientX: 250 })));
    expect(s().tabs[2]!.id).toBe(first);
    await act(async () => window.dispatchEvent(new MouseEvent('pointerup', {})));
    expect(labels()).toEqual(['b.tlaku', 'Sin título 1', 'a.tlaku']);
    // La activa sigue siendo la misma (reordenar no cambia de pestaña).
    expect(s().activeTabId).toBe(s().tabs[1]!.id);
  });
});

describe('menú de la pestaña', () => {
  const open = async (i: number) => {
    await act(async () => {
      tabs()[i]!.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 20, clientY: 20 }));
    });
  };
  const items = () => [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')];
  const item = (text: string) => items().find((e) => e.textContent === text)!;

  it('lista cerrar, cerrar las demás, duplicar, mover y mover a ventana nueva', async () => {
    await mount(ui);
    await open(1);
    expect(items().map((e) => e.textContent)).toEqual([
      'Cerrar pestaña',
      'Cerrar las demás pestañas',
      'Duplicar pestaña',
      'Mover a la izquierda',
      'Mover a la derecha',
      'Mover a una ventana nueva',
    ]);
    expect(document.querySelector('[role="menu"]')!.getAttribute('aria-label')).toContain('b.tlaku');
  });

  it('«Mover a la derecha» reordena y cierra el menú; en el extremo está deshabilitado', async () => {
    await mount(ui);
    await open(0);
    expect(item('Mover a la izquierda').getAttribute('aria-disabled')).toBe('true');
    await click(item('Mover a la derecha'));
    expect(labels()).toEqual(['b.tlaku', 'a.tlaku', 'Sin título 1']);
    expect(document.querySelector('[role="menu"]')).toBeNull();
  });

  it('«Duplicar» abre una copia sin archivo y con cambios', async () => {
    await mount(ui);
    await open(0);
    await click(item('Duplicar pestaña'));
    expect(s().tabs).toHaveLength(4);
    expect(s().filePath).toBeNull();
    expect(s().dirty).toBe(true);
  });

  it('«Cerrar las demás» deja solo la pestaña del menú', async () => {
    await mount(ui);
    await open(1);
    await click(item('Cerrar las demás pestañas'));
    await act(async () => {});
    expect(tabViews(s()).map((v) => v.filePath)).toEqual(['/p/b.tlaku']);
  });

  it('Esc cierra el menú y devuelve el foco a la pestaña', async () => {
    await mount(ui);
    await open(1);
    await act(async () => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    });
    expect(document.querySelector('[role="menu"]')).toBeNull();
    expect(document.activeElement).toBe(tabs()[1]);
  });

  it('con una sola pestaña, cerrar las demás y mover a ventana nueva están deshabilitados con su motivo', async () => {
    await mount(ui);
    await act(async () => {
      s().removeTab(s().tabs[0]!.id);
      s().removeTab(s().tabs[0]!.id);
    });
    await open(0);
    expect(item('Cerrar las demás pestañas').getAttribute('aria-disabled')).toBe('true');
    expect(item('Mover a una ventana nueva').title).toBe('Solo hay una pestaña abierta.');
  });
});
