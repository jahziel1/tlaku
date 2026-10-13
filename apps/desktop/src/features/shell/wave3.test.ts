import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { commands } from '@/commands';
import { rectObj } from '@/features/canvas/testUtil';
import { normalizeDockLayout, setDockTabHidden } from '@/lib/dockLayout';
import { makeLayer } from '@/lib/layers';
import { createAppStore, useStore } from '@/store';
import { DEFAULT_PREFS, sanitizePrefs } from '@/store/prefs';
import { deleteTemplate, documentFromTemplate, newFromTemplate, saveAsTemplate } from './templates';
import { useTemplatesUi } from './templatesUi';

describe('notas del proyecto', () => {
  it('teclear seguido es un solo paso de deshacer y las notas viajan con el documento', () => {
    const store = createAppStore();
    store.getState().setNotes('M');
    store.getState().setNotes('Mad');
    store.getState().setNotes('Madera 3 mm');
    expect(store.getState().doc.notes).toBe('Madera 3 mm');
    expect(store.getState().past).toHaveLength(1);
    expect(store.getState().dirty).toBe(true);
    const saved = JSON.parse(JSON.stringify(store.getState().doc));
    const other = createAppStore();
    other.getState().replaceDocument(saved, null);
    expect(other.getState().doc.notes).toBe('Madera 3 mm');
    store.getState().undo();
    expect(store.getState().doc.notes ?? '').toBe('');
  });

  it('escribir lo mismo no crea pasos', () => {
    const store = createAppStore();
    store.getState().setNotes('');
    expect(store.getState().past).toHaveLength(0);
  });
});

describe('puentes e inicio siguen al objeto', () => {
  it('al mover o escalar, los puntos anclados al contorno se llevan consigo', () => {
    const store = createAppStore();
    store.getState().applyImport({ objects: [{ ...rectObj('r', 0, 0, 40, 40), manual_tabs: [[20, 0]], start: { point: [40, 40], reverse: false } }], new_layers: [makeLayer('#ff0000', 'Rojo')], warnings: [] });
    store.getState().moveObjects(['r'], 10, 5);
    const o = store.getState().doc.objects[0]!;
    expect(o.manual_tabs).toEqual([[30, 5]]);
    expect(o.start?.point).toEqual([50, 45]);
    store.getState().transformObjects(['r'], [2, 0, 0, 2, 0, 0]);
    expect(store.getState().doc.objects[0]!.manual_tabs).toEqual([[60, 10]]);
    store.getState().undo();
    expect(store.getState().doc.objects[0]!.manual_tabs).toEqual([[30, 5]]);
  });
});

describe('panel acoplable: orden y visibilidad en las preferencias', () => {
  it('la clave vive en el catálogo con su valor de fábrica y se sanea al leer', () => {
    expect(DEFAULT_PREFS.dockLayout.order).toContain('notes');
    expect(sanitizePrefs({ dockLayout: { order: ['camera'], hidden: ['zzz'] } }).dockLayout).toEqual(normalizeDockLayout({ order: ['camera'] }));
    expect(sanitizePrefs({ dockLayout: 'x' }).dockLayout).toEqual(DEFAULT_PREFS.dockLayout);
  });

  it('abrir una pestaña oculta (por su atajo) la vuelve a mostrar', () => {
    const store = createAppStore();
    store.getState().setPref('dockLayout', setDockTabHidden(store.getState().prefs.dockLayout, 'camera', true));
    expect(store.getState().prefs.dockLayout.hidden).toEqual(['camera']);
    store.getState().setDockTab('camera');
    expect(store.getState().dockTab).toBe('camera');
    expect(store.getState().prefs.dockLayout.hidden).toEqual([]);
  });
});

