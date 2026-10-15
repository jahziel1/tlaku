import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi } from '@/api/mock';
import type { Document } from '@/api/types';
import { createAppStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { Autosaver } from './autosave';
import { formatBytes, formatCopyDate } from './format';
import { checkRecovery, discardAll, discardCopy, recoverAll, recoverCopy, useRecovery, type RecoveryDeps } from './recover';

/** Documento con un objeto, distinto de uno vacío (para tener «cambios»). */
function docWith(id: string): Document {
  const doc = emptyDocument();
  return {
    ...doc,
    objects: [{ id, layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [] }, locked: false, hidden: false, group: null }],
  };
}

function setup() {
  const store = createAppStore();
  const api = createMockApi({ latencyMs: 0 });
  const calls: string[] = [];
  const rawWrite = api.autosaveWrite.bind(api);
  const writes = vi.spyOn(api, 'autosaveWrite');
  const discards = vi.spyOn(api, 'autosaveDiscard');
  let n = 0;
  const timers: { fn: () => void; ms: number; cleared: boolean }[] = [];
  const saver = new Autosaver(api, store, {
    newId: () => `id${++n}`,
    setInterval: (fn, ms) => {
      timers.push({ fn, ms, cleared: false });
      return timers.length - 1;
    },
    clearInterval: (h) => {
      timers[h as number]!.cleared = true;
    },
  });
  return { store, api, saver, writes, rawWrite, discards, timers, calls };
}

/** Hace que el documento tenga cambios sin guardar. */
function edit(store: ReturnType<typeof createAppStore>, id = 'o1') {
  store.getState().applyImport({ objects: docWith(id).objects, new_layers: [], warnings: [] });
}

describe('autoguardado', () => {
  it('sin cambios no guarda nada', async () => {
    const { saver, writes } = setup();
    expect(await saver.tick()).toBe('skipped');
    expect(writes).not.toHaveBeenCalled();
  });

  it('con cambios guarda una copia con el nombre y la ruta del documento', async () => {
    const { store, saver, writes } = setup();
    edit(store);
    expect(await saver.tick()).toBe('saved');
    expect(writes).toHaveBeenCalledTimes(1);
    const arg = writes.mock.calls[0]![0];
    expect(arg.id).toBe('id1');
    expect(arg.name).toBe('Sin título');
    expect(arg.path).toBeNull();
    expect(arg.doc).toBe(store.getState().doc);
  });

  it('no repite la copia si el documento no cambió desde la última', async () => {
    const { store, saver, writes } = setup();
    edit(store);
    await saver.tick();
    expect(await saver.tick()).toBe('skipped');
    edit(store, 'o2');
    expect(await saver.tick()).toBe('saved');
    expect(writes).toHaveBeenCalledTimes(2);
    expect(writes.mock.calls[1]![0].id).toBe('id1'); // misma sesión: reemplaza la copia
  });

  it('con el autoguardado apagado no guarda', async () => {
    const { store, saver, writes } = setup();
    store.getState().setPref('autosave', false);
    edit(store);
    expect(await saver.tick()).toBe('skipped');
    expect(writes).not.toHaveBeenCalled();
    // Pero una petición explícita (recuperar) sí guarda.
    expect(await saver.tick(true)).toBe('saved');
  });

  it('al guardar el proyecto se borra la copia y se empieza otra sesión', async () => {
    const { store, api, saver, discards } = setup();
    saver.start();
    edit(store);
    await saver.tick();
    expect((await api.autosaveList()).map((c) => c.id)).toEqual(['id1']);

    store.getState().markSaved('/p/mesa.tlaku');
    await vi.waitFor(() => expect(discards).toHaveBeenCalledWith({ id: 'id1' }));
    expect(await api.autosaveList()).toEqual([]);
    expect(saver.currentId()).toBe('id2');

    edit(store, 'o3');
    await saver.tick();
    expect((await api.autosaveList()).map((c) => ({ id: c.id, name: c.name, path: c.path }))).toEqual([{ id: 'id2', name: 'mesa.tlaku', path: '/p/mesa.tlaku' }]);
    saver.stop();
  });

  it('deshacer hasta el estado guardado también borra la copia', async () => {
    const { store, api, saver } = setup();
    saver.start();
    edit(store);
    await saver.tick();
    store.getState().undo();
    expect(store.getState().dirty).toBe(false);
    await vi.waitFor(async () => expect(await api.autosaveList()).toEqual([]));
    saver.stop();
  });

  it('si se guarda mientras se escribe la copia, la copia no queda', async () => {
    const { store, api, saver, writes, rawWrite } = setup();
    saver.start();
    edit(store);
    writes.mockImplementation(async (a) => {
      const r = await rawWrite(a);
      store.getState().markSaved('/p/x.tlaku'); // el usuario guarda justo ahora
      return r;
    });
    await saver.tick();
    await vi.waitFor(async () => expect(await api.autosaveList()).toEqual([]));
    saver.stop();
  });

  it('cerrar descartando: endSession borra la copia', async () => {
    const { store, api, saver } = setup();
    edit(store);
    await saver.tick();
    await saver.endSession();
    expect(await api.autosaveList()).toEqual([]);
  });

  it('un fallo avisa una sola vez y se recupera solo', async () => {
    const { store, saver, writes: spy, rawWrite: write } = setup();
    spy.mockRejectedValue('disco lleno');
    edit(store);
    expect(await saver.tick()).toBe('failed');
    edit(store, 'o2');
    expect(await saver.tick()).toBe('failed');
    expect(store.getState().notices.filter((n) => n.level === 'warning')).toHaveLength(1);
    spy.mockImplementation(write);
    expect(await saver.tick()).toBe('saved');
    spy.mockRejectedValue('otra vez');
    edit(store, 'o3');
    await saver.tick();
    expect(store.getState().notices.filter((n) => n.level === 'warning')).toHaveLength(2);
  });

  it('el intervalo sale de la preferencia y se rearma al cambiarla', () => {
    const { store, saver, timers } = setup();
    saver.start();
    expect(timers).toHaveLength(1);
    expect(timers[0]!.ms).toBe(60_000);
    store.getState().setPref('autosaveSecs', 30);
    expect(timers).toHaveLength(2);
    expect(timers[0]!.cleared).toBe(true);
    expect(timers[1]!.ms).toBe(30_000);
    store.getState().setPref('autosave', false);
    expect(timers).toHaveLength(2);
    expect(timers[1]!.cleared).toBe(true);
    store.getState().setPref('autosave', true);
    expect(timers).toHaveLength(3);
    saver.stop();
    expect(timers[2]!.cleared).toBe(true);
  });

  it('una vuelta del temporizador ejecuta tick', async () => {
    const { store, saver, timers, writes } = setup();
    saver.start();
    edit(store);
    timers[0]!.fn();
    await vi.waitFor(() => expect(writes).toHaveBeenCalledTimes(1));
    saver.stop();
  });
});

