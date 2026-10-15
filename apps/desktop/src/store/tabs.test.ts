import { beforeEach, describe, expect, it } from 'vitest';
import type { Document, JobPreview } from '@/api/types';
import { createMockApi } from '@/api/mock';
import { setApi } from '@/api';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { createAppStore, findTabByPath, isPristine, pinTab, selectJobPreview, tabDoc, tabViews } from './index';
import { emptyDocument } from './document';
import { INACTIVE_HISTORY_STEPS, INACTIVE_POINT_BUDGET } from './tabs';

let store: ReturnType<typeof createAppStore>;
const s = () => store.getState();

beforeEach(() => {
  setApi(createMockApi({ latencyMs: 0 }));
  store = createAppStore();
});

const docWith = (ids: string[]): Document => ({
  ...emptyDocument(),
  objects: sampleObjects().filter((o) => ids.includes(o.id)),
  layers: sampleLayers(),
});
const names = () => tabViews(s()).map((v) => v.filePath ?? `#${v.untitled}`);

describe('pestañas: creación y cambio', () => {
  it('arranca con una sola pestaña vacía, activa y sin cambios', () => {
    expect(s().tabs).toHaveLength(1);
    expect(s().tabs[0]!.id).toBe(s().activeTabId);
    expect(s().tabs[0]!.data).toBeNull(); // la activa vive en los slices
    expect(isPristine(s())).toBe(true);
  });

  it('abrir en una pestaña vacía y sin cambios la reemplaza', () => {
    const first = s().activeTabId;
    expect(s().openInTab(docWith(['obj-placa']), '/p/a.tlaku')).toBe(first);
    expect(s().tabs).toHaveLength(1);
    expect(s().filePath).toBe('/p/a.tlaku');
    expect(s().dirty).toBe(false);
  });

  it('abrir con la actual ocupada crea una pestaña nueva y la activa; la anterior queda intacta', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    const b = s().openInTab(docWith(['obj-foto']), '/p/b.tlaku');
    expect(b).not.toBe(a);
    expect(s().activeTabId).toBe(b);
    expect(s().doc.objects.map((o) => o.id)).toEqual(['obj-foto']);
    expect(s().tabs.find((e) => e.id === a)!.data!.doc.objects.map((o) => o.id)).toEqual(['obj-placa']);
    expect(names()).toEqual(['/p/a.tlaku', '/p/b.tlaku']);
  });

  it('«Sin título N» usa el menor número libre y se reutiliza al cerrar', () => {
    s().newTab();
    s().newTab();
    expect(tabViews(s()).map((v) => v.untitled)).toEqual([1, 2, 3]);
    s().removeTab(tabViews(s())[1]!.id);
    s().newTab();
    expect(tabViews(s()).map((v) => v.untitled)).toEqual([1, 3, 2]);
  });

  it('cada pestaña conserva su archivo y sus cambios sin guardar al ir y volver', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    s().deleteObjects(['obj-placa']);
    expect(s().dirty).toBe(true);
    const b = s().newTab();
    expect(s().filePath).toBeNull();
    expect(s().dirty).toBe(false);
    s().activateTab(a);
    expect(s().filePath).toBe('/p/a.tlaku');
    expect(s().dirty).toBe(true);
    expect(tabViews(s()).find((v) => v.id === b)!.dirty).toBe(false);
  });

  it('markSaved de una pestaña no toca a las otras', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    s().deleteObjects(['obj-placa']);
    const b = s().newTab();
    s().applyImport({ objects: sampleObjects().slice(0, 1), new_layers: [], warnings: [] });
    expect(tabViews(s()).map((v) => v.dirty)).toEqual([true, true]);
    s().runInTab(a, () => s().markSaved('/p/a.tlaku'));
    expect(s().activeTabId).toBe(b);
    expect(tabViews(s()).map((v) => v.dirty)).toEqual([false, true]);
  });
});

