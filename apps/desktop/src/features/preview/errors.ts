import type { I18nKey } from '@/i18n';

/**
 * Pista para el usuario según el texto del error del motor (ya viene en español).
 * El motor rechaza el trabajo, por ejemplo, cuando "se sale de la cama" con el overscan.
 */
export function previewErrorHint(message: string): I18nKey | null {
  if (/cama|fuera de|se sale|no cabe|límites|limites|bed/i.test(message)) return 'preview.error.hintBed';
  if (/nada que enviar|vac[ií]o|sin objetos/i.test(message)) return 'preview.error.hintEmpty';
  return null;
}

/** Texto del error sin prefijos técnicos como "Error: ". Vacío si no hay nada útil. */
export function cleanErrorMessage(message: string | null): string {
  return (message ?? '').replace(/^\s*(?:Error|Uncaught Error)\s*:\s*/i, '').trim();
}
