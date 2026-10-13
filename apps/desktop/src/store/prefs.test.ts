import { describe, expect, it } from 'vitest';
import { createAppStore } from './index';
import { checkPref, DEFAULT_PREFS, PREF_KEYS, PREF_SPECS, sanitizePref, sanitizePrefs } from './prefs';

describe('catálogo de preferencias', () => {
  it('los valores de fábrica son válidos según sus propios límites', () => {
    for (const key of PREF_KEYS) expect(checkPref(key, DEFAULT_PREFS[key])?.value, key).toBe(DEFAULT_PREFS[key]);
  });

  it('conserva los valores de las constantes que reemplazan', () => {
    expect(DEFAULT_PREFS.snapPx).toBe(8);
    expect([DEFAULT_PREFS.nudgeStep, DEFAULT_PREFS.nudgeStepBig, DEFAULT_PREFS.nudgeStepFine]).toEqual([1, 10, 0.1]);
    expect(DEFAULT_PREFS.units).toBe('mm');
    expect(DEFAULT_PREFS.language).toBe('es');
  });

  it('acota los números al rango y redondea los enteros', () => {
    expect(sanitizePref('snapPx', 500)).toBe(PREF_SPECS.snapPx.max);
    expect(sanitizePref('snapPx', 0)).toBe(1);
    expect(sanitizePref('snapPx', 8.6)).toBe(9);
    expect(sanitizePref('autosaveSecs', 1)).toBe(10);
    expect(sanitizePref('nudgeStepFine', 0.25)).toBe(0.25);
  });

  it('un valor de otro tipo toma el valor de fábrica', () => {
    expect(sanitizePref('snapPx', '12')).toBe(8);
    expect(sanitizePref('snapPx', Number.NaN)).toBe(8);
    expect(sanitizePref('autosave', 'sí')).toBe(true);
    expect(sanitizePref('units', 'cm')).toBe('mm');
    expect(sanitizePref('theme', 3)).toBe('dark');
    expect(checkPref('units', 'cm')).toBeNull();
  });

  it('sanitizePrefs completa lo que falta y descarta lo que sobra', () => {
    const out = sanitizePrefs({ snapPx: 12, units: 'in', inventada: 1 });
    expect(out.snapPx).toBe(12);
    expect(out.units).toBe('in');
    expect(out.autosaveSecs).toBe(60);
    expect(Object.keys(out).sort()).toEqual([...PREF_KEYS].sort());
    expect(sanitizePrefs(null)).toEqual(DEFAULT_PREFS);
    expect(sanitizePrefs('basura')).toEqual(DEFAULT_PREFS);
  });
});

describe('preferencias de capas (ola 2)', () => {
  it('el modo principiante es un interruptor apagado de fábrica', () => {
    expect(DEFAULT_PREFS.beginnerMode).toBe(false);
    expect(sanitizePref('beginnerMode', true)).toBe(true);
    expect(sanitizePref('beginnerMode', 'sí')).toBe(false);
  });

  it('layerDefaults vale null de fábrica y acepta una plantilla válida, saneada', () => {
    expect(DEFAULT_PREFS.layerDefaults).toBeNull();
    expect(checkPref('layerDefaults', null)).toEqual({ value: null });
    const t = checkPref('layerDefaults', { speed: 1500, mode: 'raro' })?.value;
    expect(t).toMatchObject({ speed: 1500, mode: 'line', power: 50 });
    expect(t && 'color' in t).toBe(false);
  });

  it('layerDefaults inválido toma el valor de fábrica y sobrevive a un paquete', () => {
    expect(checkPref('layerDefaults', 7)).toBeNull();
    expect(sanitizePref('layerDefaults', 'basura')).toBeNull();
    const saved = sanitizePrefs({ layerDefaults: { speed: 900 } });
    expect(sanitizePrefs(JSON.parse(JSON.stringify(saved)))).toEqual(saved);
  });

  it('setPref guarda los valores por defecto y no emite si el contenido es igual', () => {
    const store = createAppStore();
    store.getState().setPref('layerDefaults', checkPref('layerDefaults', { speed: 900 })!.value);
    expect(store.getState().prefs.layerDefaults?.speed).toBe(900);
    store.getState().resetPrefs();
    expect(store.getState().prefs.layerDefaults).toBeNull();
  });
});

describe('estado de preferencias', () => {
  it('setPref cambia un valor sin tocar los demás y sanea', () => {
    const store = createAppStore();
    const before = store.getState().prefs;
    store.getState().setPref('snapPx', 12);
    expect(store.getState().prefs.snapPx).toBe(12);
    expect(store.getState().prefs.units).toBe(before.units);
    store.getState().setPref('snapPx', 9999);
    expect(store.getState().prefs.snapPx).toBe(PREF_SPECS.snapPx.max);
  });

  it('no emite un cambio si el valor es el mismo', () => {
    const store = createAppStore();
    const before = store.getState().prefs;
    store.getState().setPref('snapPx', before.snapPx);
    expect(store.getState().prefs).toBe(before);
  });

  it('replacePrefs sanea y marca como cargadas; resetPrefs vuelve a fábrica', () => {
    const store = createAppStore();
    expect(store.getState().prefsLoaded).toBe(false);
    store.getState().replacePrefs({ prefs: { snapPx: 20, units: 'raro' }, shortcuts: { 'edit.undo': ['Shift+Mod+U', 'zzz'] } });
    expect(store.getState().prefsLoaded).toBe(true);
    expect(store.getState().prefs.snapPx).toBe(20);
    expect(store.getState().prefs.units).toBe('mm');
    expect(store.getState().shortcutOverrides).toEqual({ 'edit.undo': ['Mod+Shift+U'] });
    store.getState().resetPrefs();
    expect(store.getState().prefs).toEqual(DEFAULT_PREFS);
    // Restablecer no toca los atajos.
    expect(store.getState().shortcutOverrides).toEqual({ 'edit.undo': ['Mod+Shift+U'] });
  });

  it('replacePrefs con loaded=false no marca como cargadas', () => {
    const store = createAppStore();
    store.getState().replacePrefs({ prefs: {} }, false);
    expect(store.getState().prefsLoaded).toBe(false);
  });
});
