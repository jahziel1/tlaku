import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import type { DesignObject } from '@/api/types';
import { commands } from '@/commands';
import { rectObj } from '@/features/canvas/testUtil';
import { createAppStore, useStore } from '@/store';
import { ContextMenuHost } from './ContextMenu';
import { buildContextMenu, type MenuEntry, type MenuHooks } from './model';
import { closeContextMenu, openContextMenu, useContextMenu } from './uiStore';

const hooks: MenuHooks = { editText: () => {}, textToPaths: () => {}, has: (id) => commands.has(id) };
const flat = (entries: MenuEntry[]): MenuEntry[] => entries.flatMap((e) => (e.kind === 'submenu' ? [e, ...flat(e.items)] : [e]));
const ids = (entries: MenuEntry[]) => flat(entries).flatMap((e) => (e.kind === 'command' ? [e.id] : []));

function stateWith(objects: DesignObject[], selected: string[]) {
  const store = createAppStore();
  store.getState().applyImport({ objects, new_layers: [], warnings: [] });
  store.getState().select(selected);
  return store.getState();
}

describe('menú contextual: contenido', () => {
  it('sobre objetos trae editar, agrupar, bloquear, orden, voltear y seleccionar', () => {
    const entries = buildContextMenu(stateWith([rectObj('a', 0, 0, 5, 5), rectObj('b', 6, 0, 9, 5)], ['a', 'b']), true, hooks);
    const list = ids(entries);
    for (const id of ['edit.cut', 'edit.copy', 'edit.paste', 'edit.delete', 'edit.group', 'edit.ungroup', 'arrange.lock', 'arrange.order.front', 'arrange.flip.h', 'arrange.mirrorLine', 'edit.selectInvert']) {
      expect(list, id).toContain(id);
    }
    expect(entries.filter((e) => e.kind === 'submenu').map((e) => e.kind === 'submenu' && e.id)).toEqual(['ctx.order', 'ctx.transform', 'ctx.align', 'ctx.select']);
  });

  it('sobre el fondo ofrece pegar, seleccionar y la vista; desbloquear solo si hay bloqueados', () => {
    const plain = ids(buildContextMenu(stateWith([rectObj('a', 0, 0, 5, 5)], []), false, hooks));
    expect(plain).toEqual(['edit.paste', 'edit.pasteInPlace', 'edit.selectAll', 'edit.selectInvert', 'view.fitBed', 'view.fitContent']);
    const locked = ids(buildContextMenu(stateWith([rectObj('a', 0, 0, 5, 5, { locked: true })], []), false, hooks));
    expect(locked).toContain('arrange.unlockAll');
  });

  it('un texto con contorno ofrece editar y convertir; una imagen, vectorizar', () => {
    const text: DesignObject = {
      ...rectObj('t', 0, 0, 5, 5),
      kind: { type: 'text', text: 'x', font_family: 'A', bold: false, italic: false, size_mm: 5, letter_spacing_mm: 0, line_spacing: 1, align: 'left', on_path: null, outline: [[{ type: 'move_to', to: [0, 0] }, { type: 'line_to', to: [1, 1] }]] },
    };
    const entries = buildContextMenu(stateWith([text], ['t']), true, hooks);
    expect(entries.filter((e) => e.kind === 'action').map((e) => e.kind === 'action' && e.id)).toEqual(['ctx.editText', 'ctx.textToPaths']);
    const img: DesignObject = { ...rectObj('i', 0, 0, 5, 5), kind: { type: 'image', source_path: 'a.png', width_px: 1, height_px: 1, size_mm: [5, 5] } };
    expect(ids(buildContextMenu(stateWith([img], ['i']), true, hooks))).toContain('tools.traceImage');
  });

  it('omite comandos que no existen y no deja separadores sobrantes', () => {
    const entries = buildContextMenu(stateWith([rectObj('a', 0, 0, 5, 5)], ['a']), true, { ...hooks, has: (id) => id === 'edit.copy' });
    expect(ids(entries)).toEqual(['edit.copy']);
    expect(entries[0]!.kind).toBe('command');
    expect(entries.at(-1)!.kind).not.toBe('separator');
    const check = (list: MenuEntry[]) => list.forEach((e, i) => {
      if (e.kind === 'separator') {
        expect(i).toBeGreaterThan(0);
        expect(list[i - 1]!.kind).not.toBe('separator');
        expect(i).toBeLessThan(list.length - 1);
      }
      if (e.kind === 'submenu') check(e.items);
    });
    check(buildContextMenu(stateWith([rectObj('a', 0, 0, 5, 5)], ['a']), true, hooks));
  });

  it('todos los comandos que usa existen en el registro', () => {
    const s = stateWith([rectObj('a', 0, 0, 5, 5)], ['a']);
    const all = { ...hooks, has: () => true };
    for (const onObject of [true, false]) {
      for (const id of ids(buildContextMenu(s, onObject, all))) expect(commands.has(id), id).toBe(true);
    }
  });
});

describe('menú contextual: estado y dibujo', () => {
  beforeEach(() => closeContextMenu());

  it('abre y cierra', () => {
    openContextMenu({ x: 10, y: 20, at: [1, 2], onObject: false });
    expect(useContextMenu.getState().menu).toEqual({ x: 10, y: 20, at: [1, 2], onObject: false });
    closeContextMenu();
    expect(useContextMenu.getState().menu).toBeNull();
  });

  it('cerrado no pinta nada', () => {
    expect(renderToString(<ContextMenuHost />)).toBe('');
  });

  it('su host está en el lienzo (el módulo exporta el componente y el estado)', async () => {
    const mod = await import('./index');
    expect(typeof mod.ContextMenuHost).toBe('function');
    expect(useStore.getState().doc).toBeDefined();
  });
});
