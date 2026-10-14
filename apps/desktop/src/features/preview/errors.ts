import type { I18nKey } from '@/i18n';
import { ENGINE_ERROR } from '@/lib/errors';

/**
 * Pista para el usuario según el código del error del motor (`errorCode` en `lib/errors.ts`), que no
 * cambia con el idioma. El motor rechaza el trabajo, por ejemplo, cuando "se sale de la cama" con el
 * overscan (`bed_overflow`) o cuando no hay nada que producir (`nothing_to_send`).
 */
export function previewErrorHint(code: string | null): I18nKey | null {
  if (code === ENGINE_ERROR.bedOverflow) return 'preview.error.hintBed';
  if (code === ENGINE_ERROR.nothingToSend) return 'preview.error.hintEmpty';
  return null;
}

/** Texto del error sin prefijos técnicos como "Error: ". Vacío si no hay nada útil. */
export function cleanErrorMessage(message: string | null): string {
  return (message ?? '').replace(/^\s*(?:Error|Uncaught Error)\s*:\s*/i, '').trim();
}
