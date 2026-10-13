import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi, type PrefsFile } from '@/api';
import { createMockApi } from '@/api/mock';
import { CommandRegistry } from '@/commands/registry';
import { ALL_COMMANDS } from '@/commands';
import type { CommandDef } from '@/commands';
import { useRecents } from '@/features/history/recent';
import { createAppStore, useStore } from '@/store';
import { DEFAULT_PREFS } from '@/store/prefs';
import { applyBundle, buildBundle, buildFile, isBundle } from './bundle';
import { exportPrefs, importPrefs } from './files';
import { loadPrefsFromDisk, startPrefsPersistence, startThemeSync } from './persist';
import {
  assignShortcuts, buildEditorRows, clearShortcuts, commandLabel, filterEditorRows, findConflicts, groupEditorRows, restoreShortcuts,
} from './shortcutsModel';

const base = { label: 'cmd.file.new', category: 'file', run: () => {} } as const;
const DEFS: CommandDef[] = [
  { ...base, id: 'a', shortcuts: ['Mod+K'] },
  { ...base, id: 'b', shortcuts: ['Mod+J', 'Mod+Shift+J'] },
  { ...base, id: 'c', category: 'edit' },
];
function setup() {
  const store = createAppStore();
  const registry = new CommandRegistry(DEFS, () => store.getState());
  return { store, registry };
}

describe('paquetes .tlakuprefs', () => {
  const known = new Set(['edit.undo', 'file.save']);

  it('un paquete exportado se reconoce y se vuelve a aplicar igual', () => {
    const prefs = { ...DEFAULT_PREFS, snapPx: 12, units: 'in' as const };
    const bundle = buildBundle(prefs, { 'edit.undo': ['Mod+U'] });
    expect(bundle.format).toBe('tlaku-prefs');
    expect(isBundle(JSON.parse(JSON.stringify(bundle)))).toBe(true);
    const change = applyBundle(bundle, { prefs: DEFAULT_PREFS, shortcuts: {} }, known);
    expect(change.prefs).toEqual(prefs);
    expect(change.shortcuts).toEqual({ 'edit.undo': ['Mod+U'] });
    expect(change.appliedShortcuts).toBe(1);
    expect(change.unknownCommands).toBe(0);
  });

  it('un paquete parcial deja lo demás como está', () => {
    const current = { prefs: { ...DEFAULT_PREFS, snapPx: 20 }, shortcuts: { 'file.save': ['Mod+Shift+K'] } };
    const change = applyBundle({ prefs: { units: 'in' } }, current, known);
    expect(change.prefs.units).toBe('in');
    expect(change.prefs.snapPx).toBe(20);
    expect(change.shortcuts).toEqual(current.shortcuts); // no trae atajos: se quedan los actuales
    expect(change.appliedPrefs).toBe(1);
  });

  it('descarta lo que no entiende y lo cuenta', () => {
    const change = applyBundle(
      { prefs: { snapPx: 'mucho', units: 'cm', inventada: 1, theme: 'light' }, shortcuts: { 'edit.undo': ['zzz', 'Mod+U'], 'viejo.cmd': ['Mod+Q'] } },
      { prefs: DEFAULT_PREFS, shortcuts: {} },
      known,
    );
    expect(change.appliedPrefs).toBe(1);
    expect(change.ignoredPrefs).toBe(3);
    expect(change.prefs.theme).toBe('light');
    expect(change.prefs.snapPx).toBe(DEFAULT_PREFS.snapPx);
    expect(change.shortcuts['edit.undo']).toEqual(['Mod+U']);
    expect(change.unknownCommands).toBe(1);
  });

  it('con alcance «atajos» ignora las preferencias del paquete', () => {
    const bundle = buildBundle({ ...DEFAULT_PREFS, snapPx: 30 }, { 'edit.undo': ['Mod+U'] });
    const change = applyBundle(bundle, { prefs: DEFAULT_PREFS, shortcuts: {} }, known, 'shortcuts');
    expect(change.prefs.snapPx).toBe(DEFAULT_PREFS.snapPx);
    expect(change.shortcuts).toEqual({ 'edit.undo': ['Mod+U'] });
    // Exportar solo atajos no arrastra preferencias.
    expect(buildBundle({ ...DEFAULT_PREFS, snapPx: 30 }, {}, 'shortcuts').prefs).toEqual({});
  });

  it('no reconoce lo que no es un paquete', () => {
    expect(isBundle(null)).toBe(false);
    expect(isBundle({ version: 1, prefs: {}, shortcuts: {} })).toBe(false);
    expect(isBundle({ format: 'tlaku-prefs', version: 2 })).toBe(false);
    expect(buildFile(DEFAULT_PREFS, {}).version).toBe(1);
  });
});

