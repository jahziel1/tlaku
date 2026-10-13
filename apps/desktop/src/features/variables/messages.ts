import { t } from '@/i18n';
import type { Issue } from './resolve';

/** Texto para el usuario de un problema al resolver una variable. */
export function issueMessage(issue: Issue): string {
  const p = { raw: issue.raw, detail: issue.detail };
  switch (issue.code) {
    case 'unknown':
      return t('variables.issue.unknown', p);
    case 'unclosed':
      return t('variables.issue.unclosed', p);
    case 'empty':
      return t('variables.issue.empty', p);
    case 'noCsv':
      return t('variables.issue.noCsv', p);
    case 'noColumn':
      return t('variables.issue.noColumn', p);
    case 'noColumnArg':
      return t('variables.issue.noColumnArg', p);
    case 'rowEnd':
      return t('variables.issue.rowEnd', p);
    case 'badFormat':
      return t('variables.issue.badFormat', p);
  }
}
