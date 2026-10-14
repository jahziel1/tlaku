import { describe, expect, it } from 'vitest';
import { ALL_COMMANDS, commands, CommandRegistry, formatCombo, MENUS, type CommandDef } from '@/commands';
import { createAppStore } from '@/store';
import { buildMenus } from './menuModel';
import { buildShortcutRows, filterShortcutRows, groupShortcutRows } from './shortcutsModel';
import { useShellUi } from './uiStore';

const registryOf = (defs: readonly CommandDef[]) => {
  const store = createAppStore();
  return new CommandRegistry(defs, () => store.getState());
};

describe('diálogo de atajos: filas generadas desde el registro', () => {
  const rows = buildShortcutRows(commands, false);

  it('incluye todo comando con atajos, con todos sus atajos formateados', () => {
    for (const def of ALL_COMMANDS.filter((d) => d.shortcuts?.length)) {
      const row = rows.find((r) => r.id === def.id);
      expect(row, def.id).toBeDefined();
      expect(row!.keys).toEqual(def.shortcuts!.map((s) => formatCombo(s, false)));
    }
  });

  it('no repite filas y no muestra comandos sin atajo', () => {
    expect(new Set(rows.map((r) => r.id)).size).toBe(rows.length);
    const ids = new Set(rows.map((r) => r.id));
    for (const def of ALL_COMMANDS.filter((d) => !d.shortcuts?.length)) expect(ids.has(def.id), def.id).toBe(false);
  });

  it('distingue los pasos de las flechas por su distancia', () => {
    const labels = rows.filter((r) => r.id.startsWith('edit.nudge.left')).map((r) => r.label);
    expect(new Set(labels).size).toBe(3);
    expect(labels.join(' ')).toMatch(/0,1 mm/);
  });

  it('muestra los atajos de Mac con símbolos', () => {
    const mac = buildShortcutRows(commands, true);
    expect(mac.find((r) => r.id === 'edit.redo')!.keys[0]).toBe('⇧⌘Z');
  });

  it('busca sin importar mayúsculas ni acentos, por nombre o por atajo', () => {
    expect(filterShortcutRows(rows, 'deshacer').map((r) => r.id)).toContain('edit.undo');
    expect(filterShortcutRows(rows, 'SELECCION').map((r) => r.id)).toContain('edit.selectAll');
    expect(filterShortcutRows(rows, 'ctrl+shift+s'.replace('shift', 'mayús')).map((r) => r.id)).toContain('file.saveAs');
    expect(filterShortcutRows(rows, 'zoom').length).toBeGreaterThan(1);
    expect(filterShortcutRows(rows, 'xyzzy-no-existe')).toEqual([]);
    expect(filterShortcutRows(rows, '   ')).toHaveLength(rows.length);
  });

  it('todas las palabras deben coincidir', () => {
    const ids = filterShortcutRows(rows, 'pegar lugar').map((r) => r.id);
    expect(ids).toEqual(['edit.pasteInPlace']);
  });

  it('agrupa por categoría en el orden de los menús', () => {
    const groups = groupShortcutRows(rows);
    const cats = groups.map((g) => g.category);
    expect(new Set(cats).size).toBe(cats.length); // cada categoría aparece una sola vez
    expect(cats[0]).toBe('file');
    expect(cats.at(-1)).toBe('canvas');
  });
});

describe('abrir el diálogo de atajos', () => {
  it('el comando existe y se abre con Mod+/, F1 y Mayús+/ sin chocar con otros', () => {
    expect(commands.has('help.shortcuts')).toBe(true);
    for (const combo of ['Mod+/', 'F1', 'Shift+/']) expect(commands.match(combo)?.id, combo).toBe('help.shortcuts');
    // Una prueba de conflictos real: construir el registro completo no lanza.
    expect(() => registryOf(ALL_COMMANDS)).not.toThrow();
  });

  it('ejecutar el comando abre el diálogo y repetirlo no lo cierra', async () => {
    useShellUi.getState().closeShortcuts();
    await commands.run('help.shortcuts');
    expect(useShellUi.getState().shortcutsOpen).toBe(true);
    await commands.run('help.shortcuts');
    expect(useShellUi.getState().shortcutsOpen).toBe(true);
    useShellUi.getState().closeShortcuts();
    expect(useShellUi.getState().shortcutsOpen).toBe(false);
  });
});

describe('menús coherentes con el registro', () => {
  const menus = buildMenus(commands);
  const placed = menus.flatMap((m) => m.items.flatMap((i) => (i.kind === 'command' ? [i.id] : [])));

  it('todo comando visible aparece en un menú, una sola vez', () => {
    for (const def of ALL_COMMANDS.filter((d) => !d.hidden)) expect(placed, def.id).toContain(def.id);
    expect(new Set(placed).size).toBe(placed.length);
  });

  it('solo menciona comandos que existen y cada comando con atajo lo muestra', () => {
    for (const id of placed) {
      expect(commands.has(id), id).toBe(true);
      if (commands.get(id).shortcuts?.length) expect(commands.shortcutLabel(id, false), id).not.toBe('');
    }
  });

  it('agrega recientes bajo Abrir, pegar en sitio, invertir selección, historial y el menú de Ayuda', () => {
    const file = menus.find((m) => m.id === 'file')!.items;
    const iOpen = file.findIndex((i) => i.kind === 'command' && i.id === 'file.open');
    expect(file[iOpen + 1]).toEqual({ kind: 'recent' });
    const edit = menus.find((m) => m.id === 'edit')!.items.flatMap((i) => (i.kind === 'command' ? [i.id] : []));
    expect(edit.indexOf('edit.pasteInPlace')).toBe(edit.indexOf('edit.paste') + 1);
    expect(edit.indexOf('edit.selectInvert')).toBe(edit.indexOf('edit.selectAll') + 1);
    expect(placed).toContain('panel.history');
    expect(menus.at(-1)!.id).toBe('help');
    const help = menus.at(-1)!.items.flatMap((i) => (i.kind === 'command' ? [i.id] : []));
    expect(help).toEqual(['help.manual', 'help.shortcuts', 'help.news', 'help.jobLog', 'help.checkUpdates', 'help.report', 'help.donate', 'help.about']);
  });

  it('un comando que ningún menú menciona va al menú de su categoría', () => {
    const extra: CommandDef = { id: 'edit.nuevo', label: 'cmd.edit.undo', category: 'edit', run: () => {} };
    const hidden: CommandDef = { id: 'edit.oculto', label: 'cmd.edit.undo', category: 'edit', hidden: true, run: () => {} };
    const panel: CommandDef = { id: 'panel.extra', label: 'cmd.edit.undo', category: 'panels', run: () => {} };
    const reg = registryOf([...ALL_COMMANDS, extra, hidden, panel]);
    const out = buildMenus(reg, MENUS);
    const idsOf = (menu: string) => out.find((m) => m.id === menu)!.items.flatMap((i) => (i.kind === 'command' ? [i.id] : []));
    expect(idsOf('edit')).toContain('edit.nuevo');
    expect(idsOf('view')).toContain('panel.extra');
    expect(out.flatMap((m) => m.items).some((i) => i.kind === 'command' && i.id === 'edit.oculto')).toBe(false);
  });
});
