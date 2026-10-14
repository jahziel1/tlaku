// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DesignObject } from '@/api/types';
import { commands } from '@/commands';
import { setLanguage } from '@/i18n';
import { polylineBez } from '@/lib/bezpath';
import { DEFAULT_DOCK_LAYOUT, floatDockTab } from '@/lib/dockLayout';
import { makeLayer } from '@/lib/layers';
import { useSimple } from '@/features/simple/store';
import { useStore } from '@/store';
import { audit, cleanup, click, describeViolations, mount, press } from '@/test/dom';
import { TABS } from './dockPanels';
import { AppShell } from './AppShell';

// Montar la ventana entera es lento si el equipo está ocupado.
vi.setConfig({ testTimeout: 30000 });

// jsdom no trae estas APIs del navegador.
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

const rect = (id: string): DesignObject => ({
  id, layer: '#ff3b4a', transform: [1, 0, 0, 1, 10, 20], locked: false, hidden: false, group: null,
  kind: { type: 'vector', paths: [polylineBez([[0, 0], [30, 0], [30, 10], [0, 10]], true)] },
});

/** Cierra lo que haya quedado abierto en los stores (viven fuera del árbol y pasan de una prueba a otra). */
async function closeEverything(): Promise<void> {
  useStore.setState({ dialog: null });
  const m = await mount(<AppShell />);
  for (let i = 0; i < 5; i++) {
    const open = document.querySelector('[role="dialog"][aria-modal="true"]');
    if (!open) break;
    await press('Escape', {}, open);
    await settle(20);
  }
  await m.unmount();
}

/**
 * Monta la ventana. `SettingsHost` lee las preferencias de forma asíncrona y puede pisar las que
 * una prueba fijó antes de montar: se espera a que termine y se vuelven a aplicar.
 */
async function mountShell(prefs: Partial<Record<'uiMode' | 'dockLayout', unknown>> = {}) {
  const m = await mount(<AppShell />);
  await settle(80);
  await act(async () => {
    const st = useStore.getState();
    st.setPref('language', 'es');
    st.setPref('uiMode', (prefs.uiMode as 'full' | 'simple' | undefined) ?? 'full');
    st.setPref('dockLayout', (prefs.dockLayout as typeof DEFAULT_DOCK_LAYOUT | undefined) ?? DEFAULT_DOCK_LAYOUT);
  });
  return m;
}

const settle = (ms = 0) => act(async () => { await new Promise((r) => setTimeout(r, ms)); });

beforeEach(() => {
  useStore.getState().setPref('dockLayout', DEFAULT_DOCK_LAYOUT);
  useStore.getState().setPref('uiMode', 'full');
  useStore.getState().setPref('language', 'es');
  setLanguage('es');
});
afterEach(cleanup);

