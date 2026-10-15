import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi, type MockApi } from '@/api/mock';
import { setApi } from '@/api';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import type { Document } from '@/api/types';
import { importPaths, saveProject, saveTab } from '@/features/history/files';
import { usePrompts } from '@/features/history/prompt';
import { commands } from '@/commands';
import { eventToCombo, formatCombo } from '@/commands/shortcuts';
import { tabViews, useStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { resetTabs } from '@/test/tabs';
import { closeOtherTabs, closeTab, confirmCloseWindow, saveAllTabs } from './actions';
import { encodeHandoff, parseHandoff, receiveHandoff, sendTabToNewWindow } from './handoff';

const s = () => useStore.getState();
let api: MockApi;

const docWith = (ids: string[]): Document => ({
  ...emptyDocument(),
  objects: sampleObjects().filter((o) => ids.includes(o.id)),
  layers: sampleLayers(),
});

beforeEach(() => {
  api = createMockApi({ latencyMs: 0 });
  setApi(api);
  resetTabs();
  for (const n of s().notices) s().dismissNotice(n.id);
});

afterEach(() => {
  // Una prueba que falla no debe dejar un diálogo abierto para la siguiente.
  while (usePrompts.getState().current) usePrompts.getState().answer(usePrompts.getState().current!.cancelId);
  vi.restoreAllMocks();
});

/** Abre una pestaña con archivo y la ensucia (borra un objeto). */
function dirtyTab(path: string): string {
  s().openInTab(docWith(['obj-placa', 'obj-foto']), path);
  const id = s().activeTabId;
  s().deleteObjects(['obj-placa']);
  return id;
}

const answerNext = async (choice: string) => {
  await vi.waitFor(() => expect(usePrompts.getState().current).not.toBeNull());
  const asked = usePrompts.getState().current!;
  usePrompts.getState().answer(choice);
  return asked;
};

describe('cerrar una pestaña', () => {
  it('sin cambios se cierra sin preguntar', async () => {
    s().newTab();
    expect(await closeTab(s().activeTabId)).toBe(true);
    expect(usePrompts.getState().current).toBeNull();
    expect(s().tabs).toHaveLength(1);
  });

  it('con cambios pregunta, y cancelar la deja abierta y con sus cambios', async () => {
    const id = dirtyTab('/p/a.tlaku');
    s().newTab();
    const closing = closeTab(id);
    const asked = await answerNext('cancel');
    expect(asked.message).toContain('a.tlaku');
    expect(await closing).toBe(false);
    expect(s().tabs).toHaveLength(2);
    expect(s().activeTabId).toBe(id); // se muestra la pestaña por la que se pregunta
    expect(s().dirty).toBe(true);
  });

  it('«No guardar» la cierra sin escribir el archivo', async () => {
    const id = dirtyTab('/p/a.tlaku');
    s().newTab();
    const save = vi.spyOn(api, 'saveProject');
    const closing = closeTab(id);
    await answerNext('discard');
    expect(await closing).toBe(true);
    expect(save).not.toHaveBeenCalled();
    expect(s().tabs.map((e) => e.id)).not.toContain(id);
  });

  it('«Guardar» escribe el documento de ESA pestaña y luego la cierra', async () => {
    const id = dirtyTab('/p/a.tlaku');
    const other = s().newTab();
    s().activateTab(id);
    const save = vi.spyOn(api, 'saveProject');
    const closing = closeTab(id);
    await answerNext('save');
    expect(await closing).toBe(true);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0]![0]).toMatchObject({ path: '/p/a.tlaku' });
    expect(save.mock.calls[0]![0].doc.objects.map((o) => o.id)).toEqual(['obj-foto']);
    expect(s().activeTabId).toBe(other);
  });

  it('si el guardado falla, la pestaña no se cierra', async () => {
    const id = dirtyTab('/p/a.tlaku');
    s().newTab();
    vi.spyOn(api, 'saveProject').mockRejectedValue('disco lleno');
    const closing = closeTab(id);
    await answerNext('save');
    expect(await closing).toBe(false);
    expect(s().tabs.map((e) => e.id)).toContain(id);
    expect(s().notices.at(-1)).toMatchObject({ level: 'error' });
  });

  it('una pestaña de fondo con cambios se activa para preguntar por ella', async () => {
    const id = dirtyTab('/p/a.tlaku');
    s().newTab();
    expect(s().activeTabId).not.toBe(id);
    const closing = closeTab(id);
    await vi.waitFor(() => expect(usePrompts.getState().current).not.toBeNull());
    expect(s().activeTabId).toBe(id);
    usePrompts.getState().answer('cancel');
    await closing;
  });
});

