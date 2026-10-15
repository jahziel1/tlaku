import { openHistory, openSaveVersion } from '@/features/versions';
import type { CommandDef } from '../types';

/**
 * Historial de versiones del proyecto (`features/versions`). «Historial de versiones…» se abre siempre (sin
 * archivo ofrece los historiales de otros proyectos); «Guardar versión…» pide que el documento ya tenga archivo.
 */
export const versionsCommands: CommandDef[] = [
  { id: 'versions.open', label: 'versions.cmd.open', category: 'file', run: () => openHistory() },
  {
    id: 'versions.saveNamed',
    label: 'versions.cmd.save',
    category: 'file',
    enabled: (s) => (s.filePath ? true : 'versions.reason.noFile'),
    run: () => openSaveVersion(),
  },
];
