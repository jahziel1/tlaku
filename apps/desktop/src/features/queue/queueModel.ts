/**
 * Cola guiada de trabajos (mejora 4 del plan de usuarios): modelo puro, sin interfaz ni motor.
 *
 * REGLA DE SEGURIDAD (innegociable): la cola NUNCA avanza sola de un trabajo al siguiente. Al terminar
 * uno, la cola pasa a `awaiting` y ahí se queda hasta que una persona lo confirme (cambio de material o
 * pieza, lista de comprobación, encuadre opcional); solo entonces la interfaz abre el diálogo de inicio
 * normal (preflight y lista de comprobación incluidos). Este modelo no sabe iniciar nada: la única forma
 * de llegar a `running` es `jobStarted`, que la interfaz llama cuando la máquina EMPEZÓ de verdad un
 * trabajo de la pestaña que la cola esperaba. Si un trabajo falla o se cancela, la cola se detiene
 * (`stopped`); no se salta al siguiente.
 */

export type QueueSource = 'tab' | 'file';
export type QueueItemStatus = 'pending' | 'done' | 'skipped' | 'failed';

/** Cuántas copias de un mismo trabajo se pueden pedir. */
export const MAX_COPIES = 99;
/** Cuántos trabajos caben en la cola. */
export const MAX_ITEMS = 50;

export interface QueueItem {
  id: string;
  source: QueueSource;
  /** Pestaña que tiene el documento (se llena al abrir un archivo); `null` si aún no está abierto. */
  tabId: string | null;
  /** Ruta del proyecto; `null` si es una pestaña sin guardar. */
  path: string | null;
  name: string;
  copies: number;
  /** Copias terminadas. */
  done: number;
  status: QueueItemStatus;
  /** Aviso del usuario para el paso («cambiar a MDF 3 mm»), se muestra al pedir la confirmación. */
  note: string;
}

/**
 * - `idle`: sin empezar;
 * - `awaiting`: espera la confirmación del trabajo `currentId` (el primero también);
 * - `running`: un trabajo de la cola está en la máquina;
 * - `paused`: el usuario pausó la cola (no pide confirmaciones hasta reanudar);
 * - `stopped`: se detuvo (fallo, cancelación o a mano);
 * - `done`: no queda nada pendiente.
 */
export type QueueRun = 'idle' | 'awaiting' | 'running' | 'paused' | 'stopped' | 'done';
export type StopReason = 'failed' | 'cancelled' | 'user' | 'missing';

export interface QueueState {
  items: QueueItem[];
  run: QueueRun;
  /** Trabajo que corre o que espera confirmación. */
  currentId: string | null;
  /** Se pidió pausar con un trabajo en marcha: al terminar queda `paused` en vez de `awaiting`. */
  pauseAfter: boolean;
  stopReason: StopReason | null;
  /** Mensaje del motor o del motivo de la detención. */
  stopMessage: string;
}

export const EMPTY_QUEUE: QueueState = { items: [], run: 'idle', currentId: null, pauseAfter: false, stopReason: null, stopMessage: '' };

export type JobEnd = 'finished' | 'cancelled' | 'error';

const clampCopies = (n: number): number => (Number.isFinite(n) ? Math.min(MAX_COPIES, Math.max(1, Math.round(n))) : 1);

let seq = 0;
/** Elemento nuevo (copias entre 1 y `MAX_COPIES`). */
export function makeItem(init: Pick<QueueItem, 'source' | 'name'> & Partial<Omit<QueueItem, 'copies'>> & { copies?: number }): QueueItem {
  return {
    id: init.id ?? `q${Date.now().toString(36)}-${++seq}`,
    source: init.source,
    tabId: init.tabId ?? null,
    path: init.path ?? null,
    name: init.name,
    copies: clampCopies(init.copies ?? 1),
    done: Math.max(0, init.done ?? 0),
    status: init.status ?? 'pending',
    note: (init.note ?? '').slice(0, 200),
  };
}

export const itemById = (s: QueueState, id: string | null): QueueItem | null => (id === null ? null : (s.items.find((i) => i.id === id) ?? null));
export const currentItem = (s: QueueState): QueueItem | null => itemById(s, s.currentId);

/** Copias que faltan de un elemento (0 si se saltó o ya está). */
export const remainingCopies = (i: QueueItem): number => (i.status === 'skipped' || i.status === 'done' ? 0 : Math.max(0, i.copies - i.done));

/** Primer elemento con copias por hacer, en el orden de la lista. */
export const nextPending = (s: QueueState): QueueItem | null => s.items.find((i) => i.status === 'pending' && remainingCopies(i) > 0) ?? null;

/** ¿La cola tiene un trabajo en la máquina o esperando confirmación? (no se puede editar lo que está en curso). */
export const isActive = (s: QueueState): boolean => s.run === 'awaiting' || s.run === 'running';

