import { MENUS, type CommandCategory, type CommandRegistry, type MenuDef } from '@/commands';
import type { I18nKey } from '@/i18n';

/** Elemento de menú: un comando del registro, un separador o la lista de archivos recientes. */
export type ShellMenuItem = { kind: 'command'; id: string } | { kind: 'separator' } | { kind: 'recent' };

export interface ShellMenu {
  id: string;
  label: I18nKey;
  items: ShellMenuItem[];
}

/** Menú donde aterriza un comando que ningún menú menciona, según su categoría. */
const CATEGORY_MENU: Record<CommandCategory, string> = {
  file: 'file',
  edit: 'edit',
  tools: 'tools',
  view: 'view',
  machine: 'machine',
  panels: 'view',
  arrange: 'arrange',
  camera: 'machine',
};

/** Comandos que el shell agrega a los menús base justo después de otro comando. */
const INSERT_AFTER: Record<string, string[]> = {
  'file.open': ['@recent'],
  'edit.paste': ['edit.pasteInPlace'],
  'edit.selectAll': ['edit.selectInvert'],
  'panel.console': ['panel.history', 'panel.notes'],
  'file.new': ['file.newFromTemplate', 'file.newWindow'],
  'file.saveAs': ['file.saveTemplate'],
  'view.priorityBadges': ['view.guides', 'view.guidesDialog', 'view.startMarks'],
};

const HELP_MENU: ShellMenu = {
  id: 'help',
  label: 'history.menu.help',
  items: [
    { kind: 'command', id: 'help.manual' },
    { kind: 'command', id: 'help.shortcuts' },
    { kind: 'command', id: 'help.news' },
    { kind: 'separator' },
    { kind: 'command', id: 'help.report' },
    { kind: 'command', id: 'help.donate' },
    { kind: 'separator' },
    { kind: 'command', id: 'help.about' },
  ],
};

function toItem(entry: string): ShellMenuItem {
  if (entry === '-') return { kind: 'separator' };
  if (entry === '@recent') return { kind: 'recent' };
  return { kind: 'command', id: entry };
}

/**
 * Menús de la ventana a partir de `MENUS` (src/commands/menus.ts) más lo que agrega el shell
 * (recientes, pegar en sitio, historial, Ayuda). Todo comando visible del registro aparece en algún
 * menú: si ninguno lo menciona, se agrega al final del menú de su categoría, así un comando nuevo
 * nunca queda sin menú ni sin su atajo a la vista.
 */
export function buildMenus(registry: CommandRegistry, base: readonly MenuDef[] = MENUS): ShellMenu[] {
  const menus: ShellMenu[] = base.map((m) => ({
    id: m.id,
    label: m.label,
    items: m.items.flatMap<ShellMenuItem>((entry) => {
      const extra = INSERT_AFTER[entry];
      return extra ? [toItem(entry), ...extra.map(toItem)] : [toItem(entry)];
    }),
  }));
  menus.push(HELP_MENU);

  const placed = new Set(menus.flatMap((m) => m.items.flatMap((i) => (i.kind === 'command' ? [i.id] : []))));
  for (const def of registry.list()) {
    if (def.hidden || placed.has(def.id)) continue;
    const target = menus.find((m) => m.id === CATEGORY_MENU[def.category]);
    if (!target) continue;
    target.items.push({ kind: 'command', id: def.id });
    placed.add(def.id);
  }
  // Un menú sin comandos (por ejemplo Organizar antes de que su área agregue alguno) no se muestra.
  return menus.filter((m) => m.items.some((i) => i.kind !== 'separator'));
}
