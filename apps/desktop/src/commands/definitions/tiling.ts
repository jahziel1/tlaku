import { designBounds } from '@/features/machine/placement';
import { openTiling } from '@/features/tiling/store';
import type { I18nKey } from '@/i18n';
import { selectJobSelection, type AppState } from '@/store';
import type { CommandDef } from '../types';

/** Hace falta algo que enviar: sin salida no hay nada que dividir. */
export function tilingEnabled(s: AppState): true | I18nKey {
  return designBounds(s.doc, selectJobSelection(s)) ? true : 'tiling.reason.nothing';
}

export const tilingCommands: CommandDef[] = [
  {
    id: 'machine.tiling',
    label: 'tiling.cmd.open',
    category: 'machine',
    enabled: tilingEnabled,
    run: (s) => {
      if (tilingEnabled(s) !== true) return;
      openTiling();
    },
  },
];
