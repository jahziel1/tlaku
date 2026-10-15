import { t, tn } from '@/i18n';
import type { DocDiff } from './diff';

const MAX_NAMES = 4;

/** Lista de nombres para una frase: los primeros y «y N más». */
function names(colors: string[], all: Record<string, string>): string {
  const shown = colors.slice(0, MAX_NAMES).map((c) => all[c] ?? c);
  const rest = colors.length - shown.length;
  return rest > 0 ? `${shown.join(', ')} +${rest}` : shown.join(', ');
}

/**
 * Resumen en frases de lo que pasaría al restaurar una versión (visto desde el documento actual):
 * lo agregado después se perdería, lo quitado volvería y lo modificado regresaría a su estado.
 */
export function describeDiff(diff: DocDiff): string[] {
  if (diff.same) return [t('versions.compare.same')];
  const out: string[] = [];
  if (diff.added.length > 0) out.push(tn('versions.compare.lost', diff.added.length));
  if (diff.removed.length > 0) out.push(tn('versions.compare.back', diff.removed.length));
  if (diff.changed.length > 0) out.push(tn('versions.compare.reverted', diff.changed.length));
  if (diff.layersAdded.length > 0) out.push(t('versions.compare.layersLost', { names: names(diff.layersAdded, diff.layerNames) }));
  if (diff.layersRemoved.length > 0) out.push(t('versions.compare.layersBack', { names: names(diff.layersRemoved, diff.layerNames) }));
  if (diff.layersChanged.length > 0) out.push(t('versions.compare.layersChanged', { names: names(diff.layersChanged, diff.layerNames) }));
  if (diff.notesChanged) out.push(t('versions.compare.notes'));
  if (diff.guidesChanged) out.push(t('versions.compare.guides'));
  if (diff.deviceChanged) out.push(t('versions.compare.device'));
  return out;
}
