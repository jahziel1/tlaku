import { selectionUnits } from '@/features/arrange/units';
import { useNestUi } from '@/features/nest/nestStore';
import { setRemnantsPanel } from '@/features/nest/remnantStore';
import type { I18nKey } from '@/i18n';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';

/** Hace falta al menos una unidad móvil (objeto suelto o grupo completo) seleccionada. */
export function nestEnabled(s: AppState): true | I18nKey {
  if (s.selectedIds.length === 0) return 'nest.reason.noSelection';
  return selectionUnits(s.doc, s.selectedIds).length > 0 ? true : 'nest.reason.nothingMovable';
}

export const nestCommands: CommandDef[] = [
  {
    id: 'arrange.nest',
    label: 'nest.cmd.open',
    category: 'arrange',
    enabled: nestEnabled,
    run: (s) => {
      if (nestEnabled(s) !== true) return;
      useNestUi.getState().show();
    },
  },
  {
    id: 'arrange.remnants',
    label: 'nest.cmd.remnants',
    category: 'arrange',
    run: () => setRemnantsPanel(true),
  },
];
