import { HISTORY_TAB } from '@/features/shell/dockTabs';
import { useShellUi } from '@/features/shell/uiStore';
import type { I18nKey } from '@/i18n';
import { groupDoc, hasGroup, ungroupDoc } from '@/lib/groups';
import { selectSelectedObjects } from '@/store';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';

const needSelection = (s: AppState): true | I18nKey => (s.selectedIds.length > 0 ? true : 'reason.noSelection');

/** Hay algo seleccionado que se pueda cortar (no bloqueado). */
const needMovable = (s: AppState): true | I18nKey => {
  const picked = selectSelectedObjects(s);
  if (picked.length === 0) return 'reason.noSelection';
  return picked.some((o) => !o.locked) ? true : 'reason.locked';
};

const needClipboard = (s: AppState): true | I18nKey =>
  s.clipboard && s.clipboard.objects.length > 0 ? true : 'history.reason.clipboardEmpty';

/** Ids de todo lo que se puede seleccionar con el área (visible y sin bloquear). */
const selectable = (s: AppState): string[] => s.doc.objects.filter((o) => !o.hidden && !o.locked).map((o) => o.id);

/** Pasos de las flechas en mm: normal, grande (Mayús) y fino (Alt). */
export const NUDGE_STEPS = { normal: 1, big: 10, fine: 0.1 } as const;

const DIRS = {
  left: [-1, 0],
  right: [1, 0],
  up: [0, 1], // Y hacia arriba
  down: [0, -1],
} as const;

const SIZES = [
  { id: 'normal', prefix: '', suffix: '' },
  { id: 'big', prefix: 'Shift+', suffix: '.big' },
  { id: 'fine', prefix: 'Alt+', suffix: '.fine' },
] as const;

const nudges: CommandDef[] = Object.entries(DIRS).flatMap(([dir, [ux, uy]]) =>
  SIZES.map<CommandDef>((size) => ({
    id: `edit.nudge.${dir}${size.suffix}`,
    label: `cmd.edit.nudge.${dir}` as I18nKey,
    category: 'edit',
    hidden: true,
    repeatable: true,
    shortcuts: [`${size.prefix}Arrow${dir[0]!.toUpperCase()}${dir.slice(1)}`],
    enabled: (s) => {
      const picked = selectSelectedObjects(s);
      if (picked.length === 0) return 'reason.noSelection';
      return picked.some((o) => !o.locked) ? true : 'reason.locked';
    },
    run: (s) => {
      const step = NUDGE_STEPS[size.id];
      // `key` funde la ráfaga (dirección, tamaño y selección iguales) en un solo paso del historial.
      s.moveObjects(s.selectedIds, ux * step, uy * step, `nudge:${dir}:${size.id}:${s.selectedIds.join(',')}`);
    },
  })),
);

export const editCommands: CommandDef[] = [
  {
    id: 'edit.undo',
    label: 'cmd.edit.undo',
    category: 'edit',
    shortcuts: ['Mod+Z'],
    enabled: (s) => (s.canUndo() ? true : 'reason.nothingToUndo'),
    run: (s) => s.undo(),
  },
  {
    id: 'edit.redo',
    label: 'cmd.edit.redo',
    category: 'edit',
    shortcuts: ['Mod+Shift+Z', 'Mod+Y'],
    enabled: (s) => (s.canRedo() ? true : 'reason.nothingToRedo'),
    run: (s) => s.redo(),
  },
  {
    id: 'edit.selectAll',
    label: 'cmd.edit.selectAll',
    category: 'edit',
    shortcuts: ['Mod+A'],
    enabled: (s) => (selectable(s).length > 0 ? true : 'history.reason.emptyDocument'),
    run: (s) => s.selectAll(),
  },
  {
    id: 'edit.selectInvert',
    label: 'history.cmd.selectInvert',
    category: 'edit',
    shortcuts: ['Mod+Shift+I'],
    enabled: (s) => (selectable(s).length > 0 ? true : 'history.reason.emptyDocument'),
    run: (s) => {
      const picked = new Set(s.selectedIds);
      s.select(selectable(s).filter((id) => !picked.has(id)));
    },
  },
  {
    id: 'edit.duplicate',
    label: 'cmd.edit.duplicate',
    category: 'edit',
    shortcuts: ['Mod+D'],
    enabled: needSelection,
    run: (s) => s.select(s.duplicateObjects(s.selectedIds)),
  },
  {
    id: 'edit.delete',
    label: 'cmd.edit.delete',
    category: 'edit',
    shortcuts: ['Delete', 'Backspace'],
    enabled: needSelection,
    run: (s) => s.deleteObjects(s.selectedIds),
  },
  { id: 'edit.copy', label: 'cmd.edit.copy', category: 'edit', shortcuts: ['Mod+C'], enabled: needSelection, run: (s) => void s.copyObjects(s.selectedIds) },
  { id: 'edit.cut', label: 'cmd.edit.cut', category: 'edit', shortcuts: ['Mod+X'], enabled: needMovable, run: (s) => void s.cutObjects(s.selectedIds) },
  {
    id: 'edit.paste',
    label: 'cmd.edit.paste',
    category: 'edit',
    shortcuts: ['Mod+V'],
    enabled: needClipboard,
    run: (s) => s.select(s.pasteObjects()),
  },
  {
    id: 'edit.pasteInPlace',
    label: 'history.cmd.pasteInPlace',
    category: 'edit',
    shortcuts: ['Mod+Shift+V'],
    enabled: needClipboard,
    run: (s) => s.select(s.pasteObjects({ inPlace: true })),
  },
  {
    id: 'edit.group',
    label: 'cmd.edit.group',
    category: 'edit',
    shortcuts: ['Mod+G'],
    enabled: (s) => (s.doc.objects.filter((o) => s.selectedIds.includes(o.id) && !o.locked).length >= 2 ? true : 'history.reason.groupNeedsTwo'),
    run: (s) => s.applyDocChange('history.group', (doc) => groupDoc(doc, s.selectedIds)),
  },
  {
    id: 'edit.ungroup',
    label: 'cmd.edit.ungroup',
    category: 'edit',
    shortcuts: ['Mod+Shift+G'],
    enabled: (s) => (hasGroup(s.doc.objects, s.selectedIds) ? true : 'history.reason.noGroup'),
    run: (s) => s.applyDocChange('history.ungroup', (doc) => ungroupDoc(doc, s.selectedIds)),
  },
  // Paneles y ayuda. `?` (en cualquier distribución de teclado) lo atiende el shell; aquí quedan
  // los atajos que el registro puede resolver por tecla física.
  {
    id: 'panel.history',
    label: 'history.cmd.panelHistory',
    category: 'panels',
    shortcuts: ['Mod+4'],
    // El historial es una pestaña más del panel acoplado.
    checked: (s) => s.dockTab === HISTORY_TAB,
    run: (s) => s.setDockTab(HISTORY_TAB),
  },
  {
    id: 'help.shortcuts',
    label: 'history.cmd.shortcuts',
    category: 'panels',
    shortcuts: ['Mod+/', 'F1', 'Shift+/'],
    run: () => useShellUi.getState().openShortcuts(),
  },
  ...nudges,
];
