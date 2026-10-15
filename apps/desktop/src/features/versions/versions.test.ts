import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { Document } from '@/api/types';
import { saveProject, saveTab } from '@/features/history/files';
import { usePrompts } from '@/features/history/prompt';
import { setLanguage, t } from '@/i18n';
import { useStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { resetTabs } from '@/test/tabs';
import { captureBeforeSave, highlightInCanvas, resetCaptureWarning, restoreAsNewTab, restoreInPlace, saveNamedVersion } from './actions';
import { AutoSnapshots, startAutoSnapshots } from './autoSnapshot';
import { diffDocuments, highlightIds } from './diff';
import { describeDiff } from './diffText';
import type { VersionInfo } from './types';
import { useVersionsUi } from './versionsUi';

const s = () => useStore.getState();

function obj(id: string, x = 0): Document['objects'][number] {
  return { id, layer: '#ff0000', transform: [1, 0, 0, 1, x, 0], kind: { type: 'vector', paths: [] }, locked: false, hidden: false, group: null };
}

function docOf(...ids: string[]): Document {
  return { ...emptyDocument(), objects: ids.map((id) => obj(id)) };
}

/** Responde a la confirmación pendiente (la pregunta modal de la app) con `choice`. */
async function answerPrompt(choice: string): Promise<void> {
  await vi.waitFor(() => expect(usePrompts.getState().current).not.toBeNull());
  usePrompts.getState().answer(choice);
}

let api: ReturnType<typeof createMockApi>;

beforeEach(() => {
  setLanguage('es');
  resetCaptureWarning();
  api = createMockApi({ latencyMs: 0 });
  setApi(api);
  resetTabs();
  useVersionsUi.setState({ open: false, path: null, nameOpen: false, changes: 0 });
});
afterEach(() => {
  vi.restoreAllMocks();
  usePrompts.setState({ current: null, queue: [] });
});

describe('comparación de versiones', () => {
  it('dos documentos iguales no tienen diferencias, aunque cambie el orden de las claves o haya nulls', () => {
    const a = docOf('a', 'b');
    const b: Document = { ...a, objects: a.objects.map((o) => ({ group: null, hidden: false, locked: false, kind: o.kind, transform: o.transform, layer: o.layer, id: o.id })) as Document['objects'] };
    const d = diffDocuments(a, b);
    expect(d.same).toBe(true);
    expect(describeDiff(d)).toEqual([t('versions.compare.same')]);
  });

  it('cuenta lo agregado, lo quitado y lo modificado desde la versión', () => {
    const version = { ...docOf('a', 'b', 'c'), notes: 'viejo' };
    const current: Document = { ...docOf('a', 'c', 'd', 'e'), notes: 'nuevo' };
    current.objects[1] = obj('c', 50); // c se movió
    const d = diffDocuments(version, current);
    expect(d.added.map((o) => o.id)).toEqual(['d', 'e']);
    expect(d.removed.map((o) => o.id)).toEqual(['b']);
    expect(d.changed.map((o) => o.id)).toEqual(['c']);
    expect(d.notesChanged).toBe(true);
    expect(highlightIds(d).sort()).toEqual(['c', 'd', 'e']);
    const lines = describeDiff(d);
    expect(lines).toContain('Se perderían 2 objetos agregados después.');
    expect(lines).toContain('Volvería 1 objeto que se quitó después.');
    expect(lines).toContain('1 objeto volvería a como estaba.');
    expect(lines).toContain(t('versions.compare.notes'));
  });

  it('detecta capas agregadas, quitadas y con ajustes distintos por su color', () => {
    const base = emptyDocument();
    const layer = (color: string, name: string, speed: number) => ({ ...base.layers[0]!, color, name, speed });
    const version: Document = { ...base, layers: [layer('#ff0000', 'Corte', 600), layer('#0000ff', 'Marca', 300)] };
    const current: Document = { ...base, layers: [layer('#ff0000', 'Corte', 900), layer('#00ff00', 'Grabado', 3000)] };
    const d = diffDocuments(version, current);
    expect(d.layersAdded).toEqual(['#00ff00']);
    expect(d.layersRemoved).toEqual(['#0000ff']);
    expect(d.layersChanged).toEqual(['#ff0000']);
    expect(describeDiff(d).join(' ')).toMatch(/Grabado.*Marca.*Corte/);
  });
});

describe('guardar versiones desde el guardado del proyecto', () => {
  it('cada guardado conserva primero lo que tenía el archivo, y el guardado principal sigue aunque el historial falle', async () => {
    s().applyImport({ objects: [obj('a')], new_layers: [], warnings: [] });
    expect(await saveProject(true)).toBe(true);
    const path = s().filePath!;
    // El primer guardado no tenía archivo previo: no hay versiones.
    expect((await api.versionsList({ path })).versions).toHaveLength(0);

    s().applyImport({ objects: [obj('b')], new_layers: [], warnings: [] });
    expect(await saveProject(false)).toBe(true);
    const list = (await api.versionsList({ path })).versions;
    expect(list).toHaveLength(1);
    expect(list[0]).toMatchObject({ kind: 'saved', objects: 1 });
    expect((await api.versionsRead({ path, id: list[0]!.id })).objects.map((o) => o.id)).toEqual(['a']);

    // Un disco que falla avisa pero el proyecto se guarda.
    vi.spyOn(api, 'versionsCapture').mockRejectedValue(new Error('disco lleno'));
    s().applyImport({ objects: [obj('c')], new_layers: [], warnings: [] });
    expect(await saveProject(false)).toBe(true);
    expect(s().dirty).toBe(false);
    expect(s().notices.some((n) => n.level === 'warning' && n.text.includes('disco lleno'))).toBe(true);
  });

  it('captureBeforeSave nunca lanza y avisa una vez por racha de fallos', async () => {
    vi.spyOn(api, 'versionsCapture').mockRejectedValue(new Error('sin permisos'));
    const before = s().notices.length;
    await captureBeforeSave('/x/a.tlaku');
    await captureBeforeSave('/x/a.tlaku');
    const warnings = s().notices.slice(before).filter((n) => n.text.includes('sin permisos'));
    expect(warnings).toHaveLength(1);
  });

  it('guardar una pestaña de fondo conserva la versión de SU archivo', async () => {
    s().applyImport({ objects: [obj('a')], new_layers: [], warnings: [] });
    await saveProject(true);
    const first = s().activeTabId;
    const path = s().filePath!;
    s().applyImport({ objects: [obj('b')], new_layers: [], warnings: [] });
    s().newTab();
    expect(s().activeTabId).not.toBe(first);
    expect(await saveTab(first)).toBe(true);
    const list = (await api.versionsList({ path })).versions;
    expect(list).toHaveLength(1);
    expect(list[0]!.kind).toBe('saved');
  });
});

describe('versión con nombre', () => {
  it('guarda el documento de la pestaña activa tal como está (con cambios sin guardar)', async () => {
    s().applyImport({ objects: [obj('a')], new_layers: [], warnings: [] });
    await saveProject(true);
    const path = s().filePath!;
    s().applyImport({ objects: [obj('b')], new_layers: [], warnings: [] });
    expect(await saveNamedVersion('  Antes del corte  ')).toBe(true);
    const list = (await api.versionsList({ path })).versions;
    expect(list).toHaveLength(1);
    expect(list[0]).toMatchObject({ kind: 'manual', label: 'Antes del corte', objects: 2 });
    expect(useVersionsUi.getState().changes).toBe(1);
  });

  it('un documento sin archivo no tiene historial', async () => {
    expect(await saveNamedVersion('x')).toBe(false);
  });
});

describe('restaurar', () => {
  async function projectWithTwoStates() {
    s().applyImport({ objects: [obj('a')], new_layers: [], warnings: [] });
    await saveProject(true);
    const path = s().filePath!;
    const oldVersion = (await api.versionsSnapshot({ path, doc: s().doc, kind: 'manual', label: 'v1' }))!;
    s().applyImport({ objects: [obj('b'), obj('c')], new_layers: [], warnings: [] });
    return { path, oldVersion };
  }

  it('como pestaña nueva: no toca el documento abierto', async () => {
    const { path, oldVersion } = await projectWithTwoStates();
    const tab = s().activeTabId;
    const before = s().doc;
    expect(await restoreAsNewTab(path, oldVersion)).toBe(true);
    expect(s().tabs).toHaveLength(2);
    expect(s().activeTabId).not.toBe(tab);
    expect(s().filePath).toBeNull();
    expect(s().dirty).toBe(true);
    expect(s().doc.objects.map((o) => o.id)).toEqual(['a']);
    s().activateTab(tab);
    expect(s().doc).toBe(before);
  });

  it('reemplazando: pide confirmación, guarda el estado actual como versión y es un solo paso de deshacer', async () => {
    const { path, oldVersion } = await projectWithTwoStates();
    const current = s().doc;
    const pastBefore = s().past.length;
    const done = restoreInPlace(path, oldVersion);
    await answerPrompt('restore');
    expect(await done).toBe(true);

    expect(s().doc.objects.map((o) => o.id)).toEqual(['a']);
    expect(s().dirty).toBe(true);
    expect(s().past).toHaveLength(pastBefore + 1);
    const safety = (await api.versionsList({ path })).versions.find((v) => v.kind === 'restore');
    expect(safety).toBeDefined();
    expect((await api.versionsRead({ path, id: safety!.id })).objects.map((o) => o.id)).toEqual(['a', 'b', 'c']);

    s().undo();
    expect(s().doc).toBe(current);
  });

  it('cancelar la confirmación no cambia nada', async () => {
    const { path, oldVersion } = await projectWithTwoStates();
    const before = s().doc;
    const done = restoreInPlace(path, oldVersion);
    await answerPrompt('cancel');
    expect(await done).toBe(false);
    expect(s().doc).toBe(before);
    expect((await api.versionsList({ path })).versions.some((v) => v.kind === 'restore')).toBe(false);
  });

  it('si no se puede guardar el estado actual, no se reemplaza nada', async () => {
    const { path, oldVersion } = await projectWithTwoStates();
    const before = s().doc;
    vi.spyOn(api, 'versionsSnapshot').mockRejectedValue(new Error('disco lleno'));
    const done = restoreInPlace(path, oldVersion);
    await answerPrompt('restore');
    expect(await done).toBe(false);
    expect(s().doc).toBe(before);
  });

  it('el reemplazo cae en SU pestaña aunque se cambie de pestaña mientras se confirma', async () => {
    const { path, oldVersion } = await projectWithTwoStates();
    const target = s().activeTabId;
    const done = restoreInPlace(path, oldVersion);
    await vi.waitFor(() => expect(usePrompts.getState().current).not.toBeNull());
    const other = s().newTab();
    s().applyImport({ objects: [obj('z')], new_layers: [], warnings: [] });
    usePrompts.getState().answer('restore');
    expect(await done).toBe(true);
    // La pestaña activa (otra) no se tocó y la restaurada es la del archivo.
    expect(s().activeTabId).toBe(other);
    expect(s().doc.objects.map((o) => o.id)).toEqual(['z']);
    s().activateTab(target);
    expect(s().doc.objects.map((o) => o.id)).toEqual(['a']);
  });

  it('un archivo que no está abierto solo se restaura como pestaña nueva', async () => {
    const { oldVersion } = await projectWithTwoStates();
    expect(await restoreInPlace('/otro/lado.tlaku', oldVersion)).toBe(false);
    expect(usePrompts.getState().current).toBeNull();
  });

  it('resalta en el lienzo los objetos que cambian y activa la pestaña del archivo', async () => {
    const { path } = await projectWithTwoStates();
    const tab = s().activeTabId;
    s().newTab();
    expect(highlightInCanvas(path, ['b', 'c'])).toBe(true);
    expect(s().activeTabId).toBe(tab);
    expect(s().selectedIds).toEqual(['b', 'c']);
    expect(highlightInCanvas(path, [])).toBe(false);
  });
});

describe('instantáneas automáticas', () => {
  const info = (over: Partial<VersionInfo> = {}): VersionInfo => ({
    id: 'x', kind: 'auto', label: null, created_ms: 1, size_bytes: 1, stored_bytes: 1, hash: 'h', objects: 0, layers: 0, has_thumb: false, ...over,
  });

  it('solo para pestañas con archivo y cambios, y sin repetir el mismo documento', async () => {
    const snapshot = vi.spyOn(api, 'versionsSnapshot').mockResolvedValue(info());
    const snaps = new AutoSnapshots(api, useStore);
    // Sin archivo (aunque tenga cambios): lo cubre la recuperación.
    s().applyImport({ objects: [obj('a')], new_layers: [], warnings: [] });
    expect(await snaps.tick()).toBe(0);

    await saveProject(true);
    expect(await snaps.tick()).toBe(0); // guardado: sin cambios
    s().applyImport({ objects: [obj('b')], new_layers: [], warnings: [] });
    expect(await snaps.tick()).toBe(1);
    expect(snapshot).toHaveBeenCalledWith({ path: s().filePath, doc: s().doc, kind: 'auto' });
    expect(await snaps.tick()).toBe(0); // el mismo documento
    s().applyImport({ objects: [obj('c')], new_layers: [], warnings: [] });
    expect(await snaps.tick()).toBe(1);
  });

  it('cada pestaña con cambios genera la suya', async () => {
    const snapshot = vi.spyOn(api, 'versionsSnapshot').mockResolvedValue(info());
    const snaps = new AutoSnapshots(api, useStore);
    s().applyImport({ objects: [obj('a')], new_layers: [], warnings: [] });
    await saveProject(true);
    const firstPath = s().filePath!;
    s().applyImport({ objects: [obj('b')], new_layers: [], warnings: [] });
    s().newTab();
    s().applyImport({ objects: [obj('c')], new_layers: [], warnings: [] });
    await saveProject(true);
    const secondPath = s().filePath!;
    s().applyImport({ objects: [obj('d')], new_layers: [], warnings: [] });
    expect(firstPath).toBe(secondPath); // el mock propone la misma ruta; se distingue por pestaña
    expect(await snaps.tick()).toBe(2);
    expect(snapshot).toHaveBeenCalledTimes(2);
  });

  it('un fallo avisa una sola vez y no rompe nada', async () => {
    vi.spyOn(api, 'versionsSnapshot').mockRejectedValue(new Error('sin espacio'));
    const snaps = new AutoSnapshots(api, useStore);
    s().applyImport({ objects: [obj('a')], new_layers: [], warnings: [] });
    await saveProject(true);
    s().applyImport({ objects: [obj('b')], new_layers: [], warnings: [] });
    const before = s().notices.length;
    expect(await snaps.tick()).toBe(0);
    s().applyImport({ objects: [obj('c')], new_layers: [], warnings: [] });
    expect(await snaps.tick()).toBe(0);
    expect(s().notices.slice(before).filter((n) => n.text.includes('sin espacio'))).toHaveLength(1);
  });
});

describe('temporizador de instantáneas', () => {
  it('se arma con los minutos de la configuración y se vuelve a armar al cambiarla', async () => {
    const timers: { ms: number; cleared: boolean }[] = [];
    const stop = startAutoSnapshots({
      setInterval: (_fn, ms) => timers.push({ ms, cleared: false }) - 1,
      clearInterval: (h) => {
        timers[h as number]!.cleared = true;
      },
    });
    await vi.waitFor(() => expect(timers).toHaveLength(1));
    expect(timers[0]!.ms).toBe(5 * 60_000);

    useVersionsUi.getState().setConfig({ ...useVersionsUi.getState().config, auto_minutes: 10 });
    expect(timers[0]!.cleared).toBe(true);
    expect(timers[1]!.ms).toBe(10 * 60_000);

    // Apagado o en 0 minutos: no hay temporizador.
    useVersionsUi.getState().setConfig({ ...useVersionsUi.getState().config, auto_minutes: 0 });
    expect(timers[1]!.cleared).toBe(true);
    expect(timers).toHaveLength(2);
    stop();
  });
});

describe('API simulada del historial', () => {
  it('imita al motor: no repite contenido, la manual nombra a la última y borrar/limpiar funcionan', async () => {
    const path = '/p/mesa.tlaku';
    const a = (await api.versionsSnapshot({ path, doc: docOf('a'), kind: 'auto' }))!;
    expect(await api.versionsSnapshot({ path, doc: docOf('a'), kind: 'auto' })).toBeNull();
    const named = (await api.versionsSnapshot({ path, doc: docOf('a'), kind: 'manual', label: 'Hito' }))!;
    expect(named.id).toBe(a.id);
    expect(named).toMatchObject({ kind: 'manual', label: 'Hito' });
    await expect(api.versionsSnapshot({ path, doc: docOf('b'), kind: 'manual', label: '  ' })).rejects.toThrow();
    await api.versionsSnapshot({ path, doc: docOf('b'), kind: 'auto' });
    expect((await api.versionsProjects())[0]).toMatchObject({ path, versions: 2 });
    expect(await api.versionsDelete({ path, ids: [a.id] })).toBe(1);
    expect((await api.versionsList({ path })).versions).toHaveLength(1);
    expect(await api.versionsClear({ path })).toBe(1);
    expect(await api.versionsProjects()).toEqual([]);
  });

  it('la configuración se acota', async () => {
    const cfg = await api.versionsConfigGet();
    const kept = await api.versionsConfigSet({ config: { ...cfg, auto_minutes: 9999, max_versions: 1 } });
    expect(kept.auto_minutes).toBe(240);
    expect(kept.max_versions).toBe(10);
  });
});
