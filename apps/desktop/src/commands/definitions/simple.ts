import { toggleUiMode } from '@/features/simple/mode';
import type { CommandDef } from '../types';

/** Modo sencillo: el flujo guiado de 5 pasos. Cambiar de modo no toca el documento. */
export const simpleCommands: CommandDef[] = [
  {
    id: 'simple.toggle',
    label: 'simple.cmd.toSimple',
    labelFor: (s) => (s.prefs.uiMode === 'simple' ? 'simple.cmd.toFull' : 'simple.cmd.toSimple'),
    category: 'view',
    checked: (s) => s.prefs.uiMode === 'simple',
    run: () => toggleUiMode(),
  },
];
