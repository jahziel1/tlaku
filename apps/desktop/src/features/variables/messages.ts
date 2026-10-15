import { t } from '@/i18n';
import type { Issue } from './resolve';

/** Texto para el usuario de un problema al resolver una variable. */
export function issueMessage(issue: Issue): string {
  const p = { raw: issue.raw, detail: issue.detail, row: issue.row ?? '' };
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
    case 'noSource':
      return t('variables.issue.noSource', p);
    case 'emptySource':
      return t('variables.issue.emptySource', p);
    case 'sourceRowEnd':
      return t('variables.issue.sourceRowEnd', p);
    case 'emptyCell':
      return t('variables.issue.emptyCell', p);
    case 'imageEmpty':
      return t('variables.issue.imageEmpty', p);
    case 'imageMissing':
      return t('variables.issue.imageMissing', p);
    case 'codeEmpty':
      return t('variables.issue.codeEmpty', p);
    case 'codeFailed':
      return t('variables.issue.codeFailed', p);
  }
}
