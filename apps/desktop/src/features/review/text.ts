import type { Document } from '@/api/types';
import type { I18nKey, PluralBase } from '@/i18n';
import { t, tn } from '@/i18n';
import { fmt } from '@/lib/platform';
import type { Finding } from './types';

/** Nombre de la capa de un hallazgo (el que le puso el usuario, o su color). */
export function layerLabel(doc: Pick<Document, 'layers'>, f: Finding): string {
  if (!f.layer) return '';
  const layer = doc.layers.find((l) => l.color.toLowerCase() === f.layer?.toLowerCase());
  return layer?.name || f.layer;
}

/** Valores que llenan los marcadores `{n}`, `{mm}`... de los textos del hallazgo. */
export function findingParams(f: Finding, layer: string): Record<string, string | number> {
  return {
    n: f.count,
    layer,
    gap: fmt(f.value, 2),
    // Quitar duplicados dice cuánto trazo repetido; los fragmentos dicen su umbral.
    mm: fmt(f.kind === 'tiny' ? (f.tolerance ?? f.value) : f.value, 2),
    tol: fmt(f.tolerance ?? 0, 2),
    nodes: Math.round(f.value),
    dpi: Math.round(f.value),
    want: Math.round(f.detail),
  };
}

export const findingTitle = (f: Finding, doc: Pick<Document, 'layers'>): string =>
  tn(`review.kind.${f.kind}.title` as PluralBase, f.count, findingParams(f, layerLabel(doc, f)));

/** Por qué importa, más (si aplica) el aviso de los textos sin contorno. */
export function findingWhy(f: Finding, doc: Pick<Document, 'layers'>): string {
  const why = t(`review.kind.${f.kind}.why` as I18nKey, findingParams(f, layerLabel(doc, f)));
  return f.kind === 'text' && f.detail > 0 ? `${why} ${tn('review.kind.text.empty', f.detail)}` : why;
}

/** El mismo hallazgo en palabras de todos los días (modo sencillo). */
export const findingSimple = (f: Finding, doc: Pick<Document, 'layers'>): string =>
  tn(`review.simple.${f.kind}` as PluralBase, f.count, findingParams(f, layerLabel(doc, f)));

/** Texto del botón de arreglo del modo sencillo: concuerda en número con el hallazgo (los de capa hablan de una sola). */
export const findingSimpleFix = (f: Finding): string =>
  f.kind === 'silent_layer' || f.kind === 'hidden_layer'
    ? t(`review.simple.fix.${f.kind}`)
    : tn(`review.simple.fix.${f.kind}` as PluralBase, f.count);