/** Trabajos (copias) terminados y total de la cola, sin contar lo que se saltó. */
export function totals(s: QueueState): { done: number; total: number } {
  let done = 0;
  let total = 0;
  for (const i of s.items) {
    if (i.status === 'skipped') continue;
    total += i.copies;
    done += Math.min(i.done, i.copies);
  }
  return { done, total };
}

/** Copia que está en curso o por confirmar del elemento actual (desde 1). */
export const currentCopy = (s: QueueState): number => {
  const c = currentItem(s);
  return c ? Math.min(c.copies, c.done + 1) : 0;
};

/**
 * Segundos que faltan estimados. `estimate` da los de UNA copia de un elemento (o `null` si no se conocen);
 * `unknown` cuenta cuántas copias pendientes no tienen estimado.
 */
export function remainingEstimate(s: QueueState, estimate: (i: QueueItem) => number | null): { seconds: number; unknown: number } {
  let seconds = 0;
  let unknown = 0;
  for (const i of s.items) {
    const left = remainingCopies(i);
    if (left === 0) continue;
    const one = estimate(i);
    if (one === null) unknown += left;
    else seconds += one * left;
  }
  return { seconds, unknown };
}

// ---- edición de la lista ----------------------------------------------------------------------------

/** Agrega un elemento al final. Si ya está lleno, no hace nada. */
export function addItem(s: QueueState, item: QueueItem): QueueState {
  if (s.items.length >= MAX_ITEMS) return s;
  // Una cola terminada o detenida con elementos nuevos vuelve a poder arrancar.
  return { ...s, items: [...s.items, item], run: s.run === 'done' ? 'idle' : s.run };
}

/** Quita un elemento. El que está en la máquina no se puede quitar. */
export function removeItem(s: QueueState, id: string): QueueState {
  const item = itemById(s, id);
  if (!item || (s.run === 'running' && s.currentId === id)) return s;
  return retarget({ ...s, items: s.items.filter((i) => i.id !== id) });
}

/** Mueve un elemento a la posición `to` (0 = primero). */
export function moveItem(s: QueueState, id: string, to: number): QueueState {
  const from = s.items.findIndex((i) => i.id === id);
  if (from < 0) return s;
  const target = Math.max(0, Math.min(s.items.length - 1, Math.round(to)));
  if (target === from) return s;
  const items = s.items.slice();
  const [moved] = items.splice(from, 1);
  items.splice(target, 0, moved!);
  return { ...s, items };
}

export function setCopies(s: QueueState, id: string, copies: number): QueueState {
  const item = itemById(s, id);
  if (!item) return s;
  const n = clampCopies(copies);
  // Pedir menos copias que las ya hechas no deshace nada: el elemento queda completo.
  const status: QueueItemStatus = item.status === 'skipped' ? 'skipped' : item.done >= n ? 'done' : item.status === 'done' ? 'pending' : item.status;
  const next = { ...s, items: s.items.map((i) => (i.id === id ? { ...i, copies: n, status } : i)) };
  return s.currentId === id && s.run !== 'running' ? retarget(next) : next;
}

export function setNote(s: QueueState, id: string, note: string): QueueState {
  return { ...s, items: s.items.map((i) => (i.id === id ? { ...i, note: note.slice(0, 200) } : i)) };
}

/** Enlaza un elemento con una pestaña (al abrir su archivo o al cerrarse la pestaña). */
export function bindTab(s: QueueState, id: string, tabId: string | null): QueueState {
  return { ...s, items: s.items.map((i) => (i.id === id ? { ...i, tabId } : i)) };
}

/** Saltar un elemento (no se manda). El que está en la máquina no se puede saltar. */
export function skipItem(s: QueueState, id: string): QueueState {
  const item = itemById(s, id);
  if (!item || (s.run === 'running' && s.currentId === id)) return s;
  return retarget({ ...s, items: s.items.map((i) => (i.id === id ? { ...i, status: 'skipped' as const } : i)) });
}

/** Vuelve a poner en juego un elemento saltado o fallido. */
export function restoreItem(s: QueueState, id: string): QueueState {
  const item = itemById(s, id);
  if (!item || (item.status !== 'skipped' && item.status !== 'failed')) return s;
  const status: QueueItemStatus = item.done >= item.copies ? 'done' : 'pending';
  return { ...s, items: s.items.map((i) => (i.id === id ? { ...i, status } : i)), run: s.run === 'done' ? 'idle' : s.run };
}

/**
 * Si el elemento actual ya no está disponible (se quitó, se saltó o se completó) y la cola espera, pasa al
 * siguiente pendiente (sigue en `awaiting`) o termina. Nunca pasa a `running`.
 */
