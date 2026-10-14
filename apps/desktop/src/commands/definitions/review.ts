import { openReview } from '@/features/review';
import type { I18nKey } from '@/i18n';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';

/** Hace falta algo que revisar. */
export function reviewEnabled(s: AppState): true | I18nKey {
  return s.doc.objects.length > 0 ? true : 'review.reason.empty';
}

export const reviewCommands: CommandDef[] = [
  {
    id: 'review.open',
    label: 'review.cmd.open',
    category: 'machine',
    shortcuts: ['Mod+Alt+R'],
    enabled: reviewEnabled,
    run: (s) => {
      if (reviewEnabled(s) !== true) return;
      openReview();
    },
  },
];
