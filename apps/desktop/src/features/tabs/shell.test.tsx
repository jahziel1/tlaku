// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { commands } from '@/commands';
import { useShortcuts } from '@/commands/useShortcuts';
import { AppShell } from '@/features/shell/AppShell';
import { setLanguage } from '@/i18n';
import { useStore } from '@/store';
import { cleanup, mount } from '@/test/dom';
import { resetTabs } from '@/test/tabs';
import { resetTabsHostForTests } from './TabsHost';

vi.setConfig({ testTimeout: 30000 });

class NoopObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= NoopObserver as unknown as typeof ResizeObserver;
window.matchMedia ??= ((query: string) => ({ matches: false, media: query, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {} })) as unknown as typeof window.matchMedia;
Element.prototype.scrollTo ??= () => {};
Element.prototype.scrollIntoView ??= () => {};
HTMLCanvasElement.prototype.getContext = (() => null) as unknown as HTMLCanvasElement['getContext'];

const s = () => useStore.getState();
const settle = (ms = 0) => act(async () => { await new Promise((r) => setTimeout(r, ms)); });

/** La app con sus atajos globales (como `App.tsx`). */
function Root() {
  useShortcuts(commands);
  return <AppShell />;
}

async function mountApp(uiMode: 'full' | 'simple' = 'full') {
  const m = await mount(<Root />);
  await settle(80);
  await act(async () => {
    s().setPref('language', 'es');
    s().setPref('uiMode', uiMode);
  });
  return m;
}

const key = (init: KeyboardEventInit & { key: string; code: string }) =>
  act(async () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }));
  });
const names = () => [...document.querySelectorAll('.tabbar [role="tab"] .tab__name')].map((e) => e.textContent);

beforeEach(() => {
  localStorage.clear();
  resetTabsHostForTests();
  resetTabs();
  setLanguage('es');
});
afterEach(cleanup);

describe('pestañas dentro de la ventana', () => {
  it('la barra va encima del lienzo y el lienzo es el tabpanel de la pestaña activa', async () => {
    await mountApp();
    const bar = document.querySelector('.tabbar')!;
    const panel = document.querySelector('[role="tabpanel"]')!;
    expect(bar).not.toBeNull();
    expect(panel.querySelector('.canvas-view')).not.toBeNull();
    // La barra queda antes que el lienzo en el mismo contenedor.
    expect(bar.parentElement).toBe(panel.parentElement);
    expect(bar.compareDocumentPosition(panel) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(names()).toEqual(['Sin título 1']);
  });

  it('el título de la ventana y el nombre de la barra superior siguen a la pestaña activa', async () => {
    await mountApp();
    expect(document.title).toBe('Sin título — Tlaku');
    await act(async () => s().newTab());
    await settle(20);
    expect(document.title).toBe('Sin título 2 — Tlaku');
    expect(document.querySelector('.docname__text')!.textContent).toBe('Sin título 2');
    await act(async () => {
      s().openInTab({ version: 2, objects: [], layers: [], device_id: null }, '/p/mesa.tlaku');
    });
    await settle(20);
    // La pestaña activa estaba vacía: se reemplazó, y el título pasa a ser el del archivo.
    expect(document.title).toBe('mesa.tlaku — Tlaku');
    await act(async () => s().newTab());
    await settle(20);
    expect(document.title).toBe('Sin título 2 — Tlaku');
  });

  it('modo sencillo: la barra solo aparece cuando hay varias pestañas, y el nombre de la barra es el de la activa', async () => {
    await mountApp('simple');
    expect(document.querySelector('.tabbar')).toBeNull();
    await act(async () => s().newTab());
    await settle(20);
    expect(document.querySelector('.tabbar')).not.toBeNull();
    expect(document.querySelector('.docname__text')!.textContent).toBe('Sin título 2');
    await act(async () => s().removeTab(s().activeTabId));
    await settle(20);
    expect(document.querySelector('.tabbar')).toBeNull();
  });

  it('Ctrl+T, Ctrl+Tab, Ctrl+Mayús+Tab y Ctrl+W con el sistema de atajos de la app', async () => {
    await mountApp();
    await key({ key: 't', code: 'KeyT', ctrlKey: true });
    expect(names()).toEqual(['Sin título 1', 'Sin título 2']);
    expect(s().activeTabId).toBe(s().tabs[1]!.id);
    await key({ key: 'Tab', code: 'Tab', ctrlKey: true });
    expect(s().activeTabId).toBe(s().tabs[0]!.id);
    await key({ key: 'Tab', code: 'Tab', ctrlKey: true, shiftKey: true });
    expect(s().activeTabId).toBe(s().tabs[1]!.id);
    await key({ key: 'w', code: 'KeyW', ctrlKey: true });
    await settle(20);
    expect(names()).toEqual(['Sin título 1']);
  });

  it('el botón de la barra (+) y el menú Archivo llevan los atajos en su ayuda', async () => {
    await mountApp();
    expect(document.querySelector('.tabbar__new')!.getAttribute('title')).toBe('Nueva pestaña (Ctrl+T)');
    expect(commands.shortcutLabel('tabs.close', false)).toBe('Ctrl+W');
    expect(commands.shortcutLabel('tabs.next', false)).toBe('Ctrl+Tab');
    expect(commands.shortcutLabel('tabs.prev', false)).toBe('Ctrl+Mayús+Tab');
  });

  it('la barra de Vista previa / Encuadrar / Iniciar dice de qué pestaña es solo cuando hay varias', async () => {
    await mountApp();
    const label = () => document.querySelector('.topbar__group--actions')!.getAttribute('aria-label');
    expect(label()).toBe('Trabajo');
    await act(async () => s().newTab());
    await settle(20);
    expect(label()).toBe('Trabajo de «Sin título 2»');
  });
});