/** API mínima en memoria para las pruebas con temporizadores falsos (el mock responde con `setTimeout`). */
function memoryApi() {
  let stored: PrefsFile | null = null;
  return {
    loadPreferences: async () => (stored ? structuredClone(stored) : null),
    savePreferences: vi.fn(async ({ data }: { data: PrefsFile }) => {
      stored = structuredClone(data);
    }),
  };
}

describe('guardado de preferencias', () => {
  let api: ReturnType<typeof memoryApi>;
  beforeEach(() => {
    vi.useFakeTimers();
    api = memoryApi();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('lee el archivo y lo aplica', async () => {
    const store = createAppStore();
    await api.savePreferences({ data: { version: 1, prefs: { snapPx: 15, units: 'in' }, shortcuts: { 'edit.undo': ['Mod+U'] } } });
    expect(await loadPrefsFromDisk(api, store)).toBe(true);
    expect(store.getState().prefs.snapPx).toBe(15);
    expect(store.getState().prefs.units).toBe('in');
    expect(store.getState().shortcutOverrides).toEqual({ 'edit.undo': ['Mod+U'] });
    expect(store.getState().prefsLoaded).toBe(true);
  });

  it('sin archivo conserva lo que hay y queda listo para escribir', async () => {
    const store = createAppStore();
    store.getState().setPref('snapPx', 14);
    expect(await loadPrefsFromDisk(api, store)).toBe(true);
    expect(store.getState().prefs.snapPx).toBe(14);
    expect(store.getState().prefsLoaded).toBe(true);
  });

  it('si el archivo no se puede leer avisa, reintenta una vez y no escribe si sigue fallando', async () => {
    const store = createAppStore();
    const failing = { loadPreferences: vi.fn().mockRejectedValue('versión de preferencias no soportada') };
    expect(await loadPrefsFromDisk(failing, store)).toBe(false);
    expect(failing.loadPreferences).toHaveBeenCalledTimes(2);
    expect(store.getState().prefsLoaded).toBe(false);
    expect(store.getState().notices.some((n) => n.level === 'error')).toBe(true);
  });

  it('un archivo dañado se aparta y la segunda lectura sale limpia', async () => {
    const store = createAppStore();
    const load = vi.fn().mockRejectedValueOnce('dañado').mockResolvedValueOnce(null);
    expect(await loadPrefsFromDisk({ loadPreferences: load }, store)).toBe(true);
    expect(store.getState().prefsLoaded).toBe(true);
  });

  it('guarda tras una pausa, junta varios cambios y no escribe antes de leer el archivo', async () => {
    const store = createAppStore();
    const save = api.savePreferences;
    const p = startPrefsPersistence(api, store, 400);
    store.getState().setPref('snapPx', 10); // aún no se leyó el archivo
    await vi.advanceTimersByTimeAsync(1000);
    expect(save).not.toHaveBeenCalled();

    store.getState().replacePrefs({ prefs: store.getState().prefs }, true);
    store.getState().setPref('snapPx', 11);
    store.getState().setPref('snapPx', 12);
    await vi.advanceTimersByTimeAsync(399);
    expect(save).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(2);
    expect(save).toHaveBeenCalledTimes(1);
    const saved = await api.loadPreferences();
    expect(saved?.prefs.snapPx).toBe(12);
    p.stop();
  });

  it('flush escribe de inmediato lo pendiente', async () => {
    const store = createAppStore();
    store.getState().replacePrefs({}, true);
    const p = startPrefsPersistence(api, store, 400);
    store.getState().setPref('units', 'in');
    await p.flush();
    expect((await api.loadPreferences())?.prefs.units).toBe('in');
    p.stop();
  });

  it('avisa una sola vez si no puede guardar', async () => {
    const store = createAppStore();
    store.getState().replacePrefs({}, true);
    api.savePreferences.mockRejectedValue('disco lleno');
    const p = startPrefsPersistence(api, store, 10);
    store.getState().setPref('snapPx', 9);
    await vi.advanceTimersByTimeAsync(20);
    store.getState().setPref('snapPx', 10);
    await vi.advanceTimersByTimeAsync(20);
    expect(store.getState().notices.filter((n) => n.level === 'error')).toHaveLength(1);
    p.stop();
  });
});

describe('tema', () => {
  it('la preferencia y la vista se arrastran entre sí', () => {
    const store = createAppStore();
    store.getState().setTheme('dark');
    const stop = startThemeSync(store);
    store.getState().setPref('theme', 'light');
    expect(store.getState().theme).toBe('light');
    store.getState().toggleTheme(); // el comando «Cambiar tema»
    expect(store.getState().theme).toBe('dark');
    expect(store.getState().prefs.theme).toBe('dark');
    stop();
    store.getState().toggleTheme();
    expect(store.getState().prefs.theme).toBe('dark');
  });
});

describe('editor de atajos: modelo', () => {
  it('lista todos los comandos, también los sin atajo, con su estado', () => {
    const { store, registry } = setup();
    const rows = buildEditorRows(registry, false);
    expect(rows.map((r) => r.id).sort()).toEqual(['a', 'b', 'c']);
    expect(rows.find((r) => r.id === 'c')?.combos).toEqual([]);
    store.getState().setShortcutOverrides({ a: ['Mod+L'] });
    const a = buildEditorRows(registry, false).find((r) => r.id === 'a')!;
    expect(a).toMatchObject({ combos: ['Mod+L'], factory: ['Mod+K'], customized: true });
  });

  it('busca por nombre, id o atajo, sin acentos', () => {
    const { registry } = setup();
    const rows = buildEditorRows(registry, false);
    expect(filterEditorRows(rows, 'ctrl j').map((r) => r.id)).toEqual(['b']);
    expect(filterEditorRows(rows, 'NUEVO').length).toBe(3); // la etiqueta de las tres es «Nuevo proyecto»
    expect(filterEditorRows(rows, 'xyz')).toEqual([]);
    expect(filterEditorRows(rows, '')).toHaveLength(3);
  });

  it('agrupa por categoría en el orden de los menús', () => {
    const { registry } = setup();
    const groups = groupEditorRows(buildEditorRows(registry, false));
    expect(groups.map((g) => g.category)).toEqual(['file', 'edit']);
  });

  it('el registro real: las flechas se distinguen por tamaño y todo comando aparece', () => {
    const store = createAppStore();
    const registry = new CommandRegistry(ALL_COMMANDS, () => store.getState());
    const rows = buildEditorRows(registry, false);
    expect(rows).toHaveLength(ALL_COMMANDS.length);
    const labels = rows.filter((r) => r.id.startsWith('edit.nudge.left')).map((r) => r.label);
    expect(new Set(labels).size).toBe(3);
    expect(commandLabel(registry.get('edit.nudge.left.big'))).toContain('grande');
  });

  it('reasignar libre: guarda solo ese comando', () => {
    const { registry } = setup();
    expect(assignShortcuts(registry, {}, 'c', ['Shift+Mod+U'])).toEqual({ c: ['Mod+Shift+U'] });
  });

  it('un conflicto es un error salvo que se pida quitárselo al otro', () => {
    const { store, registry } = setup();
    expect(findConflicts(registry, 'c', ['Mod+K']).map((c) => c.owner.id)).toEqual(['a']);
    expect(() => assignShortcuts(registry, {}, 'c', ['Mod+K'])).toThrow(/Conflicto/);
    const next = assignShortcuts(registry, {}, 'c', ['Mod+K'], true);
    expect(next).toEqual({ c: ['Mod+K'], a: [] });
    store.getState().setShortcutOverrides(next);
    expect(registry.match('Mod+K')?.id).toBe('c');
    expect(registry.shortcutsOf('a')).toEqual([]);
  });

  it('quitarle un atajo a un comando con varios conserva los otros', () => {
    const { registry } = setup();
    expect(assignShortcuts(registry, {}, 'c', ['Mod+J'], true)).toEqual({ c: ['Mod+J'], b: ['Mod+Shift+J'] });
  });

  it('dejar los atajos como los de fábrica borra el cambio', () => {
    const { registry } = setup();
    expect(assignShortcuts(registry, { a: ['Mod+L'] }, 'a', ['Mod+K'])).toEqual({});
  });

  it('quitar el atajo deja una lista vacía; restaurar vuelve al de fábrica', () => {
    const { store, registry } = setup();
    const cleared = clearShortcuts(registry, {}, 'a');
    expect(cleared).toEqual({ a: [] });
    store.getState().setShortcutOverrides(cleared);
    expect(registry.shortcutsOf('a')).toEqual([]);
    const restored = restoreShortcuts(registry, cleared, 'a');
    expect(restored).toEqual({});
    store.getState().setShortcutOverrides(restored);
    expect(registry.shortcutsOf('a')).toEqual(['Mod+K']);
  });

  it('restaurar un comando cuyo atajo de fábrica usa ahora otro se lo quita a ese otro', () => {
    const { store, registry } = setup();
    const stolen = assignShortcuts(registry, {}, 'c', ['Mod+K'], true);
    store.getState().setShortcutOverrides(stolen);
    const restored = restoreShortcuts(registry, stolen, 'a');
    store.getState().setShortcutOverrides(restored);
    expect(registry.match('Mod+K')?.id).toBe('a');
    expect(registry.shortcutsOf('c')).toEqual([]);
  });
});

describe('exportar e importar desde la ventana', () => {
  beforeEach(() => {
    setApi(createMockApi({ latencyMs: 0 }));
    useStore.getState().resetPrefs();
    useStore.getState().setShortcutOverrides({});
  });

  it('un viaje de ida y vuelta conserva preferencias y atajos', async () => {
    useStore.getState().setPref('snapPx', 17);
    useStore.getState().setPref('units', 'in');
    useStore.getState().setShortcutOverrides({ 'edit.undo': ['Mod+U'] });
    expect(await exportPrefs('all')).toBe(true);

    useStore.getState().resetPrefs();
    useStore.getState().setShortcutOverrides({});
    expect(await importPrefs('all')).toBe(true);
    expect(useStore.getState().prefs.snapPx).toBe(17);
    expect(useStore.getState().prefs.units).toBe('in');
    expect(useStore.getState().shortcutOverrides).toEqual({ 'edit.undo': ['Mod+U'] });
  });

  it('importar un paquete de atajos no toca las preferencias', async () => {
    useStore.getState().setShortcutOverrides({ 'edit.undo': ['Mod+U'] });
    await exportPrefs('shortcuts');
    useStore.getState().setPref('snapPx', 25);
    useStore.getState().setShortcutOverrides({});
    expect(await importPrefs('shortcuts')).toBe(true);
    expect(useStore.getState().prefs.snapPx).toBe(25);
    expect(useStore.getState().shortcutOverrides).toEqual({ 'edit.undo': ['Mod+U'] });
  });

  it('avisa si el archivo no existe y no cambia nada', async () => {
    const api = createMockApi({ latencyMs: 0 });
    setApi(api);
    const before = useStore.getState().prefs;
    expect(await importPrefs('all')).toBe(false); // el mock no tiene ningún paquete exportado
    expect(useStore.getState().prefs).toBe(before);
    expect(useStore.getState().notices.at(-1)?.level).toBe('error');
  });
});

describe('archivos recientes y preferencias', () => {
  beforeEach(() => {
    useStore.getState().resetPrefs();
    useRecents.setState({ paths: [] });
  });

  it('agregar respeta el máximo de la preferencia', () => {
    useStore.getState().setPref('recentMax', 3);
    for (const p of ['/a', '/b', '/c', '/d', '/e']) useRecents.getState().add(p);
    expect(useRecents.getState().paths).toEqual(['/e', '/d', '/c']);
  });

  it('con máximo 0 no se recuerda nada', () => {
    useStore.getState().setPref('recentMax', 0);
    useRecents.getState().add('/a');
    expect(useRecents.getState().paths).toEqual([]);
  });

  it('bajar el máximo recorta la lista', () => {
    useRecents.setState({ paths: ['/a', '/b', '/c', '/d'] });
    useRecents.getState().trim(2);
    expect(useRecents.getState().paths).toEqual(['/a', '/b']);
  });

  it('quita los archivos que ya no existen, también si la lista cambió mientras tanto', async () => {
    useRecents.setState({ paths: ['/a', '/b', '/c'] });
    const gone = await useRecents.getState().pruneMissing(async (paths) => {
      useRecents.setState({ paths: ['/n', ...paths] }); // se abrió otro archivo durante la comprobación
      return paths.map((p) => p !== '/b');
    });
    expect(gone).toBe(1);
    expect(useRecents.getState().paths).toEqual(['/n', '/a', '/c']);
  });

  it('si no se puede comprobar, no quita nada', async () => {
    useRecents.setState({ paths: ['/a'] });
    expect(await useRecents.getState().pruneMissing(async () => { throw new Error('sin permiso'); })).toBe(0);
    expect(await useRecents.getState().pruneMissing(async () => [])).toBe(0);
    expect(useRecents.getState().paths).toEqual(['/a']);
  });

  it('el mock distingue archivos que existen de los que no', async () => {
    const api = createMockApi({ latencyMs: 0 });
    expect(await api.pathsExist({ paths: ['/x/a.tlaku', '/x/no-existe.tlaku'] })).toEqual([true, false]);
  });
});
