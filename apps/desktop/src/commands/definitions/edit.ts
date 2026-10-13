import type { I18nKey } from '@/i18n';
import { selectSelectedObjects } from '@/store';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';

const needSelection = (s: AppState): true | I18nKey => (s.selectedIds.length > 0 ? true : 'reason.noSelection');
const soon = (): I18nKey => 'reason.soon';

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
      // `key` funde pulsaciones seguidas en un solo paso del historial.
      s.moveObjects(s.selectedIds, ux * step, uy * step, `nudge:${dir}:${size.id}`);
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
  { id: 'edit.selectAll', label: 'cmd.edit.selectAll', category: 'edit', shortcuts: ['Mod+A'], run: (s) => s.selectAll() },
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
  // Pendientes: el registro ya los reserva para que menús y atajos no se reasignen después.
  { id: 'edit.copy', label: 'cmd.edit.copy', category: 'edit', shortcuts: ['Mod+C'], enabled: soon, run: () => {} },
  { id: 'edit.cut', label: 'cmd.edit.cut', category: 'edit', shortcuts: ['Mod+X'], enabled: soon, run: () => {} },
  { id: 'edit.paste', label: 'cmd.edit.paste', category: 'edit', shortcuts: ['Mod+V'], enabled: soon, run: () => {} },
  { id: 'edit.group', label: 'cmd.edit.group', category: 'edit', shortcuts: ['Mod+G'], enabled: soon, run: () => {} },
  { id: 'edit.ungroup', label: 'cmd.edit.ungroup', category: 'edit', shortcuts: ['Mod+Shift+G'], enabled: soon, run: () => {} },
  ...nudges,
];
