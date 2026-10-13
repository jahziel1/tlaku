import { useVariablesUi } from '@/features/variables/context';
import type { CommandDef } from '../types';

export const variablesCommands: CommandDef[] = [
  {
    // Abre el diálogo "Texto variable" (serie, CSV, avance y copias por fila).
    id: 'tools.variableText',
    label: 'variables.cmd.open',
    category: 'tools',
    run: () => useVariablesUi.getState().show(),
  },
];
