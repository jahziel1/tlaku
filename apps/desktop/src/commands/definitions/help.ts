import { useHelpUi } from '@/features/help/helpStore';
import { openUpdateCheck } from '@/features/help/updater';
import type { CommandDef } from '../types';

/**
 * Comandos del menú Ayuda. El de atajos (`help.shortcuts`) vive junto a los de edición; estos abren
 * los diálogos de `features/help`. No llevan atajo: `F1` ya es el de atajos de teclado.
 */
export const helpCommands: CommandDef[] = [
  { id: 'help.manual', label: 'help.cmd.manual', category: 'panels', run: () => useHelpUi.getState().openManual() },
  { id: 'help.news', label: 'help.cmd.news', category: 'panels', run: () => useHelpUi.getState().open('news') },
  { id: 'help.checkUpdates', label: 'help.cmd.checkUpdates', category: 'panels', run: openUpdateCheck },
  { id: 'help.report', label: 'help.cmd.report', category: 'panels', run: () => useHelpUi.getState().open('report') },
  { id: 'help.donate', label: 'help.cmd.donate', category: 'panels', run: () => useHelpUi.getState().open('donate') },
  { id: 'help.jobLog', label: 'help.cmd.jobLog', category: 'panels', run: () => useHelpUi.getState().open('jobLog') },
  { id: 'help.about', label: 'help.cmd.about', category: 'panels', run: () => useHelpUi.getState().open('about') },
];
