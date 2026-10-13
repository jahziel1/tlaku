import { useCodesDialog } from '@/features/codes/codesStore';
import { importShxFonts } from '@/features/text/shx';
import type { CommandDef } from '../types';

export const codesCommands: CommandDef[] = [
  {
    id: 'tools.insertCode',
    label: 'codes.cmd.insert',
    category: 'tools',
    run: () => useCodesDialog.getState().show(),
  },
  {
    id: 'tools.importShxFont',
    label: 'text.cmd.importShx',
    category: 'tools',
    run: () => void importShxFonts(),
  },
];
