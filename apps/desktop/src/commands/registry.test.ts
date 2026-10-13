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
      objects: [{ id: 'a', layer: '#ff0000', transform: [1, 0, 0, 1, 10, 10], kind: { type: 'vector', paths: [] }, locked: false, hidden: false, group: null }],
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

describe('atajos del usuario encima de los de fábrica', () => {
  const base = { label: 'cmd.file.new', category: 'file', run: () => {} } as const;
  const defs: CommandDef[] = [
    { ...base, id: 'a', shortcuts: ['Mod+K'] },
    { ...base, id: 'b', shortcuts: ['Mod+J', 'Mod+Shift+J'] },
    { ...base, id: 'c' },
  ];

  it('sin cambios todo sigue como de fábrica', () => {
    const { registry } = makeRegistry(defs);
    expect(registry.match('Mod+K')?.id).toBe('a');
    expect(registry.shortcutsOf('b')).toEqual(['Mod+J', 'Mod+Shift+J']);
    expect(registry.shortcutsOf('c')).toEqual([]);
    expect(registry.isCustomized('a')).toBe(false);
  });

  it('un comando con cambios usa solo los suyos', () => {
    const { registry, store } = makeRegistry(defs);
    store.getState().setShortcutOverrides({ b: ['Mod+L'] });
    expect(registry.match('Mod+L')?.id).toBe('b');
    expect(registry.match('Mod+J')).toBeUndefined();
    expect(registry.match('Mod+Shift+J')).toBeUndefined();
    expect(registry.shortcutsOf('b')).toEqual(['Mod+L']);
    expect(registry.isCustomized('b')).toBe(true);
    // Los de fábrica del propio comando siguen disponibles.
    expect(registry.factoryShortcuts('b')).toEqual(['Mod+J', 'Mod+Shift+J']);
    expect(registry.get('b').shortcuts).toEqual(['Mod+J', 'Mod+Shift+J']);
  });

  it('una lista vacía deja el comando sin atajo y libera el de fábrica', () => {
    const { registry, store } = makeRegistry(defs);
    store.getState().setShortcutOverrides({ a: [] });
    expect(registry.match('Mod+K')).toBeUndefined();
    expect(registry.shortcutLabel('a', false)).toBe('');
    expect(registry.isCustomized('a')).toBe(true);
  });

  it('un atajo del usuario gana ante uno de fábrica de otro comando', () => {
    const { registry, store } = makeRegistry(defs);
    store.getState().setShortcutOverrides({ c: ['Mod+K'] });
    expect(registry.match('Mod+K')?.id).toBe('c');
    expect(registry.shortcutsOf('a')).toEqual([]);
    expect(registry.conflictOf('Mod+K', 'a')?.id).toBe('c');
    expect(registry.conflictOf('Mod+K', 'c')).toBeUndefined();
  });

  it('acepta el atajo en cualquier orden de modificadores e ignora los inválidos', () => {
    const { registry, store } = makeRegistry(defs);
    store.getState().setShortcutOverrides({ c: ['Shift+Mod+U', 'cosa rara'] });
    expect(registry.match('Mod+Shift+U')?.id).toBe('c');
    expect(registry.shortcutsOf('c')).toEqual(['Mod+Shift+U']);
  });

  it('ids desconocidos en los cambios no rompen nada', () => {
    const { registry, store } = makeRegistry(defs);
    store.getState().setShortcutOverrides({ 'no.existe': ['Mod+Q'] });
    expect(registry.match('Mod+Q')).toBeUndefined();
    expect(registry.match('Mod+K')?.id).toBe('a');
  });

  it('el texto del atajo principal usa el del usuario', () => {
    const { registry, store } = makeRegistry(defs);
    store.getState().setShortcutOverrides({ a: ['Mod+Shift+P'] });
    expect(registry.shortcutLabel('a', false)).toBe('Ctrl+Mayús+P');
    expect(registry.shortcutLabel('a', true)).toBe('⇧⌘P');
  });

  it('ejecutar el atajo nuevo corre el comando (no el de fábrica)', async () => {
    const calls: string[] = [];
    const tracked: CommandDef[] = [
      { ...base, id: 'a', shortcuts: ['Mod+K'], run: () => void calls.push('a') },
      { ...base, id: 'c', run: () => void calls.push('c') },
    ];
    const { registry, store } = makeRegistry(tracked);
    store.getState().setShortcutOverrides({ c: ['Mod+K'] });
    await registry.run(registry.match('Mod+K')!.id);
    expect(calls).toEqual(['c']);
  });

  it('los atajos de fábrica del registro real siguen sin cambios por los del usuario', () => {
    const { registry, store } = makeRegistry();
    store.getState().setShortcutOverrides({ 'edit.undo': ['Mod+U'] });
    expect(registry.match('Mod+U')?.id).toBe('edit.undo');
    expect(registry.match('Mod+Z')).toBeUndefined();
    expect(registry.get('edit.undo').shortcuts).toEqual(['Mod+Z']);
  });

  it('las preferencias se abren con Mod+,', () => {
    const { registry } = makeRegistry();
    expect(registry.match('Mod+,')?.id).toBe('prefs.open');
  });
});
