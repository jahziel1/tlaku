import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import type { Document } from '@/api/types';
import { createAppStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { parseSession, restoreSession, sameSession, sessionOf, SESSION_KEY, SESSION_MAX_TABS, startSessionSaving } from './session';

let store: ReturnType<typeof createAppStore>;
const s = () => store.getState();
const docWith = (ids: string[]): Document => ({ ...emptyDocument(), objects: sampleObjects().filter((o) => ids.includes(o.id)), layers: sampleLayers() });

beforeEach(() => {
  store = createAppStore();
});

describe('sesión de pestañas', () => {
  it('solo recuerda las pestañas con archivo, en orden, y cuál estaba activa', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    s().newTab(); // Sin título: no se recuerda
    s().openInTab(docWith(['obj-foto']), '/p/b.tlaku'); // reemplaza la vacía activa
    const session = sessionOf(s());
    expect(session).toEqual({ v: 1, paths: ['/p/a.tlaku', '/p/b.tlaku'], active: 1 });
    s().activateTab(s().tabs[0]!.id);
    expect(sessionOf(s()).active).toBe(0);
  });

  it('si la activa no tiene archivo, active es -1', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    s().newTab();
    expect(sessionOf(s())).toEqual({ v: 1, paths: ['/p/a.tlaku'], active: -1 });
  });

  it('saneado: ignora lo roto, quita repetidos y acota la cantidad', () => {
    expect(parseSession(null)).toBeNull();
    expect(parseSession('{no es json')).toBeNull();
    expect(parseSession('{"v":2,"paths":[]}')).toBeNull();
    expect(parseSession('{"v":1,"paths":"x"}')).toBeNull();
    expect(parseSession('{"v":1,"paths":["/a",7,"","/a","/b"],"active":9}')).toEqual({ v: 1, paths: ['/a', '/b'], active: -1 });
    const many = JSON.stringify({ v: 1, paths: Array.from({ length: 100 }, (_, i) => `/p/${i}.tlaku`), active: 1 });
    expect(parseSession(many)!.paths).toHaveLength(SESSION_MAX_TABS);
  });

  it('se guarda al cambiar las pestañas, el archivo o la activa, y no se repite si no cambió nada', () => {
    const write = vi.fn();
    const stop = startSessionSaving(store, write);
    expect(write).toHaveBeenCalledTimes(1); // estado inicial: sin archivos
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    expect(JSON.parse(write.mock.calls.at(-1)![1])).toEqual({ v: 1, paths: ['/p/a.tlaku'], active: 0 });
    const n = write.mock.calls.length;
    s().select(['obj-placa']); // un cambio que no toca la sesión
    s().deleteObjects(['obj-placa']);
    expect(write).toHaveBeenCalledTimes(n);
    s().newTab();
    expect(JSON.parse(write.mock.calls.at(-1)![1]).active).toBe(-1);
    stop();
    s().openInTab(docWith(['obj-foto']), '/p/b.tlaku');
    expect(JSON.parse(write.mock.calls.at(-1)![1]).paths).toEqual(['/p/a.tlaku']); // detenido
    expect(write.mock.calls[0]![0]).toBe(SESSION_KEY);
  });

  it('sameSession compara ruta por ruta', () => {
    expect(sameSession({ v: 1, paths: ['a'], active: 0 }, { v: 1, paths: ['a'], active: 0 })).toBe(true);
    expect(sameSession({ v: 1, paths: ['a'], active: 0 }, { v: 1, paths: ['b'], active: 0 })).toBe(false);
    expect(sameSession({ v: 1, paths: ['a'], active: 0 }, { v: 1, paths: ['a'], active: -1 })).toBe(false);
  });
});

describe('restaurar la sesión', () => {
  const session = (paths: string[], active: number) => JSON.stringify({ v: 1, paths, active });

  it('reabre cada proyecto en su pestaña, la primera reemplaza la vacía, y deja activa la que estaba', async () => {
    const api = createMockApi({ latencyMs: 0 });
    await api.saveProject({ path: '/p/a.tlaku', doc: docWith(['obj-placa']) });
    await api.saveProject({ path: '/p/b.tlaku', doc: docWith(['obj-foto']) });
    const first = s().activeTabId;
    const opened = await restoreSession({ api, store, read: () => session(['/p/a.tlaku', '/p/b.tlaku'], 0) });
    expect(opened).toBe(2);
    expect(s().tabs).toHaveLength(2);
    expect(s().tabs[0]!.id).toBe(first); // la vacía se reutilizó
    expect(s().filePath).toBe('/p/a.tlaku');
    expect(s().dirty).toBe(false);
    expect(s().doc.objects.map((o) => o.id)).toEqual(['obj-placa']);
  });

  it('un archivo que ya no existe se avisa y se salta; las demás pestañas se abren', async () => {
    const api = createMockApi({ latencyMs: 0 });
    await api.saveProject({ path: '/p/b.tlaku', doc: docWith(['obj-foto']) });
    const real = api.loadProject.bind(api);
    vi.spyOn(api, 'loadProject').mockImplementation(async (a) => {
      if (a.path === '/p/perdido.tlaku') throw new Error('No existe el archivo');
      return real(a);
    });
    const opened = await restoreSession({ api, store, read: () => session(['/p/perdido.tlaku', '/p/b.tlaku'], 1) });
    expect(opened).toBe(1);
    expect(s().filePath).toBe('/p/b.tlaku');
    const warn = s().notices.find((n) => n.level === 'warning');
    expect(warn?.text).toContain('perdido.tlaku');
  });

  it('con la preferencia apagada no reabre nada', async () => {
    const api = createMockApi({ latencyMs: 0 });
    const load = vi.spyOn(api, 'loadProject');
    s().setPref('restoreTabs', false);
    expect(await restoreSession({ api, store, read: () => session(['/p/a.tlaku'], 0) })).toBe(0);
    expect(load).not.toHaveBeenCalled();
    expect(s().tabs).toHaveLength(1);
  });

  it('viene encendida de fábrica, y sin sesión o con sesión vacía no hace nada', async () => {
    const api = createMockApi({ latencyMs: 0 });
    expect(s().prefs.restoreTabs).toBe(true);
    expect(await restoreSession({ api, store, read: () => null })).toBe(0);
    expect(await restoreSession({ api, store, read: () => session([], -1) })).toBe(0);
    expect(s().tabs).toHaveLength(1);
  });
});