describe('comandos de la ola 3', () => {
  beforeEach(() => {
    useStore.getState().newDocument();
    useStore.getState().setTool('select');
    useStore.getState().select([]);
  });

  it('las herramientas de puentes e inicio tienen comando, atajo y paleta', () => {
    expect(commands.get('tool.tabs').shortcuts).toEqual(['U']);
    expect(commands.get('tool.start').shortcuts).toEqual(['Q']);
    useStore.getState().setTool('select');
    commands.get('tool.tabs').run(useStore.getState());
    expect(useStore.getState().activeTool).toBe('tabs');
  });

  it('Invertir sentido necesita un objeto con trazo y se deshace', () => {
    const s0 = useStore.getState();
    expect(commands.get('arrange.reverseDirection').enabled?.(s0)).toBe('reason.noSelection');
    s0.applyImport({ objects: [rectObj('r', 0, 0, 10, 10)], new_layers: [makeLayer('#ff0000', 'Rojo')], warnings: [] });
    s0.select(['r']);
    const cmd = commands.get('arrange.reverseDirection');
    expect(cmd.enabled?.(useStore.getState())).toBe(true);
    cmd.run(useStore.getState());
    expect(useStore.getState().doc.objects[0]!.start).toEqual({ point: null, reverse: true });
    expect(commands.get('arrange.clearStart').enabled?.(useStore.getState())).toBe(true);
    commands.get('arrange.clearStart').run(useStore.getState());
    expect(useStore.getState().doc.objects[0]!.start).toBeNull();
    expect(commands.get('arrange.clearTabs').enabled?.(useStore.getState())).toBe('canvas.reason.noTabs');
  });

  it('Mostrar guías y Mostrar inicio son interruptores del menú Ver', () => {
    const guides = commands.get('view.guides');
    expect(guides.checked?.(useStore.getState())).toBe(true);
    guides.run(useStore.getState());
    expect(guides.checked?.(useStore.getState())).toBe(false);
    guides.run(useStore.getState());
    const marks = commands.get('view.startMarks');
    expect(marks.checked?.(useStore.getState())).toBe(false);
    marks.run(useStore.getState());
    expect(useStore.getState().showStartMarks).toBe(true);
    marks.run(useStore.getState());
  });

  it('plantillas y ventana nueva abren sus diálogos o acciones', () => {
    commands.get('file.saveTemplate').run(useStore.getState());
    expect(useTemplatesUi.getState().dialog).toBe('save');
    commands.get('file.newFromTemplate').run(useStore.getState());
    expect(useTemplatesUi.getState().dialog).toBe('new');
    useTemplatesUi.getState().close();
    expect(commands.get('panel.notes').shortcuts).toEqual(['Mod+7']);
    commands.get('panel.notes').run(useStore.getState());
    expect(useStore.getState().dockTab).toBe('notes');
  });
});

describe('plantillas', () => {
  beforeEach(() => {
    setApi(createMockApi({ latencyMs: 0 }));
    useStore.getState().newDocument();
    useStore.getState().setNotes('');
    useStore.getState().clearHistory();
  });

  it('guarda el diseño, las capas y las notas, y un proyecto nuevo parte de ellos', async () => {
    const s = useStore.getState();
    s.applyImport({ objects: [rectObj('r', 0, 0, 10, 10)], new_layers: [makeLayer('#ff0000', 'Rojo')], warnings: [] });
    s.setNotes('Acrílico 3 mm');
    s.addGuide({ axis: 'x', pos: 5 });
    expect(await saveAsTemplate('Llavero')).toBe(true);
    expect((await (await import('@/api')).getApi().listTemplates()).map((x) => x.name)).toEqual(['Llavero']);

    useStore.getState().markSaved('/p/a.tlaku');
    useStore.getState().newDocument();
    expect(useStore.getState().doc.objects).toHaveLength(0);
    expect(await newFromTemplate('Llavero')).toBe(true);
    const d = useStore.getState().doc;
    expect(d.objects.map((o) => o.id)).toEqual(['r']);
    expect(d.layers[0]!.color).toBe('#ff0000');
    expect(d.notes).toBe('Acrílico 3 mm');
    expect(d.guides).toEqual([{ axis: 'x', pos: 5 }]);
    // Es un proyecto sin archivo y sin historial.
    expect(useStore.getState().filePath).toBeNull();
    expect(useStore.getState().past).toHaveLength(0);
    expect(useStore.getState().dirty).toBe(false);
  });

  it('un nombre inválido o una plantilla que no existe avisan con un error', async () => {
    expect(await saveAsTemplate('../fuera')).toBe(false);
    expect(useStore.getState().notices.at(-1)?.level).toBe('error');
    expect(await newFromTemplate('Fantasma')).toBe(false);
    expect(useStore.getState().notices.at(-1)?.text).toContain('Fantasma');
  });

  it('borra la plantilla y la lista queda vacía', async () => {
    await saveAsTemplate('Temporal');
    expect(await deleteTemplate('Temporal')).toBe(true);
    const { getApi } = await import('@/api');
    expect(await getApi().listTemplates()).toEqual([]);
  });

  it('el documento nuevo conserva la máquina activa si la plantilla no trae una', () => {
    const base = { version: 2 as const, objects: [], layers: [], device_id: null };
    expect(documentFromTemplate(base, 'k40').device_id).toBe('k40');
    expect(documentFromTemplate({ ...base, device_id: 'otra' }, 'k40').device_id).toBe('otra');
  });
});

describe('ajustar a la vista con copias virtuales', () => {
  it('la caja del diseño incluye las copias del arreglo', async () => {
    const { objectBoundsWithCopies } = await import('@/store/view');
    const o = { ...rectObj('r', 0, 0, 10, 10), array: { type: 'grid' as const, cols: 3, rows: 1, dx: 5, dy: 0 } };
    expect(objectBoundsWithCopies(o)).toEqual([[0, 0], [40, 10]]);
    expect(objectBoundsWithCopies(rectObj('r', 0, 0, 10, 10))).toEqual([[0, 0], [10, 10]]);
  });
});