describe('autoguardado por pestaña', () => {
  it('escribe una copia por cada pestaña con cambios, con id y nombre propios; las limpias no tienen copia', async () => {
    const { store, api, saver } = setup();
    store.getState().openInTab(docWith('a'), '/p/mesa.tlaku');
    const a = store.getState().activeTabId;
    edit(store, 'a2'); // A con cambios
    store.getState().newTab();
    edit(store, 'b1'); // B (Sin título 2) con cambios
    store.getState().newTab(); // C limpia
    expect(await saver.tick()).toBe('saved');
    const copies = await api.autosaveList();
    expect(copies.map((c) => c.name).sort()).toEqual(['Sin título 1', 'mesa.tlaku']);
    expect(new Set(copies.map((c) => c.id)).size).toBe(2);
    expect(copies.find((c) => c.name === 'mesa.tlaku')!.path).toBe('/p/mesa.tlaku');
    expect(saver.currentId(a)).toBe(copies.find((c) => c.name === 'mesa.tlaku')!.id);
  });

  it('cambiar de pestaña no cierra la sesión de la copia (la pestaña sigue con cambios)', async () => {
    const { store, api, saver, discards } = setup();
    saver.start();
    edit(store);
    const a = store.getState().activeTabId;
    await saver.tick();
    store.getState().newTab(); // A pasa a segundo plano con cambios; la activa (limpia) no
    store.getState().activateTab(a);
    store.getState().newTab();
    await new Promise((r) => setTimeout(r, 5));
    expect(discards).not.toHaveBeenCalled();
    expect(await api.autosaveList()).toHaveLength(1);
    saver.stop();
  });

  it('guardar una pestaña de fondo borra solo su copia', async () => {
    const { store, api, saver } = setup();
    saver.start();
    store.getState().openInTab(docWith('a'), '/p/a.tlaku');
    edit(store, 'a2');
    const a = store.getState().activeTabId;
    store.getState().newTab();
    edit(store, 'b1');
    await saver.tick();
    expect(await api.autosaveList()).toHaveLength(2);
    store.getState().runInTab(a, () => store.getState().markSaved('/p/a.tlaku'));
    await vi.waitFor(async () => expect((await api.autosaveList()).map((c) => c.name)).toEqual(['Sin título 1']));
    saver.stop();
  });

  it('cerrar una pestaña (sin importar cómo) borra su copia y deja las demás', async () => {
    const { store, api, saver } = setup();
    saver.start();
    edit(store, 'a1');
    const a = store.getState().activeTabId;
    store.getState().newTab();
    edit(store, 'b1');
    await saver.tick();
    store.getState().removeTab(a);
    await vi.waitFor(async () => expect((await api.autosaveList()).map((c) => c.name)).toEqual(['Sin título 2']));
    saver.stop();
  });

  it('cerrar la última pestaña con cambios descarta su copia (la pestaña vacía que queda es otra)', async () => {
    const { store, api, saver } = setup();
    saver.start();
    edit(store);
    await saver.tick();
    store.getState().removeTab(store.getState().activeTabId);
    await vi.waitFor(async () => expect(await api.autosaveList()).toEqual([]));
    saver.stop();
  });

  it('endAll (cerrar la ventana) borra las copias de todas las pestañas', async () => {
    const { store, api, saver } = setup();
    edit(store, 'a1');
    store.getState().newTab();
    edit(store, 'b1');
    await saver.tick();
    expect(await api.autosaveList()).toHaveLength(2);
    await saver.endAll();
    expect(await api.autosaveList()).toEqual([]);
  });

  it('una petición forzada (recuperar) escribe solo la pestaña pedida', async () => {
    const { store, saver, writes } = setup();
    store.getState().newTab();
    const b = store.getState().activeTabId;
    expect(await saver.tick(true, b)).toBe('saved');
    expect(writes).toHaveBeenCalledTimes(1);
  });
});

