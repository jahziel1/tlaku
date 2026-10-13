import { describe, expect, it } from 'vitest';
import { es } from '@/i18n';
import { createAppStore } from '@/store';
import { ALL_COMMANDS } from './index';
import { MENUS } from './menus';
import { CommandRegistry, ShortcutConflictError } from './registry';
import { eventToCombo, formatCombo, isEditableTarget, normalizeCombo, parseCombo, type KeyEventLike } from './shortcuts';
import type { CommandDef } from './types';

const ev = (over: Partial<KeyEventLike>): KeyEventLike => ({
  key: '', code: '', ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...over,
});

function makeRegistry(defs: readonly CommandDef[] = ALL_COMMANDS) {
  const store = createAppStore();
  return { store, registry: new CommandRegistry(defs, () => store.getState()) };
}

describe('registro de comandos por defecto', () => {
  it('se construye sin ids ni atajos repetidos', () => {
    expect(() => makeRegistry()).not.toThrow();
  });

  it('todas las etiquetas existen en el diccionario', () => {
    for (const c of ALL_COMMANDS) {
      expect(es, c.id).toHaveProperty(c.label);
      expect(es, `${c.id} (labelFor)`).toHaveProperty(c.labelFor ? c.labelFor(makeRegistry().store.getState()) : c.label);
    }
  });

  it('todos los atajos son válidos y están en forma canónica', () => {
    for (const c of ALL_COMMANDS) {
      for (const s of c.shortcuts ?? []) expect(normalizeCombo(s), `${c.id}: ${s}`).toBe(s);
    }
  });

  it('los menús solo mencionan comandos que existen', () => {
    const { registry } = makeRegistry();
    for (const menu of MENUS) {
      for (const item of menu.items) if (item !== '-') expect(registry.has(item), `${menu.id}: ${item}`).toBe(true);
      expect(es).toHaveProperty(menu.label);
    }
  });

  it('las teclas de herramienta no usan modificadores y las acciones peligrosas sí', () => {
    const { registry } = makeRegistry();
    expect(registry.match('V')?.id).toBe('tool.select');
    expect(registry.match('Escape')?.id).toBe('tool.select');
    expect(registry.match('Mod+Shift+Z')?.id).toBe('edit.redo');
    expect(registry.match('Mod+Y')?.id).toBe('edit.redo');
    expect(registry.match('Mod+Enter')?.id).toBe('machine.start');
  });
});

describe('detección de conflictos', () => {
  const base = { label: 'cmd.file.new', category: 'file', run: () => {} } as const;
  it('rechaza el mismo atajo en dos comandos', () => {
    const defs: CommandDef[] = [
      { ...base, id: 'a', shortcuts: ['Mod+K'] },
      { ...base, id: 'b', shortcuts: ['Mod+K'] },
    ];
    expect(() => makeRegistry(defs)).toThrow(ShortcutConflictError);
  });
  it('trata Shift+Mod+K y Mod+Shift+K como el mismo atajo', () => {
    expect(normalizeCombo('Shift+Mod+K')).toBe('Mod+Shift+K');
  });
  it('rechaza ids repetidos', () => {
    expect(() => makeRegistry([{ ...base, id: 'a' }, { ...base, id: 'a' }])).toThrow(/repetido/);
  });
});

describe('ejecución', () => {
  it('no ejecuta un comando deshabilitado y explica por qué', async () => {
    const { registry, store } = makeRegistry();
    expect(registry.availability('edit.delete')).toBe('reason.noSelection');
    expect(await registry.run('edit.delete')).toBe(false);
    expect(registry.availability('machine.start', store.getState())).toBe('reason.needMachine');
  });

  it('las flechas mueven la selección y se funden en un solo paso de historial', async () => {
    const { registry, store } = makeRegistry();
    store.getState().applyImport({
      objects: [{ id: 'a', layer: '#ff0000', transform: [1, 0, 0, 1, 10, 10], kind: { type: 'vector', paths: [] }, locked: false, hidden: false }],
      new_layers: [],
      warnings: [],
    });
    store.getState().select(['a']);
    const before = store.getState().past.length;
    await registry.run('edit.nudge.right');
    await registry.run('edit.nudge.right');
    await registry.run('edit.nudge.up.big');
    const obj = store.getState().doc.objects[0]!;
    expect(obj.transform[4]).toBe(12);
    expect(obj.transform[5]).toBe(20); // 10 mm hacia arriba (Y crece hacia arriba)
    expect(store.getState().past.length - before).toBe(2); // dos a la derecha = 1 paso; arriba = otro
    await registry.run('edit.undo');
    await registry.run('edit.undo');
    expect(store.getState().doc.objects[0]!.transform[4]).toBe(10);
  });

  it('Escape quita la selección y vuelve a Seleccionar', async () => {
    const { registry, store } = makeRegistry();
    store.getState().setTool('rect');
    await registry.run('tool.select');
    expect(store.getState().activeTool).toBe('select');
  });
});

describe('atajos de teclado', () => {
  it('parseCombo valida modificadores y teclas', () => {
    expect(parseCombo('Mod+Shift+Z')).toEqual({ mod: true, alt: false, shift: true, key: 'Z' });
    expect(() => parseCombo('Cmd+Z')).toThrow();
    expect(() => parseCombo('Mod+ñ')).toThrow();
  });

  it('eventToCombo usa Cmd en macOS y Ctrl en el resto', () => {
    const e = ev({ key: 'z', code: 'KeyZ', metaKey: true, shiftKey: true });
    expect(eventToCombo(e, true)).toBe('Mod+Shift+Z');
    expect(eventToCombo(e, false)).toBeNull(); // Meta en Windows no es Ctrl
    expect(eventToCombo(ev({ key: 'z', code: 'KeyZ', ctrlKey: true }), false)).toBe('Mod+Z');
    expect(eventToCombo(ev({ key: 'z', code: 'KeyZ', ctrlKey: true }), true)).toBeNull(); // Ctrl en Mac no es Mod
  });

  it('con Alt en Mac la letra se toma de la tecla física', () => {
    expect(eventToCombo(ev({ key: 'π', code: 'KeyP', altKey: true }), true)).toBe('Alt+P');
  });

  it('signos y flechas', () => {
    expect(eventToCombo(ev({ key: '=', code: 'Equal', ctrlKey: true }), false)).toBe('Mod+=');
    expect(eventToCombo(ev({ key: '+', code: 'Equal', ctrlKey: true, shiftKey: true }), false)).toBe('Mod+Shift+=');
    expect(eventToCombo(ev({ key: 'ArrowLeft', code: 'ArrowLeft', shiftKey: true }), false)).toBe('Shift+ArrowLeft');
    expect(eventToCombo(ev({ key: 'Shift', code: 'ShiftLeft', shiftKey: true }), false)).toBeNull();
  });

  it('formatCombo muestra símbolos en Mac y nombres en el resto', () => {
    expect(formatCombo('Mod+Shift+S', true)).toBe('⇧⌘S');
    expect(formatCombo('Mod+Shift+S', false)).toBe('Ctrl+Mayús+S');
    expect(formatCombo('Delete', false)).toBe('Supr');
  });

  it('reconoce campos de texto', () => {
    expect(isEditableTarget({ tagName: 'INPUT', type: 'text' } as unknown as EventTarget)).toBe(true);
    expect(isEditableTarget({ tagName: 'INPUT', type: 'checkbox' } as unknown as EventTarget)).toBe(false);
    expect(isEditableTarget({ tagName: 'BUTTON' } as unknown as EventTarget)).toBe(false);
    expect(isEditableTarget(null)).toBe(false);
  });
});
