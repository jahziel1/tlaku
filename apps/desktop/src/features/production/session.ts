import { useStore } from '@/store';
import type { StartSnapshot } from './model';

/**
 * Memoria de la SESIÓN de la app del modo producción (nada se guarda en disco: al cerrar la app se
 * empieza de cero y el primer inicio vuelve a mostrar la lista completa).
 *
 * Un trabajo confirmado con la lista completa deja una marca `awaiting`. Si termina bien
 * (`jobState === 'finished'`) pasa a `ok` y solo entonces vale como referencia para omitir la lista.
 * Cancelado o fallido, la marca se borra: el siguiente inicio vuelve a la lista completa. Un
 * trabajo iniciado con el paso compacto no cambia la referencia al terminar bien y la borra si falla.
 */

type Phase = 'awaiting' | 'ok';

let mark: { snapshot: StartSnapshot; phase: Phase } | null = null;
let unsubscribe: (() => void) | null = null;

/** Vigila el estado del trabajo (una sola vez). Cubre también las alarmas en pleno trabajo, que no traen evento propio. */
function watch(): void {
  if (unsubscribe) return;
  unsubscribe = useStore.subscribe((s, prev) => {
    if (s.jobState === prev.jobState || !mark) return;
    if (s.jobState === 'finished') {
      if (mark.phase === 'awaiting') mark = { ...mark, phase: 'ok' };
    } else if (s.jobState === 'cancelled' || s.jobState === 'error') {
      mark = null;
    }
  });
}

/** Se confirmó un trabajo con el diálogo de inicio COMPLETO: su huella queda esperando a que termine bien. */
export function noteFullConfirm(snapshot: StartSnapshot): void {
  watch();
  mark = { snapshot, phase: 'awaiting' };
}

/** Tras pedir el inicio: si el motor lo rechazó (preflight, error) el trabajo nunca empezó y no sirve de referencia. */
export function noteStartRejected(): void {
  if (mark?.phase === 'awaiting') mark = null;
}

/** La referencia de un trabajo anterior confirmado con la lista completa y terminado bien, o `null`. */
export function verifiedBaseline(): StartSnapshot | null {
  return mark?.phase === 'ok' ? mark.snapshot : null;
}

/** Olvida todo (pruebas y al apagar el modo). */
export function resetProductionSession(): void {
  mark = null;
}
