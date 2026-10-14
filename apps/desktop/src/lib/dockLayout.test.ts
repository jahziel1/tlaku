import { describe, expect, it } from 'vitest';
import {
  DEFAULT_DOCK_LAYOUT, DOCK_TAB_IDS, dockFloatingTab, fitFloatRect, FLOAT_MAX_W, FLOAT_MIN_W, floatDockTab, isFloating, moveDockTab, normalizeDockLayout,
  nudgeDockTab, sanitizeDockLayout, setDockSide, setDockTabHidden, setFloatRect, setToolsSide, shellColumns, shownFloating, visibleTabs,
} from './dockLayout';

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

describe('paneles flotantes', () => {
  it('sacar una pestaña la quita del panel y la deja en una ventana con su posición', () => {
    const l = floatDockTab(DEFAULT_DOCK_LAYOUT, 'camera', { x: 300, y: 120 });
    expect(isFloating(l, 'camera')).toBe(true);
    expect(visibleTabs(l)).not.toContain('camera');
    expect(l.floating[0]).toMatchObject({ id: 'camera', x: 300, y: 120 });
    // Sacarla otra vez no cambia nada.
    expect(floatDockTab(l, 'camera')).toBe(l);
  });

  it('las ventanas nuevas salen en cascada para no taparse', () => {
    let l = floatDockTab(DEFAULT_DOCK_LAYOUT, 'camera');
    l = floatDockTab(l, 'notes');
    expect(l.floating[1]!.x).toBeGreaterThan(l.floating[0]!.x);
    expect(l.floating[1]!.y).toBeGreaterThan(l.floating[0]!.y);
  });

  it('la última pestaña acoplada no se puede sacar', () => {
    let l = DEFAULT_DOCK_LAYOUT;
    for (const id of DOCK_TAB_IDS.slice(1)) l = floatDockTab(l, id);
    expect(visibleTabs(l)).toEqual(['layers']);
    expect(floatDockTab(l, 'layers')).toBe(l);
  });

  it('acoplar devuelve la pestaña al panel en su sitio del orden', () => {
    const l = floatDockTab(DEFAULT_DOCK_LAYOUT, 'machine');
    const back = dockFloatingTab(l, 'machine');
    expect(back.floating).toEqual([]);
    expect(visibleTabs(back)).toEqual([...DOCK_TAB_IDS]);
    expect(dockFloatingTab(back, 'machine')).toBe(back);
  });

  it('mover o cambiar el tamaño acota a los límites y no toca lo demás', () => {
    const l = floatDockTab(DEFAULT_DOCK_LAYOUT, 'camera', { x: 100, y: 100, w: 400, h: 300 });
    const moved = setFloatRect(l, 'camera', { x: 150.4, y: 90, w: 5, h: 99999 });
    expect(moved.floating[0]).toMatchObject({ x: 150, y: 90, w: FLOAT_MIN_W });
    expect(moved.floating[0]!.h).toBeLessThanOrEqual(1200);
    expect(setFloatRect(l, 'camera', { x: 100, y: 100, w: 400, h: 300 })).toBe(l);
    expect(setFloatRect(l, 'notes', { x: 1, y: 1, w: 300, h: 300 })).toBe(l);
  });

  it('ocultar una flotante no cuenta contra las pestañas acopladas y deja de mostrarse', () => {
    let l = floatDockTab(DEFAULT_DOCK_LAYOUT, 'camera');
    l = setDockTabHidden(l, 'camera', true);
    expect(shownFloating(l)).toEqual([]);
    expect(l.hidden).toEqual(['camera']);
    expect(shownFloating(setDockTabHidden(l, 'camera', false))).toHaveLength(1);
  });

  it('normaliza datos ajenos: descarta repetidas, desconocidas y medidas rotas', () => {
    const out = normalizeDockLayout({
      floating: [
        { id: 'camera', x: 10, y: 10, w: 400, h: 300 },
        { id: 'camera', x: 99, y: 99, w: 400, h: 300 },
        { id: 'inventada', x: 1, y: 1, w: 300, h: 300 },
        { id: 'notes', x: 'a', y: 1, w: 300, h: 300 },
        { id: 'materials', x: -50, y: 5, w: 99999, h: 10 },
        null,
      ],
    });
    expect(out.floating.map((f) => f.id)).toEqual(['camera', 'materials']);
    expect(out.floating[0]).toMatchObject({ x: 10, y: 10 });
    expect(out.floating[1]).toMatchObject({ x: 0, w: FLOAT_MAX_W });
  });

  it('si todas las pestañas estarían flotando u ocultas, vuelven al panel', () => {
    const all = DOCK_TAB_IDS.map((id) => ({ id, x: 0, y: 0, w: 300, h: 300 }));
    const out = normalizeDockLayout({ floating: all });
    expect(out.floating).toEqual([]);
    expect(visibleTabs(out)).toHaveLength(DOCK_TAB_IDS.length);
    const mixed = normalizeDockLayout({ floating: all.slice(0, 4), hidden: DOCK_TAB_IDS.slice(4) });
    expect(visibleTabs(mixed).length).toBeGreaterThan(0);
  });

  it('un diseño de una versión anterior (solo orden y ocultas) se completa con los valores de fábrica', () => {
    const out = sanitizeDockLayout({ order: [...DOCK_TAB_IDS], hidden: [] });
    expect(out).toEqual(DEFAULT_DOCK_LAYOUT);
    expect(sanitizeDockLayout(DEFAULT_DOCK_LAYOUT)).toBe(DEFAULT_DOCK_LAYOUT);
  });

  it('un panel no se pierde si la ventana se achica', () => {
    const r = fitFloatRect({ x: 1500, y: 900, w: 400, h: 300 }, { w: 1000, h: 600 });
    expect(r.x + r.w).toBeLessThanOrEqual(1000);
    expect(r.y).toBeLessThan(600);
    expect(fitFloatRect({ x: 10, y: 10, w: 2000, h: 2000 }, { w: 800, h: 500 })).toMatchObject({ w: 800, h: 500 });
  });
});

describe('bordes del panel y de las herramientas', () => {
  it('cambian por separado y se validan', () => {
    const l = setDockSide(DEFAULT_DOCK_LAYOUT, 'left');
    expect(l.side).toBe('left');
    expect(setDockSide(l, 'left')).toBe(l);
    expect(setToolsSide(l, 'right').toolsSide).toBe('right');
    expect(normalizeDockLayout({ side: 'arriba', toolsSide: 3 })).toMatchObject({ side: 'right', toolsSide: 'left' });
  });

  it('las columnas de la ventana siguen los bordes, con el panel del lado del lienzo', () => {
    expect(shellColumns(DEFAULT_DOCK_LAYOUT)).toEqual(['tools', 'main', 'dock']);
    expect(shellColumns({ side: 'left', toolsSide: 'left' })).toEqual(['tools', 'dock', 'main']);
    expect(shellColumns({ side: 'left', toolsSide: 'right' })).toEqual(['dock', 'main', 'tools']);
    expect(shellColumns({ side: 'right', toolsSide: 'right' })).toEqual(['main', 'dock', 'tools']);
  });
});