function retarget(s: QueueState): QueueState {
  if (s.run !== 'awaiting') return s;
  const cur = currentItem(s);
  if (cur && cur.status === 'pending' && remainingCopies(cur) > 0) return s;
  const next = nextPending(s);
  return next ? { ...s, currentId: next.id } : { ...s, run: 'done', currentId: null };
}

// ---- control de la cola -----------------------------------------------------------------------------

/**
 * Empieza (o reintenta tras una detención): la cola pasa a esperar la confirmación del primer trabajo
 * pendiente. NO inicia nada. Los elementos fallidos vuelven a pendientes.
 */
export function begin(s: QueueState): QueueState {
  if (s.run === 'running' || s.run === 'awaiting') return s;
  const items = s.items.map((i) => (i.status === 'failed' ? { ...i, status: 'pending' as const } : i));
  const base: QueueState = { ...s, items, pauseAfter: false, stopReason: null, stopMessage: '' };
  const next = nextPending(base);
  return next ? { ...base, run: 'awaiting', currentId: next.id } : { ...base, run: 'done', currentId: null };
}

/** Pausa: con un trabajo en la máquina, la cola se pausa al terminar éste; esperando confirmación, ya. */
export function pause(s: QueueState): QueueState {
  if (s.run === 'awaiting') return { ...s, run: 'paused' };
  if (s.run === 'running') return { ...s, pauseAfter: true };
  return s;
}

/** Quita la pausa (también la pedida durante un trabajo). La cola vuelve a esperar la confirmación, nunca arranca. */
export function resume(s: QueueState): QueueState {
  if (s.run === 'running') return { ...s, pauseAfter: false };
  if (s.run !== 'paused') return s;
  const cur = currentItem(s);
  const target = cur && cur.status === 'pending' && remainingCopies(cur) > 0 ? cur : nextPending(s);
  return target ? { ...s, run: 'awaiting', currentId: target.id } : { ...s, run: 'done', currentId: null };
}

/** Detiene la cola. Un trabajo que ya esté en la máquina NO se toca: sigue y la cola no continuará después. */
export function stop(s: QueueState, reason: StopReason = 'user', message = ''): QueueState {
  if (s.run === 'idle' || s.run === 'done' || s.run === 'stopped') return s;
  return { ...s, run: 'stopped', pauseAfter: false, stopReason: reason, stopMessage: message };
}

/**
 * La máquina empezó un trabajo que salió de la pestaña `tabId`. Solo cuenta si la cola lo esperaba
 * (`awaiting`) y es el documento del elemento actual; cualquier otro trabajo no mueve la cola.
 */
export function jobStarted(s: QueueState, tabId: string | null): QueueState {
  if (s.run !== 'awaiting') return s;
  const cur = currentItem(s);
  if (!cur || tabId === null || cur.tabId !== tabId) return s;
  return { ...s, run: 'running' };
}

/**
 * El trabajo de la cola terminó. Con éxito: cuenta la copia y la cola pasa a `awaiting` (o `paused` si se
 * pidió pausar, o `done`). Con cancelación o error: la cola se DETIENE; no se ofrece el siguiente.
 */
export function jobEnded(s: QueueState, end: JobEnd, message = ''): QueueState {
  if (s.run !== 'running') return s;
  if (end !== 'finished') {
    const items = s.items.map((i) => (i.id === s.currentId && end === 'error' ? { ...i, status: 'failed' as const } : i));
    return { ...s, items, run: 'stopped', pauseAfter: false, stopReason: end === 'error' ? 'failed' : 'cancelled', stopMessage: message };
  }
  const items = s.items.map((i) => {
    if (i.id !== s.currentId) return i;
    const done = i.done + 1;
    return { ...i, done, status: done >= i.copies ? ('done' as const) : i.status };
  });
  const after: QueueState = { ...s, items };
  const next = nextPending(after);
  if (!next) return { ...after, run: 'done', currentId: null, pauseAfter: false };
  // Aunque el mismo documento tenga más copias, SIEMPRE se vuelve a pedir confirmación.
  return { ...after, run: s.pauseAfter ? 'paused' : 'awaiting', currentId: next.id, pauseAfter: false };
}

/** Borra los elementos terminados o saltados (lo que está en la máquina se queda). */
export function clearFinished(s: QueueState): QueueState {
  if (s.run === 'running') return s;
  const items = s.items.filter((i) => i.status === 'pending' || i.status === 'failed');
  if (items.length === 0) return { ...EMPTY_QUEUE };
  const keep = s.currentId !== null && items.some((i) => i.id === s.currentId);
  const currentId = keep ? s.currentId : (nextPending({ ...s, items })?.id ?? null);
  const next = { ...s, items, currentId };
  return s.run === 'awaiting' && currentId === null ? { ...next, run: 'done' } : next;
}
