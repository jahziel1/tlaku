import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { commands } from '@/commands';
import { floatDockTab, setDockSide, setToolsSide, DEFAULT_DOCK_LAYOUT } from '@/lib/dockLayout';
import { useStore } from '@/store';
import { shellGrid } from './AppShell';
import { Dock } from './Dock';
import { FloatingPanels } from './FloatingPanels';
import { TABS } from './dockPanels';

beforeEach(() => useStore.getState().setPref('dockLayout', DEFAULT_DOCK_LAYOUT));

describe('distribución de la ventana', () => {
  it('de fábrica: herramientas a la izquierda, lienzo y panel a la derecha', () => {
    const g = shellGrid(DEFAULT_DOCK_LAYOUT);
    expect(g.gridTemplateColumns).toBe('var(--tools-w) minmax(0, 1fr) auto');
    expect(g.gridTemplateAreas).toContain("'tools main dock'");
  });

  it('el panel a la izquierda y las herramientas a la derecha cambian el orden de las columnas', () => {
    const g = shellGrid(setToolsSide(setDockSide(DEFAULT_DOCK_LAYOUT, 'left'), 'right'));
    expect(g.gridTemplateColumns).toBe('auto minmax(0, 1fr) var(--tools-w)');
    expect(g.gridTemplateAreas).toContain("'dock main tools'");
    expect(g.gridTemplateAreas).toContain("'top top top'");
    expect(g.gridTemplateAreas).toContain("'status status status'");
  });

  it('todas las pestañas del panel tienen su definición (nombre, ícono y comando)', () => {
    expect(TABS.map((t) => t.id)).toEqual(['layers', 'machine', 'console', 'history', 'camera', 'materials', 'notes']);
    for (const t of TABS) expect(commands.has(t.cmd), t.cmd).toBe(true);
  });

  it('sin paneles flotantes no se pinta nada, y el panel acoplado sale en su borde', () => {
    expect(renderToStaticMarkup(<FloatingPanels />)).toBe('');
    expect(renderToStaticMarkup(<Dock />)).not.toContain('dock--left');
  });

  it('«Restablecer distribución» devuelve bordes y paneles a lo de fábrica', async () => {
    const s = useStore.getState();
    s.setPref('dockLayout', floatDockTab(setDockSide(DEFAULT_DOCK_LAYOUT, 'left'), 'camera'));
    expect(useStore.getState().prefs.dockLayout.side).toBe('left');
    await commands.run('prefs.resetLayout');
    expect(useStore.getState().prefs.dockLayout).toEqual(DEFAULT_DOCK_LAYOUT);
  });

  it('abrir una pestaña que flota no la acopla: sigue flotando y la elegida queda activa', () => {
    const s = useStore.getState();
    s.setPref('dockLayout', floatDockTab(DEFAULT_DOCK_LAYOUT, 'machine'));
    s.setDockTab('machine');
    expect(useStore.getState().dockTab).toBe('machine');
    expect(useStore.getState().prefs.dockLayout.floating.map((f) => f.id)).toEqual(['machine']);
  });
});