describe('recuperación por pestaña', () => {
  beforeEach(() => useRecovery.setState({ open: false, items: [], busy: null }));

  it('cada copia se recupera en su propia pestaña, con cambios sin guardar y sin tocar lo que ya había abierto', async () => {
    const s = setup();
    const d: RecoveryDeps = { api: s.api, store: s.store, writeNow: async (tabId) => (await s.saver.tick(true, tabId)) === 'saved' };
    s.store.getState().openInTab(docWith('abierto'), '/p/abierto.tlaku');
    await s.api.autosaveWrite({ id: 'c1', name: 'Mesa', path: '/p/mesa.tlaku', doc: docWith('m') });
    await s.api.autosaveWrite({ id: 'c2', name: 'Sin título', path: null, doc: docWith('n') });
    const items = await checkRecovery(d);
    expect(items).toHaveLength(2);
    expect(await recoverAll(d)).toBe(2);
    const st = s.store.getState();
    expect(st.tabs).toHaveLength(3);
    const byPath = Object.fromEntries(
      st.tabs.map((e) => {
        const data = e.data ?? { doc: st.doc, filePath: st.filePath, dirty: st.dirty };
        return [data.filePath ?? 'nuevo', { objs: data.doc.objects.map((o) => o.id), dirty: data.dirty }];
      }),
    );
    expect(byPath['/p/abierto.tlaku']).toEqual({ objs: ['abierto'], dirty: false });
    expect(byPath['/p/mesa.tlaku']).toEqual({ objs: ['m'], dirty: true });
    expect(byPath['nuevo']).toEqual({ objs: ['n'], dirty: true });
    // Las copias viejas se sustituyeron por una nueva por pestaña recuperada.
    const left = await s.api.autosaveList();
    expect(left.map((c) => c.id)).not.toContain('c1');
    expect(left.map((c) => c.id)).not.toContain('c2');
    expect(left).toHaveLength(2);
    expect(useRecovery.getState().open).toBe(false);
  });

  it('si el proyecto de la copia ya está abierto y sin cambios, la copia ocupa esa pestaña', async () => {
    const s = setup();
    const d: RecoveryDeps = { api: s.api, store: s.store, writeNow: async () => false };
    s.store.getState().openInTab(docWith('viejo'), '/p/mesa.tlaku');
    s.store.getState().newTab();
    await s.api.autosaveWrite({ id: 'c1', name: 'mesa.tlaku', path: '/p/mesa.tlaku', doc: docWith('rec') });
    const [info] = await checkRecovery(d);
    expect(await recoverCopy(info!, d)).toBe(true);
    const st = s.store.getState();
    expect(st.tabs).toHaveLength(2);
    expect(st.filePath).toBe('/p/mesa.tlaku');
    expect(st.doc.objects.map((o) => o.id)).toEqual(['rec']);
    expect(st.dirty).toBe(true);
  });
});