describe('pestañas: aislamiento', () => {
  it('deshacer y rehacer son por pestaña', () => {
    s().openInTab(docWith(['obj-placa', 'obj-foto']), '/p/a.tlaku');
    const a = s().activeTabId;
    s().deleteObjects(['obj-placa']);
    const b = s().newTab();
    s().applyImport({ objects: sampleObjects().slice(0, 2), new_layers: [], warnings: [] });
    expect(s().past).toHaveLength(1);
    s().undo();
    expect(s().doc.objects).toHaveLength(0);
    expect(s().canRedo()).toBe(true);
    s().activateTab(a);
    expect(s().past).toHaveLength(1);
    expect(s().canRedo()).toBe(false);
    expect(s().doc.objects.map((o) => o.id)).toEqual(['obj-foto']);
    s().undo();
    expect(s().doc.objects.map((o) => o.id).sort()).toEqual(['obj-foto', 'obj-placa']);
    expect(s().dirty).toBe(false); // volvió al estado guardado de SU archivo
    s().activateTab(b);
    expect(s().canRedo()).toBe(true); // lo rehacible de la otra pestaña sigue ahí
  });

  it('la selección es por pestaña', () => {
    s().openInTab(docWith(['obj-placa', 'obj-foto']), '/p/a.tlaku');
    const a = s().activeTabId;
    s().select(['obj-foto']);
    s().newTab();
    expect(s().selectedIds).toEqual([]);
    s().activateTab(a);
    expect(s().selectedIds).toEqual(['obj-foto']);
  });

  it('zoom y paneo son por pestaña', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    store.setState({ zoom: 4, panX: 123, panY: 456, autoFit: false });
    const b = s().newTab();
    store.setState({ zoom: 0.5, panX: -9, panY: 77, autoFit: false });
    s().activateTab(a);
    expect([s().zoom, s().panX, s().panY]).toEqual([4, 123, 456]);
    s().activateTab(b);
    expect([s().zoom, s().panX, s().panY]).toEqual([0.5, -9, 77]);
  });

  it('el estado de la vista previa es por pestaña, y un cálculo en vuelo no pisa a la otra', async () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    s().setPreviewVisible(true);
    s().setScrub(0.3);
    s().setJobOptions({ job_origin: 4 });
    const pending = s().computePreview(); // empieza en A...
    const b = s().newTab(); // ...y el usuario se va a B
    expect(s().previewVisible).toBe(false);
    expect(s().scrub).toBe(1);
    await pending;
    // El resultado de A no se aplicó a B.
    expect(s().activeTabId).toBe(b);
    expect(s().preview).toBeNull();
    expect(s().previewStatus).toBe('idle');
    s().activateTab(a);
    expect(s().previewVisible).toBe(true);
    expect(s().scrub).toBe(0.3);
    expect(s().jobOptions.job_origin).toBe(4);
    // A quedó marcada para recalcular (su cálculo se abandonó).
    expect(s().previewStale).toBe(true);
  });

  it('las opciones del trabajo de una pestaña nueva heredan las actuales sin la selección ni el tiling', () => {
    s().setJobOptions({ job_origin: 2, user_origin: [5, 6], tile: null });
    s().newTab();
    expect(s().jobOptions.job_origin).toBe(2);
    expect(s().jobOptions.user_origin).toEqual([5, 6]);
    expect(s().jobOptions.selection).toBeNull();
  });

  it('el portapapeles es de la ventana: se copia en una pestaña y se pega en otra', () => {
    s().openInTab(docWith(['obj-placa', 'obj-foto']), '/p/a.tlaku');
    expect(s().copyObjects(['obj-foto'])).toBe(1);
    s().newTab();
    expect(s().doc.objects).toHaveLength(0);
    const pasted = s().pasteObjects();
    expect(pasted).toHaveLength(1);
    expect(s().doc.objects).toHaveLength(1);
    expect(s().doc.layers.length).toBeGreaterThan(0); // trae los ajustes de su capa
    expect(s().dirty).toBe(true);
  });

  it('un cambio de dispositivo no se pierde y el activo es de la ventana', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const dev = s().activeDeviceId;
    s().newTab();
    expect(s().activeDeviceId).toBe(dev);
  });
});

