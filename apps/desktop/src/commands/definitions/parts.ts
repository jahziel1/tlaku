import { openPartsDialog, openRegenerateDialog, selectedRecord } from '@/features/parts';
import type { CommandDef } from '../types';

/**
 * «Generadores de piezas…» (menú Herramientas): cajas con dientes, bisagras flexibles y la pieza de prueba de
 * encaje (`features/parts`). «Regenerar piezas…» reabre el diálogo con los parámetros del grupo seleccionado.
 */
export const partsCommands: CommandDef[] = [
  { id: 'parts.open', label: 'parts.cmd.open', category: 'tools', run: () => openPartsDialog() },
  {
    id: 'parts.regenerate',
    label: 'parts.cmd.regenerate',
    category: 'tools',
    enabled: (s) => (selectedRecord() && s.selectedIds.length > 0 ? true : 'parts.reason.noGenerated'),
    run: async () => {
      const record = selectedRecord();
      if (record) await openRegenerateDialog(record);
    },
  },
];
