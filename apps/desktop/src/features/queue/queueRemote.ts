import type { RemoteQueue, RemoteQueueItem } from '@/api/api';
import { currentCopy, currentItem, remainingEstimate, totals, type QueueItem, type QueueState } from './queueModel';

/**
 * Lo que la cola le cuenta al control remoto (celular, solo lectura) y la regla que lo limita. Puro: no
 * importa el store de la app. La cola se ve en el celular; confirmar el siguiente trabajo desde ahí vale
 * solo con las mismas condiciones que «iniciar» (checklist en el teléfono y encuadre en la PC, ver
 * `features/remote/remote.ts`), y además el documento a encuadrar tiene que ser el que la cola espera.
 */

const itemInfo = (i: QueueItem): RemoteQueueItem => ({ name: i.name, copies: i.copies, done: i.done, status: i.status });

/** Resumen para el celular; `null` si no hay cola. */
export function buildQueueInfo(q: QueueState, estimate: (i: QueueItem) => number | null): RemoteQueue | null {
  if (q.items.length === 0) return null;
  const cur = currentItem(q);
  const { done, total } = totals(q);
  const eta = remainingEstimate(q, estimate);
  return {
    state: q.run,
    done,
    total,
    // Con copias sin estimado, mejor no dar un tiempo engañosamente corto.
    eta_s: eta.unknown === 0 && eta.seconds > 0 ? eta.seconds : null,
    current: cur ? itemInfo(cur) : null,
    copy: currentCopy(q),
    message: q.run === 'stopped' ? q.stopMessage : '',
    items: q.items.slice(0, 12).map(itemInfo),
  };
}

/**
 * ¿La cola impide iniciar desde el celular el documento de la pestaña activa? Sí cuando espera confirmar un
 * trabajo y la pestaña activa no es la de ese trabajo: lo que el celular vería (y lo que se enviaría) sería
 * otro documento. En los demás estados la cola no interviene.
 */
export function queueBlocksRemoteStart(q: QueueState, activeTabId: string): boolean {
  if (q.run !== 'awaiting') return false;
  const cur = currentItem(q);
  return !cur || cur.tabId !== activeTabId;
}