describe('ventana principal: axe, orden de tabulación y teclado', () => {
  it('sin violaciones en cada pestaña del panel lateral y con un panel flotante', { timeout: 60000 }, async () => {
    const m = await mountShell();
    for (const tab of TABS) {
      await act(async () => useStore.getState().setDockTab(tab.id));
      expect(describeViolations(await audit(m.host)), `pestaña ${tab.id}`).toEqual([]);
    }
    await act(async () => useStore.getState().setPref('dockLayout', floatDockTab(DEFAULT_DOCK_LAYOUT, 'history')));
    expect(m.host.querySelector('.floatwin')).not.toBeNull();
    expect(describeViolations(await audit(m.host))).toEqual([]);
    await m.unmount();
  });

  it('el enlace «Saltar al lienzo» es lo primero y lleva el foco al lienzo', async () => {
    const m = await mountShell();
    const link = m.host.querySelector<HTMLAnchorElement>('a.skip-link')!;
    expect(link).toBe(m.host.querySelector('.shell')!.firstElementChild);
    await click(link);
    expect(document.activeElement).toBe(document.getElementById('canvas-main'));
    await m.unmount();
  });

  it('la paleta de herramientas y las barras de la cabecera tienen una sola parada de Tab y flechas', async () => {
    const m = await mountShell();
    for (const bar of m.host.querySelectorAll<HTMLElement>('[role="toolbar"]')) {
      const stops = [...bar.querySelectorAll<HTMLElement>('button')].filter((b) => b.tabIndex === 0);
      expect(stops, bar.getAttribute('aria-label') ?? '').toHaveLength(1);
    }
    const tools = m.host.querySelector<HTMLElement>('.toolpalette')!;
    const buttons = [...tools.querySelectorAll<HTMLElement>('button')];
    buttons[0]!.focus();
    await press('ArrowDown');
    expect(document.activeElement).toBe(buttons[1]);
    await m.unmount();
  });

  it('las pestañas del panel son un tablist: flechas, Inicio y Fin cambian de pestaña', async () => {
    const m = await mountShell();
    const tabs = [...m.host.querySelectorAll<HTMLElement>('.dock [role="tab"]')];
    expect(tabs.length).toBe(TABS.length);
    tabs[0]!.focus();
    await act(async () => useStore.getState().setDockTab('layers'));
    await press('ArrowRight');
    await settle(30);
    expect(useStore.getState().dockTab).toBe('machine');
    await press('End');
    await settle(30);
    expect(useStore.getState().dockTab).toBe('notes');
    await m.unmount();
  });

  it('el panel flotante se mueve con las flechas y tiene nombre y descripción', async () => {
    const m = await mountShell({ dockLayout: floatDockTab(DEFAULT_DOCK_LAYOUT, 'history') });
    const head = m.host.querySelector<HTMLElement>('.floatwin__head')!;
    expect(head.getAttribute('aria-label')).toBeTruthy();
    expect(document.getElementById(head.getAttribute('aria-describedby')!)?.textContent).toBeTruthy();
    const before = useStore.getState().prefs.dockLayout.floating[0]!.x;
    head.focus();
    await press('ArrowRight');
    expect(useStore.getState().prefs.dockLayout.floating[0]!.x).toBeGreaterThan(before);
    await m.unmount();
  });

  it('el menú Archivo se abre con ↓, se recorre con flechas y Esc devuelve el foco al menú', async () => {
    const m = await mountShell();
    const trigger = m.host.querySelector<HTMLElement>('.menubar__trigger')!;
    trigger.focus();
    await press('ArrowDown');
    const menu = m.host.querySelector('[role="menu"]')!;
    const items = [...menu.querySelectorAll<HTMLElement>('[role^="menuitem"]')];
    expect(document.activeElement).toBe(items[0]);
    await press('ArrowDown');
    expect(document.activeElement).toBe(items[1]);
    await press('End');
    expect(document.activeElement).toBe(items[items.length - 1]);
    expect(describeViolations(await audit(m.host))).toEqual([]);
    await press('Escape');
    expect(m.host.querySelector('[role="menu"]')).toBeNull();
    expect(document.activeElement).toBe(trigger);
    await m.unmount();
  });

  it('los diálogos de preferencias y de atajos pasan axe, atrapan el foco y devuelven el foco con Esc', { timeout: 60000 }, async () => {
    const m = await mountShell();
    for (const id of ['prefs.open', 'help.shortcuts', 'help.about']) {
      const opener = m.host.querySelector<HTMLElement>('.menubar__trigger')!;
      opener.focus();
      await act(async () => {
        void commands.run(id);
        await new Promise((r) => setTimeout(r, 30));
      });
      const dialog = document.querySelector('[role="dialog"][aria-modal="true"]')!;
      expect(dialog, id).not.toBeNull();
      expect(dialog.contains(document.activeElement), `${id}: foco dentro`).toBe(true);
      expect(describeViolations(await audit(document.body)), id).toEqual([]);
      await press('Escape');
      await settle(30);
      expect(document.querySelector('[role="dialog"][aria-modal="true"]'), `${id}: Esc cierra`).toBeNull();
      expect(document.activeElement, `${id}: foco vuelve`).toBe(opener);
    }
    await m.unmount();
  });

  it('el modo sencillo pasa axe en sus cinco pasos y el foco pasa al título al cambiar de paso', { timeout: 60000 }, async () => {
    useStore.getState().applyImport({ objects: [rect('a')], new_layers: [makeLayer('#ff3b4a', 'Rojo')], warnings: [] });
    const m = await mountShell({ uiMode: 'simple' });
    expect(m.host.querySelector('.shell--simple')).not.toBeNull();
    for (let step = 0; step < 5; step++) {
      await act(async () => useSimple.getState().setStep(step));
      expect(describeViolations(await audit(m.host)), `paso ${step + 1}`).toEqual([]);
      if (step > 0) expect(document.activeElement).toBe(m.host.querySelector('#simple-title'));
    }
    await act(async () => useSimple.getState().setStep(0));
    await m.unmount();
  });
});