describe('cerrar las demás y cerrar la ventana', () => {
  it('cerrar las demás con varias sucias hace UNA pregunta con la lista; «Descartar todas» las cierra', async () => {
    dirtyTab('/p/a.tlaku');
    dirtyTab('/p/b.tlaku');
    const keep = s().newTab();
    const closing = closeOtherTabs(keep);
    const asked = await answerNext('discard');
    expect(asked.items).toEqual(['a.tlaku', 'b.tlaku']);
    expect(asked.choices.map((c) => c.id)).toEqual(['save', 'discard', 'cancel']);
    expect(await closing).toBe(true);
    expect(s().tabs.map((e) => e.id)).toEqual([keep]);
  });

  it('«Guardar todas» guarda cada una y para en la primera que falla', async () => {
    dirtyTab('/p/a.tlaku');
    dirtyTab('/p/b.tlaku');
    dirtyTab('/p/c.tlaku');
    const save = vi.spyOn(api, 'saveProject').mockImplementation(async ({ path }) => {
      if (path === '/p/b.tlaku') throw new Error('sin permiso');
    });
    const closing = confirmCloseWindow();
    await answerNext('save');
    expect(await closing).toBe(false);
    expect(save.mock.calls.map((c) => c[0].path)).toEqual(['/p/a.tlaku', '/p/b.tlaku']);
    expect(tabViews(s()).map((v) => v.dirty)).toEqual([false, true, true]);
  });

  it('cerrar la ventana con varias sucias: «Guardar todas» las deja limpias y permite cerrar', async () => {
    dirtyTab('/p/a.tlaku');
    dirtyTab('/p/b.tlaku');
    const closing = confirmCloseWindow();
    const asked = await answerNext('save');
    expect(asked.title).toContain('2');
    expect(await closing).toBe(true);
    expect(tabViews(s()).every((v) => !v.dirty)).toBe(true);
  });

  it('cerrar la ventana con varias sucias: cancelar no toca nada', async () => {
    dirtyTab('/p/a.tlaku');
    dirtyTab('/p/b.tlaku');
    const closing = confirmCloseWindow();
    await answerNext('cancel');
    expect(await closing).toBe(false);
    expect(tabViews(s()).filter((v) => v.dirty)).toHaveLength(2);
  });

  it('cerrar la ventana con UNA sucia usa el diálogo de siempre (con el nombre del documento)', async () => {
    dirtyTab('/p/a.tlaku');
    s().newTab();
    const closing = confirmCloseWindow();
    const asked = await answerNext('discard');
    expect(asked.title).toBe('Cambios sin guardar');
    expect(asked.message).toContain('a.tlaku');
    expect(asked.message).toContain('cerrar Tlaku');
    expect(await closing).toBe(true);
  });

  it('sin cambios en ninguna pestaña deja cerrar sin preguntar', async () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    s().newTab();
    expect(await confirmCloseWindow()).toBe(true);
    expect(usePrompts.getState().current).toBeNull();
  });

  it('guardar todas guarda solo las sucias y las deja limpias', async () => {
    dirtyTab('/p/a.tlaku');
    s().openInTab(docWith(['obj-placa']), '/p/limpia.tlaku');
    dirtyTab('/p/c.tlaku');
    const save = vi.spyOn(api, 'saveProject');
    expect(await saveAllTabs()).toBe(true);
    expect(save.mock.calls.map((c) => c[0].path).sort()).toEqual(['/p/a.tlaku', '/p/c.tlaku']);
    expect(tabViews(s()).every((v) => !v.dirty)).toBe(true);
  });
});

