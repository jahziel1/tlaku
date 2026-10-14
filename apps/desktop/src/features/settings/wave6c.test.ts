import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_DOCK_LAYOUT, floatDockTab } from '@/lib/dockLayout';
import { createAppStore } from '@/store';
import { CACHE_STORAGE_KEY, DEFAULT_PREFS, PREF_KEYS, writeCache } from '@/store/prefs';
import { applyBundle, buildBundle } from './bundle';
import { startPrefsWindowSync } from './persist';

const memory = new Map<string, string>();
beforeEach(() => {
  memory.clear();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => memory.get(k) ?? null,
    setItem: (k: string, v: string) => void memory.set(k, v),
    removeItem: (k: string) => void memory.delete(k),
  });
});
afterEach(() => vi.unstubAllGlobals());

describe('preferencias nuevas (ola 6C)', () => {
  it('tienen valor de fábrica: volver a Seleccionar encendido, rejilla normal, 96 ppp y recordar opciones', () => {
    expect(DEFAULT_PREFS).toMatchObject({ returnToSelect: true, gridContrast: 'normal', importDpi: 96, importRemember: true, importOptions: null });
    for (const k of ['returnToSelect', 'gridContrast', 'importDpi', 'importRemember', 'importOptions', 'dockLayout'] as const) expect(PREF_KEYS).toContain(k);
  });

  it('el store las sanea: contraste desconocido, ppp fuera de rango y opciones de importación dañadas', () => {
    const store = createAppStore();
    store.getState().setPref('gridContrast', 'altísimo' as never);
    expect(store.getState().prefs.gridContrast).toBe('normal');
    store.getState().setPref('importDpi', 99999);
    expect(store.getState().prefs.importDpi).toBe(2400);
    store.getState().setPref('importDpi', 1);
    expect(store.getState().prefs.importDpi).toBe(10);
    store.getState().setPref('importOptions', { dpi: 150, group: true } as never);
    expect(store.getState().prefs.importOptions).toMatchObject({ dpi: 150, group: true, target_layer: null });
    store.getState().setPref('importOptions', 5 as never);
    expect(store.getState().prefs.importOptions).toBeNull();
  });
});

describe('.tlakuprefs con las preferencias de la ola 6C', () => {
  it('lleva la distribución (con paneles flotantes) y las opciones de importación, y las recupera', () => {
    const dockLayout = floatDockTab({ ...DEFAULT_DOCK_LAYOUT, side: 'left', toolsSide: 'right' }, 'camera', { x: 200, y: 80 });
    const prefs = { ...DEFAULT_PREFS, dockLayout, gridContrast: 'high' as const, importDpi: 300, returnToSelect: false, importOptions: { dpi: 150, dxf_unit_mm: 25.4, group: false, auto_group: true, target_layer: '#ff0000', gcode_by_power: false } };
    const bundle = JSON.parse(JSON.stringify(buildBundle(prefs, {})));
    const change = applyBundle(bundle, { prefs: DEFAULT_PREFS, shortcuts: {} }, new Set());
    expect(change.prefs).toEqual(prefs);
    expect(change.ignoredPrefs).toBe(3); // `remoteEnabled`, `updateCheckOnStart` y `ruidaExperimentalWrites`, que un archivo ajeno nunca enciende
  });

  it('un paquete ajeno con valores rotos no deja una distribución ni opciones inválidas', () => {
    const change = applyBundle(
      { prefs: { dockLayout: { side: 'arriba', floating: [{ id: 'camera', x: 'a' }] }, importOptions: 'no', gridContrast: 'x', importDpi: 'mucho' } },
      { prefs: DEFAULT_PREFS, shortcuts: {} },
      new Set(),
    );
    expect(change.prefs.dockLayout.side).toBe('right');
    expect(change.prefs.dockLayout.floating).toEqual([]);
    expect(change.prefs.importOptions).toBeNull();
    expect(change.prefs.gridContrast).toBe('normal');
    expect(change.prefs.importDpi).toBe(96);
    expect(change.appliedPrefs).toBe(1); // solo `dockLayout` se sanea a algo válido
    expect(change.ignoredPrefs).toBe(3);
  });

  it('un paquete de una versión anterior (distribución sin bordes) se completa', () => {
    const change = applyBundle({ prefs: { dockLayout: { order: ['camera', 'layers'], hidden: ['notes'] } } }, { prefs: DEFAULT_PREFS, shortcuts: {} }, new Set());
    expect(change.prefs.dockLayout).toMatchObject({ side: 'right', toolsSide: 'left', floating: [], hidden: ['notes'] });
    expect(change.prefs.dockLayout.order.slice(0, 2)).toEqual(['camera', 'layers']);
  });
});

describe('preferencias compartidas entre ventanas', () => {
  function fakeWindow() {
    let handler: ((e: Event) => void) | null = null;
    return {
      addEventListener: (_: string, h: (e: Event) => void) => void (handler = h),
      removeEventListener: () => void (handler = null),
      fire: (key: string | null) => handler?.({ key } as unknown as Event),
      attached: () => handler !== null,
    };
  }

  it('adopta lo que otra ventana guardó en la copia local', () => {
    const store = createAppStore();
    store.getState().replacePrefs({ prefs: DEFAULT_PREFS }, true);
    const win = fakeWindow();
    const stop = startPrefsWindowSync(store, win as never);
    // «Otra ventana» escribe la copia local con otro tema y un panel flotante.
    const dockLayout = floatDockTab(DEFAULT_DOCK_LAYOUT, 'notes');
    writeCache({ prefs: { ...DEFAULT_PREFS, snapPx: 21, dockLayout }, shortcuts: {} });
    win.fire(CACHE_STORAGE_KEY);
    expect(store.getState().prefs.snapPx).toBe(21);
    expect(store.getState().prefs.dockLayout.floating.map((f) => f.id)).toEqual(['notes']);
    stop();
    expect(win.attached()).toBe(false);
  });

  it('ignora otras claves, y lo que aún no se cargó del disco, y no repite un cambio igual', () => {
    const store = createAppStore();
    const win = fakeWindow();
    startPrefsWindowSync(store, win as never);
    writeCache({ prefs: { ...DEFAULT_PREFS, snapPx: 30 }, shortcuts: {} });
    win.fire(CACHE_STORAGE_KEY); // sin cargar: no se toca
    expect(store.getState().prefs.snapPx).toBe(DEFAULT_PREFS.snapPx);
    store.getState().replacePrefs({ prefs: DEFAULT_PREFS }, true);
    win.fire('tlaku.otra');
    win.fire(null);
    expect(store.getState().prefs.snapPx).toBe(DEFAULT_PREFS.snapPx);
    const spy = vi.spyOn(store.getState(), 'replacePrefs');
    writeCache({ prefs: store.getState().prefs, shortcuts: {} });
    win.fire(CACHE_STORAGE_KEY);
    expect(spy).not.toHaveBeenCalled();
  });
});