describe('más superficies: avisos y diálogos de otras áreas', () => {
  it('los avisos de los cuatro niveles pasan axe y los urgentes son role="alert"', async () => {
    const m = await mountShell();
    await act(async () => {
      const st = useStore.getState();
      st.notify('info', 'Información');
      st.notify('success', 'Listo');
      st.notify('warning', 'Cuidado', ['uno', 'dos']);
      st.notify('error', 'Falló');
    });
    expect(m.host.querySelectorAll('.toast[role="alert"]')).toHaveLength(2);
    expect(m.host.querySelectorAll('.toast[role="status"]')).toHaveLength(2);
    expect(describeViolations(await audit(m.host))).toEqual([]);
    await m.unmount();
  });

  const DIALOGS = ['machine.devices', 'machine.addDevice', 'tools.materialTest', 'arrange.grid', 'arrange.alignDialog', 'view.guidesDialog', 'file.newFromTemplate', 'bundle.export', 'tools.insertCode', 'tools.variableText', 'tools.traceBatch', 'help.manual', 'help.report', 'help.shareMachine', 'help.shareMaterial', 'help.jobLog', 'camera.calibrate', 'fileio.print'];
  for (const id of DIALOGS) {
    it(`diálogo de «${id}»: axe y foco dentro`, { timeout: 30000 }, async () => {
      const m = await mountShell();
      // Algunos comandos piden objetos seleccionados.
      await act(async () => {
        useStore.getState().applyImport({ objects: [rect('a'), rect('b')], new_layers: [makeLayer('#ff3b4a', 'Rojo')], warnings: [] });
        useStore.getState().select(['a', 'b']);
      });
      await act(async () => {
        void commands.run(id);
        await new Promise((r) => setTimeout(r, 30));
      });
      const dialog = document.querySelector('[role="dialog"][aria-modal="true"]');
      expect(dialog, 'se abre un diálogo').not.toBeNull();
      expect(dialog!.contains(document.activeElement), 'el foco entra al diálogo').toBe(true);
      expect(describeViolations(await audit(document.body))).toEqual([]);
      await m.unmount();
      await closeEverything();
    });
  }
});

describe('lectores de pantalla: lienzo, selección y trabajo', () => {
  it('el lienzo tiene descripción y el resumen de la selección se anuncia (cantidad, tamaño y posición)', async () => {
    const m = await mountShell();
    const canvas = m.host.querySelector('#canvas-main')!;
    expect(document.getElementById(canvas.getAttribute('aria-describedby')!)?.textContent).toMatch(/\S/);
    const live = () => m.host.querySelector('.canvas-area [role="status"]')!.textContent;
    await act(async () => {
      useStore.getState().applyImport({ objects: [rect('a')], new_layers: [makeLayer('#ff3b4a', 'Rojo')], warnings: [] });
      useStore.getState().select(['a']);
    });
    await settle(800);
    expect(live()).toMatch(/1 objeto seleccionado.*30[.,]00 por 10[.,]00 mm.*X 10[.,]00, Y 20[.,]00/);
    await act(async () => useStore.getState().select([]));
    await settle(800);
    expect(live()).toBe('Nada seleccionado.');
    await m.unmount();
  });

  it('el estado del trabajo y su avance por tramos se anuncian en una región viva', async () => {
    const m = await mountShell();
    const regions = () => [...m.host.querySelectorAll('.shell > [role="status"].sr-only, .shell > [role="alert"].sr-only')].map((r) => r.textContent);
    await act(async () => useStore.setState({ jobState: 'running', progress: { acked: 0, total: 100 } as never }));
    expect(regions()).toContain('Trabajo en marcha.');
    await act(async () => useStore.setState({ progress: { acked: 30, total: 100 } as never }));
    expect(regions()).toContain('Trabajo al 25 %.');
    await act(async () => useStore.setState({ jobState: 'error' }));
    expect(regions()).toContain('El trabajo se detuvo por un error.');
    await act(async () => useStore.setState({ jobState: 'idle', progress: null }));
    await m.unmount();
  });
});