describe('pestañas: cerrar, mover, duplicar', () => {
  it('cerrar la activa pasa a la vecina de la derecha, o a la izquierda si era la última', () => {
    const a = s().activeTabId;
    const b = s().newTab();
    const c = s().newTab();
    s().activateTab(b);
    expect(s().removeTab(b)).toBe(c);
    expect(s().tabs.map((e) => e.id)).toEqual([a, c]);
    expect(s().removeTab(c)).toBe(a);
    expect(s().tabs.map((e) => e.id)).toEqual([a]);
  });

  it('cerrar una pestaña de fondo no cambia la activa', () => {
    const a = s().activeTabId;
    const b = s().newTab();
    s().activateTab(a);
    s().removeTab(b);
    expect(s().activeTabId).toBe(a);
    expect(s().tabs).toHaveLength(1);
  });

  it('cerrar la última deja una pestaña vacía nueva (otro id: su copia de recuperación se descarta)', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const old = s().activeTabId;
    const fresh = s().removeTab(old);
    expect(fresh).not.toBe(old);
    expect(s().tabs).toHaveLength(1);
    expect(isPristine(s())).toBe(true);
    expect(s().filePath).toBeNull();
  });

  it('reordenar mueve la pestaña y deja la activa como estaba', () => {
    const a = s().activeTabId;
    const b = s().newTab();
    const c = s().newTab();
    s().moveTab(c, 0);
    expect(s().tabs.map((e) => e.id)).toEqual([c, a, b]);
    expect(s().activeTabId).toBe(c);
    s().moveTab(c, 99);
    expect(s().tabs.map((e) => e.id)).toEqual([a, b, c]);
  });

  it('siguiente y anterior dan la vuelta', () => {
    const a = s().activeTabId;
    const b = s().newTab();
    expect(s().activeTabId).toBe(b);
    s().cycleTab(1);
    expect(s().activeTabId).toBe(a);
    s().cycleTab(-1);
    expect(s().activeTabId).toBe(b);
  });

  it('con una sola pestaña, siguiente no hace nada', () => {
    const a = s().activeTabId;
    s().cycleTab(1);
    expect(s().activeTabId).toBe(a);
    expect(s().tabSwitches).toBe(0);
  });

  it('duplicar copia el documento a una pestaña nueva sin archivo y con cambios sin guardar', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    s().select(['obj-placa']);
    const copy = s().duplicateTab(a)!;
    expect(s().activeTabId).toBe(copy);
    expect(s().tabs.map((e) => e.id)).toEqual([a, copy]);
    expect(s().filePath).toBeNull();
    expect(s().dirty).toBe(true);
    expect(s().past).toHaveLength(0);
    expect(s().selectedIds).toEqual([]);
    expect(s().doc.objects.map((o) => o.id)).toEqual(['obj-placa']);
    // El original sigue igual.
    expect(tabViews(s())[0]).toMatchObject({ filePath: '/p/a.tlaku', dirty: false });
  });

  it('busca una pestaña por ruta, también con otra barra', () => {
    s().openInTab(docWith(['obj-placa']), 'C:\\p\\a.tlaku');
    s().newTab();
    expect(findTabByPath(s(), 'C:/p/a.tlaku')).toBe(s().tabs[0]!.id);
    expect(findTabByPath(s(), '/otro.tlaku')).toBeNull();
  });
});