describe('guardar e importar sin equivocarse de pestaña', () => {
  it('guardar una pestaña de fondo con archivo no cambia la activa', async () => {
    const a = dirtyTab('/p/a.tlaku');
    const b = s().newTab();
    expect(await saveTab(a)).toBe(true);
    expect(s().activeTabId).toBe(b);
    expect(tabViews(s()).find((v) => v.id === a)!.dirty).toBe(false);
  });

  it('guardar una pestaña sin archivo la activa para elegir dónde y deja guardada ESA', async () => {
    s().newTab();
    s().applyImport({ objects: sampleObjects().slice(0, 1), new_layers: [], warnings: [] });
    const untitled = s().activeTabId;
    s().newTab();
    expect(await saveTab(untitled)).toBe(true);
    expect(s().activeTabId).toBe(untitled);
    expect(s().filePath).toMatch(/\.tlaku$/);
    expect(s().dirty).toBe(false);
  });

  it('si se cambia de pestaña mientras se elige el archivo, se guarda la pestaña que lo pidió', async () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    const a = s().activeTabId;
    s().deleteObjects(['obj-placa']);
    // «Guardar como»: el selector tarda; mientras tanto el usuario va a otra pestaña.
    const saving = saveProject(true);
    const b = s().newTab();
    expect(await saving).toBe(true);
    expect(s().activeTabId).toBe(b);
    expect(tabViews(s()).find((v) => v.id === a)).toMatchObject({ dirty: false, filePath: 'proyectos/sin-titulo.tlaku' });
    expect(tabViews(s()).find((v) => v.id === b)).toMatchObject({ dirty: false, filePath: null });
  });

  it('una importación lenta termina en la pestaña donde empezó, aunque se cambie de pestaña', async () => {
    setApi(createMockApi({ latencyMs: 30 }));
    const a = s().activeTabId;
    const importing = importPaths(['ejemplos/placa.svg']);
    const b = s().newTab(); // el usuario se va mientras el motor trabaja
    const n = await importing;
    expect(n).toBeGreaterThan(0);
    expect(s().activeTabId).toBe(b);
    expect(s().doc.objects).toHaveLength(0);
    expect(tabViews(s()).find((v) => v.id === b)!.dirty).toBe(false);
    s().activateTab(a);
    expect(s().doc.objects.length).toBeGreaterThan(0);
    expect(s().selectedIds.length).toBeGreaterThan(0);
    expect(s().dirty).toBe(true);
  });

  it('si la pestaña de destino se cierra mientras se importa, avisa y no escribe en otra', async () => {
    setApi(createMockApi({ latencyMs: 30 }));
    const a = s().activeTabId;
    const b = s().newTab();
    s().activateTab(a);
    const importing = importPaths(['ejemplos/placa.svg']);
    s().removeTab(a);
    expect(await importing).toBe(0);
    expect(s().activeTabId).toBe(b);
    expect(s().doc.objects).toHaveLength(0);
    expect(s().notices.at(-1)).toMatchObject({ level: 'warning' });
  });
});

describe('mover a ventana nueva', () => {
  it('codifica y lee el parámetro, y rechaza lo mal formado', () => {
    const h = { id: 'handoff-abc_1', path: '/p/a.tlaku', dirty: true };
    expect(parseHandoff(`?handoff=${encodeHandoff(h)}`)).toEqual(h);
    expect(parseHandoff('')).toBeNull();
    expect(parseHandoff('?handoff=%7Bmal')).toBeNull();
    expect(parseHandoff(`?handoff=${encodeURIComponent(JSON.stringify({ id: '../x', path: null, dirty: false }))}`)).toBeNull();
    expect(parseHandoff(`?handoff=${encodeURIComponent(JSON.stringify({ id: 'ok', path: 5, dirty: 'sí' }))}`)).toEqual({ id: 'ok', path: null, dirty: false });
  });

  it('con una sola pestaña no hay a dónde moverla', async () => {
    expect(await sendTabToNewWindow(s().activeTabId)).toBe(false);
  });

  it('la pestaña viaja como copia con id propio, se abre la ventana y aquí se cierra', async () => {
    const a = dirtyTab('/p/a.tlaku');
    s().newTab();
    const write = vi.spyOn(api, 'autosaveWrite');
    const open = vi.spyOn(api, 'newWindow').mockResolvedValue('doc-1');
    expect(await sendTabToNewWindow(a)).toBe(true);
    const sent = write.mock.calls[0]![0];
    expect(sent.doc.objects.map((o) => o.id)).toEqual(['obj-foto']);
    expect(sent.path).toBe('/p/a.tlaku');
    const arg = open.mock.calls[0]![0]!;
    expect(parseHandoff(`?handoff=${arg.handoff}`)).toEqual({ id: sent.id, path: '/p/a.tlaku', dirty: true });
    expect(s().tabs.map((e) => e.id)).not.toContain(a);
  });

  it('si la ventana no se puede abrir, la pestaña se queda y la copia se borra', async () => {
    const a = dirtyTab('/p/a.tlaku');
    s().newTab();
    vi.spyOn(api, 'newWindow').mockRejectedValue('sin memoria');
    const discard = vi.spyOn(api, 'autosaveDiscard');
    expect(await sendTabToNewWindow(a)).toBe(false);
    expect(s().tabs.map((e) => e.id)).toContain(a);
    expect(discard).toHaveBeenCalled();
    expect(s().notices.at(-1)).toMatchObject({ level: 'error' });
  });

  it('la ventana nueva recoge la pestaña, conserva lo sin guardar y borra la copia', async () => {
    const doc = docWith(['obj-foto']);
    await api.autosaveWrite({ id: 'handoff-x1', name: 'a.tlaku', path: '/p/a.tlaku', doc });
    expect(await receiveHandoff(`?handoff=${encodeHandoff({ id: 'handoff-x1', path: '/p/a.tlaku', dirty: true })}`)).toBe(true);
    expect(s().filePath).toBe('/p/a.tlaku');
    expect(s().dirty).toBe(true);
    expect(s().doc.objects.map((o) => o.id)).toEqual(['obj-foto']);
    expect(await api.autosaveList()).toEqual([]);
  });

  it('sin parámetro no hace nada', async () => {
    expect(await receiveHandoff('')).toBe(false);
    expect(s().tabs).toHaveLength(1);
  });
});

