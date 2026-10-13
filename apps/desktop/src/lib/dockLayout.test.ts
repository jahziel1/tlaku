import { describe, expect, it } from 'vitest';
import { DEFAULT_DOCK_LAYOUT, DOCK_TAB_IDS, moveDockTab, normalizeDockLayout, nudgeDockTab, setDockTabHidden, visibleTabs } from './dockLayout';

describe('diseño del panel acoplable', () => {
  it('normaliza datos ajenos: descarta lo inválido y completa lo que falta', () => {
    expect(normalizeDockLayout(null)).toEqual(DEFAULT_DOCK_LAYOUT);
    const out = normalizeDockLayout({ order: ['camera', 'layers', 'camera', 'inventada', 7], hidden: ['machine', 'zzz', 'machine'] });
    expect(out.order.slice(0, 2)).toEqual(['camera', 'layers']);
    expect([...out.order].sort()).toEqual([...DOCK_TAB_IDS].sort());
    expect(out.hidden).toEqual(['machine']);
  });

  it('nunca deja todas las pestañas ocultas', () => {
    expect(normalizeDockLayout({ hidden: [...DOCK_TAB_IDS] }).hidden).toEqual([]);
  });

  it('reordena arrastrando una pestaña antes de otra o al final', () => {
    const a = moveDockTab(DEFAULT_DOCK_LAYOUT, 'camera', 'layers');
    expect(a.order.slice(0, 3)).toEqual(['camera', 'layers', 'machine']);
    const b = moveDockTab(a, 'camera', null);
    expect(b.order.at(-1)).toBe('camera');
    expect(moveDockTab(DEFAULT_DOCK_LAYOUT, 'layers', 'layers')).toBe(DEFAULT_DOCK_LAYOUT);
    expect(moveDockTab(DEFAULT_DOCK_LAYOUT, 'layers', 'machine')).toBe(DEFAULT_DOCK_LAYOUT);
  });

  it('las flechas mueven una posición y respetan los extremos', () => {
    const l = nudgeDockTab(DEFAULT_DOCK_LAYOUT, 'machine', -1);
    expect(l.order.slice(0, 2)).toEqual(['machine', 'layers']);
    expect(nudgeDockTab(l, 'machine', -1)).toBe(l);
    expect(nudgeDockTab(DEFAULT_DOCK_LAYOUT, 'notes', 1)).toBe(DEFAULT_DOCK_LAYOUT);
  });

  it('oculta y muestra, pero deja siempre una visible', () => {
    let l = DEFAULT_DOCK_LAYOUT;
    for (const id of DOCK_TAB_IDS) l = setDockTabHidden(l, id, true);
    expect(visibleTabs(l)).toHaveLength(1);
    const shown = setDockTabHidden(l, 'camera', false);
    expect(visibleTabs(shown)).toContain('camera');
  });
});