describe('pestañas: operaciones asíncronas', () => {
  it('runInTab escribe en la pestaña pedida y restaura la activa (con su deshacer y sus marcas)', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    const b = s().newTab();
    s().select([]);
    store.setState({ zoom: 3, panX: 10, panY: 20, autoFit: false });
    const ids = s().runInTab(a, () => s().applyImport({ objects: sampleObjects().slice(1, 3), new_layers: [], warnings: [] }));
    expect(ids).toHaveLength(2);
    expect(s().activeTabId).toBe(b);
    expect(s().doc.objects).toHaveLength(0);
    expect([s().zoom, s().panX, s().panY]).toEqual([3, 10, 20]);
    const tabA = s().tabs.find((e) => e.id === a)!.data!;
    expect(tabA.doc.objects).toHaveLength(3);
    expect(tabA.dirty).toBe(true);
    expect(tabA.past).toHaveLength(1);
    s().activateTab(a);
    s().undo();
    expect(s().doc.objects.map((o) => o.id)).toEqual(['obj-placa']);
  });

  it('runInTab de una pestaña cerrada no ejecuta nada', () => {
    const a = s().activeTabId;
    const b = s().newTab();
    s().removeTab(a);
    let ran = false;
    expect(s().runInTab(a, () => (ran = true))).toBeUndefined();
    expect(ran).toBe(false);
    expect(s().activeTabId).toBe(b);
  });

  it('runInTab no cuenta como cambio del usuario (no sube tabSwitches) ni recorta el historial', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    for (let i = 0; i < INACTIVE_HISTORY_STEPS + 20; i++) s().moveObjects(['obj-placa'], 1, 0);
    s().newTab();
    const switches = s().tabSwitches;
    s().runInTab(a, () => undefined);
    expect(s().tabSwitches).toBe(switches);
    // El recorte ocurrió al pasar a segundo plano (cambio del usuario), no por el vaivén de runInTab.
    expect(s().tabs.find((e) => e.id === a)!.data!.past.length).toBeLessThanOrEqual(INACTIVE_HISTORY_STEPS);
  });

  it('pinTab fija la pestaña de partida: un resultado tardío cae en ella aunque haya otra activa', async () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    const inTab = pinTab(s());
    const b = s().newTab();
    await new Promise((r) => setTimeout(r, 5));
    inTab(() => s().deleteObjects(['obj-placa']));
    expect(s().activeTabId).toBe(b);
    expect(s().doc.objects).toHaveLength(0);
    expect(tabDoc(s(), a)!.objects).toHaveLength(0);
    expect(tabViews(s()).find((v) => v.id === a)!.dirty).toBe(true);
    expect(tabViews(s()).find((v) => v.id === b)!.dirty).toBe(false);
  });

  it('el trabajo en curso recuerda su pestaña y su vista previa aunque se cambie de pestaña', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    const preview = { total_s: 42, moves: [], layers: [] } as unknown as JobPreview;
    s().noteJobOrigin({ tabId: a, name: 'a.tlaku', path: '/p/a.tlaku', preview });
    s().newTab();
    expect(selectJobPreview(s())).toBe(preview);
    s().noteJobOrigin(null);
    expect(selectJobPreview(s())).toBeNull();
  });
});

describe('pestañas: memoria del historial de las inactivas', () => {
  it('al pasar a segundo plano una pestaña conserva a lo sumo INACTIVE_HISTORY_STEPS pasos; la activa no se recorta', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    for (let i = 0; i < 100; i++) s().moveObjects(['obj-placa'], 1, 0);
    expect(s().past.length).toBe(100);
    s().newTab();
    expect(s().tabs.find((e) => e.id === a)!.data!.past).toHaveLength(INACTIVE_HISTORY_STEPS);
    // El recorte conserva los pasos más recientes: deshacer 30 veces vuelve exactamente 30 pasos atrás.
    const before = s().tabs.find((e) => e.id === a)!.data!.doc.objects[0]!.transform[4];
    s().activateTab(a);
    for (let i = 0; i < INACTIVE_HISTORY_STEPS; i++) s().undo();
    expect(s().doc.objects[0]!.transform[4]).toBe(before - INACTIVE_HISTORY_STEPS);
    expect(s().canUndo()).toBe(false);
  });

  it('el presupuesto de puntos recorta antes que el tope de pasos', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    // Pasos «caros»: cada uno retiene geometría que el documento ya no usa.
    store.setState({
      past: Array.from({ length: 20 }, (_, i) => ({ label: 'history.move' as const, doc: emptyDocument(), at: i, cost: INACTIVE_POINT_BUDGET / 4 })),
    });
    s().newTab();
    const kept = s().tabs.find((e) => e.id === a)!.data!.past.length;
    expect(kept).toBeLessThan(20);
    expect(kept).toBeGreaterThanOrEqual(4);
    expect(kept).toBeLessThanOrEqual(5);
  });

  it('lo rehacible también se acota', () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    for (let i = 0; i < 80; i++) s().moveObjects(['obj-placa'], 1, 0);
    s().jumpTo(0);
    expect(s().future.length).toBe(80);
    s().newTab();
    expect(s().tabs.find((e) => e.id === a)!.data!.future).toHaveLength(INACTIVE_HISTORY_STEPS);
  });
});
