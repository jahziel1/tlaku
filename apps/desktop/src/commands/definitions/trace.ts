import type { I18nKey } from '@/i18n';
import { useTraceDialog } from '@/features/trace/traceStore';
import { selectSelectedObjects } from '@/store';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';

/** Solo se vectoriza con exactamente una imagen seleccionada; si no, dice qué falta. */
export function traceEnabled(s: AppState): true | I18nKey {
  const picked = selectSelectedObjects(s);
  if (picked.length === 0) return 'trace.reason.needImage';
  if (picked.length > 1) return 'trace.reason.oneImage';
  return picked[0]!.kind.type === 'image' ? true : 'trace.reason.needImage';
}

export const traceCommands: CommandDef[] = [
  {
    id: 'tools.traceImage',
    label: 'trace.cmd.image',
    category: 'tools',
    enabled: traceEnabled,
    run: (s) => {
      if (traceEnabled(s) !== true) return;
      useTraceDialog.getState().open(s.selectedIds[0]!);
    },
  },
];