describe('atajos', () => {
  it('Cmd/Ctrl+T abre, Cmd/Ctrl+W cierra, Ctrl+Tab y Ctrl+Mayús+Tab cambian', () => {
    expect(commands.match('Mod+T')?.id).toBe('tabs.new');
    expect(commands.match('Mod+W')?.id).toBe('tabs.close');
    expect(commands.match('Mod+Tab')?.id).toBe('tabs.next');
    expect(commands.match('Mod+Shift+Tab')?.id).toBe('tabs.prev');
  });

  it('Ctrl+Tab en macOS llega como Mod+Tab (Cmd+Tab es del sistema) y se muestra con ⌃', () => {
    const ev = { key: 'Tab', code: 'Tab', ctrlKey: true, metaKey: false, altKey: false, shiftKey: false };
    expect(eventToCombo(ev, true)).toBe('Mod+Tab');
    expect(eventToCombo({ ...ev, shiftKey: true }, true)).toBe('Mod+Shift+Tab');
    expect(eventToCombo(ev, false)).toBe('Mod+Tab');
    // Otras combinaciones con Ctrl en macOS siguen sin valer.
    expect(eventToCombo({ ...ev, key: 'a', code: 'KeyA' }, true)).toBeNull();
    expect(formatCombo('Mod+Tab', true)).toBe('⌃Tab');
    expect(formatCombo('Mod+Shift+Tab', true)).toBe('⌃⇧Tab');
    expect(formatCombo('Mod+Tab', false)).toBe('Ctrl+Tab');
  });

  it('Cmd/Ctrl+1..7 y Cmd+Opt+flechas ya tienen dueño (paneles y movimiento de la máquina): no se reasignan a pestañas', () => {
    for (const n of [1, 2, 3, 4, 5, 6, 7]) {
      const owner = commands.match(`Mod+${n}`)?.id ?? '';
      expect(owner.startsWith('tabs.'), `Mod+${n}`).toBe(false);
      expect(owner, `Mod+${n}`).not.toBe('');
    }
    for (const arrow of ['Left', 'Right', 'Up', 'Down']) {
      expect(commands.match(`Mod+Alt+Arrow${arrow}`)?.id).toMatch(/^machine\.jog\./);
    }
  });

  it('los comandos de pestañas actúan sobre la activa y avisan si no hay con qué', async () => {
    expect(commands.availability('tabs.next', s())).toBe('tabs.reason.onlyOne');
    expect(commands.availability('tabs.moveToWindow', s())).toBe('tabs.reason.onlyOne');
    expect(commands.availability('tabs.closeOthers', s())).toBe('tabs.reason.onlyOne');
    expect(commands.availability('tabs.saveAll', s())).toBe('tabs.reason.nothingToSave');
    const first = s().activeTabId;
    await commands.run('tabs.new');
    expect(s().tabs).toHaveLength(2);
    expect(commands.availability('tabs.next', s())).toBe(true);
    await commands.run('tabs.next');
    expect(s().activeTabId).toBe(first);
    await commands.run('tabs.prev');
    expect(s().activeTabId).not.toBe(first);
    await commands.run('tabs.close');
    expect(s().tabs).toHaveLength(1);
    expect(s().activeTabId).toBe(first);
  });

  it('duplicar desde el comando avisa y deja la copia activa', async () => {
    s().openInTab(docWith(['obj-placa']), '/p/a.tlaku');
    await commands.run('tabs.duplicate');
    expect(s().tabs).toHaveLength(2);
    expect(s().filePath).toBeNull();
    expect(s().notices.at(-1)).toMatchObject({ level: 'info' });
  });
});