describe('recuperación', () => {
  beforeEach(() => useRecovery.setState({ open: false, items: [], busy: null }));

  function deps(over: Partial<RecoveryDeps> = {}) {
    const s = setup();
    const d: RecoveryDeps = {
      api: s.api,
      store: s.store,
      writeNow: async (tabId) => (await s.saver.tick(true, tabId)) === 'saved',
      ...over,
    };
    return { ...s, d };
  }

  it('sin copias no abre el diálogo', async () => {
    const { d } = deps();
    expect(await checkRecovery(d)).toEqual([]);
    expect(useRecovery.getState().open).toBe(false);
  });

  it('con copias abre el diálogo con la lista', async () => {
    const { d, api } = deps();
    await api.autosaveWrite({ id: 'x1', name: 'Mesa', path: null, doc: docWith('a') });
    const items = await checkRecovery(d);
    expect(items.map((i) => i.id)).toEqual(['x1']);
    expect(useRecovery.getState().open).toBe(true);
  });

  it('si no se puede revisar avisa y no abre nada', async () => {
    const { d, api, store } = deps();
    vi.spyOn(api, 'autosaveList').mockRejectedValue('sin acceso');
    expect(await checkRecovery(d)).toEqual([]);
    expect(store.getState().notices.at(-1)?.level).toBe('warning');
  });

  it('recuperar abre el documento con cambios sin guardar y borra la copia vieja solo cuando hay una nueva', async () => {
    const { d, api, store } = deps();
    await api.autosaveWrite({ id: 'viejo', name: 'Mesa', path: '/p/mesa.tlaku', doc: docWith('rec') });
    const [info] = await checkRecovery(d);
    expect(await recoverCopy(info!, d)).toBe(true);

    expect(store.getState().doc.objects.map((o) => o.id)).toEqual(['rec']);
    expect(store.getState().filePath).toBe('/p/mesa.tlaku');
    expect(store.getState().dirty).toBe(true);
    const left = (await api.autosaveList()).map((c) => c.id);
    expect(left).not.toContain('viejo'); // se borró la vieja
    expect(left).toHaveLength(1); // y hay una nueva del documento recuperado
    expect(useRecovery.getState().open).toBe(false);
  });

  it('si no se puede guardar la copia nueva, la vieja se conserva en el disco', async () => {
    const { d, api } = deps({ writeNow: async () => false });
    await api.autosaveWrite({ id: 'viejo', name: 'Mesa', path: null, doc: docWith('rec') });
    const [info] = await checkRecovery(d);
    await recoverCopy(info!, d);
    expect((await api.autosaveList()).map((c) => c.id)).toEqual(['viejo']);
  });

  it('una copia que no se puede leer avisa y sigue en la lista', async () => {
    const { d, api, store } = deps();
    await api.autosaveWrite({ id: 'x1', name: 'Mesa', path: null, doc: docWith('rec') });
    const [info] = await checkRecovery(d);
    vi.spyOn(api, 'autosaveRead').mockRejectedValue('dañada');
    expect(await recoverCopy(info!, d)).toBe(false);
    expect(store.getState().notices.at(-1)?.level).toBe('error');
    expect(useRecovery.getState().items).toHaveLength(1);
  });

  it('descartar borra la copia y cierra el diálogo al quedar vacío', async () => {
    const { d, api } = deps();
    await api.autosaveWrite({ id: 'a1', name: 'A', path: null, doc: docWith('a') });
    await api.autosaveWrite({ id: 'b1', name: 'B', path: null, doc: docWith('b') });
    await checkRecovery(d);
    await discardCopy('a1', d);
    expect(useRecovery.getState().items.map((i) => i.id)).toEqual(['b1']);
    expect(useRecovery.getState().open).toBe(true);
    await discardAll(d);
    expect(useRecovery.getState().open).toBe(false);
    expect(await api.autosaveList()).toEqual([]);
  });
});

describe('formato', () => {
  it('tamaños', () => {
    expect(formatBytes(500)).toBe('500 B');
    expect(formatBytes(2048)).toBe('2 KB');
    expect(formatBytes(3.5 * 1024 * 1024)).toBe('3,5 MB');
  });
  it('fecha en español', () => {
    const text = formatCopyDate(Date.UTC(2026, 9, 13, 15, 30));
    expect(text).toMatch(/2026/);
    expect(text).toMatch(/oct/i);
  });
});
