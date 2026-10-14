import { resetRegistration } from '@/features/printcut/actions';
import { usePrintCut } from '@/features/printcut/printcutStore';
import type { I18nKey } from '@/i18n';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';

/** Hay un registro de Print & Cut aplicado al trabajo. */
export function printCutActive(s: AppState): true | I18nKey {
  return s.jobOptions.transform ? true : 'printcut.reason.none';
}

export const printcutCommands: CommandDef[] = [
  {
    id: 'printcut.open',
    label: 'printcut.cmd.open',
    category: 'machine',
    run: () => usePrintCut.getState().show(),
  },
  {
    id: 'printcut.reset',
    label: 'printcut.cmd.reset',
    category: 'machine',
    enabled: printCutActive,
    run: (s) => {
      if (printCutActive(s) !== true) return;
      resetRegistration();
      s.notifyKey('info', 'printcut.resetDone');
    },
  },
];
