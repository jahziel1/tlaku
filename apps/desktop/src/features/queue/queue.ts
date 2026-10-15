import { getApiKind } from '@/api';
import { useJobLog } from '@/features/help/jobLog';
import { openProject } from '@/features/history/files';
import { pickPath } from '@/features/history/dialogs';
import { baseName } from '@/features/history/paths';
import { t } from '@/i18n';
import { findTabByPath, tabLabel, tabViews, useStore } from '@/store';
import { isJobBusy } from '@/store/selectors';
import {
  addItem,
  begin,
  bindTab,
  clearFinished,
  currentItem,
  jobEnded,
  jobStarted,
  makeItem,
  moveItem,
  pause,
  removeItem,
  resume,
  restoreItem,
  setCopies,
  setNote,
  skipItem,
  stop,
  type QueueItem,
  type QueueState,
} from './queueModel';
import { loadQueue, saveQueue } from './queuePersist';
import { useQueue } from './queueStore';

/**
 * Orquestación de la cola guiada de trabajos (el modelo puro está en `queueModel.ts`).
 *
 * SEGURIDAD: aquí NO existe ningún camino que envíe un trabajo a la máquina. `confirmStep` solo abre el
 * diálogo de inicio de siempre (`requestStart`: preflight y lista de comprobación incluidos) sobre la pestaña del
 * trabajo, después de que una persona marcó la confirmación del paso. Quien envía es ese diálogo (o, desde el
 * celular, `remote.ts` con su checklist y su encuadre). La cola solo OBSERVA que un trabajo empezó o terminó.
 */

const app = () => useStore.getState();
const queue = () => useQueue.getState();
const apply = (fn: (q: QueueState) => QueueState) => queue().update(fn);

// ---- agregar trabajos -------------------------------------------------------------------------------

/** Agrega la pestaña `tabId` (una sola vez por pestaña). Devuelve si se agregó. */
export function addTab(tabId: string): boolean {
  const s = app();
  const view = tabViews(s).find((v) => v.id === tabId);
  if (!view || queue().q.items.some((i) => i.tabId === tabId)) return false;
  const before = queue().q.items.length;
  apply((q) => addItem(q, makeItem({ source: 'tab', tabId, path: view.filePath, name: tabLabel(view, s.tabs.length > 1) })));
  return queue().q.items.length > before;
}

/** Agrega todas las pestañas abiertas que aún no están. Devuelve cuántas. */
export function addAllTabs(): number {
  return app().tabs.reduce((n, tab) => n + (addTab(tab.id) ? 1 : 0), 0);
}

/** Agrega un archivo de proyecto (se abre en una pestaña cuando le toque). Devuelve si se agregó. */
export function addFile(path: string): boolean {
  const q = queue().q;
  if (q.items.some((i) => i.path !== null && sameFile(i.path, path))) return false;
  // Si ya está abierto, el elemento nace enlazado a su pestaña.
  const tabId = findTabByPath(app(), path);
  const before = q.items.length;
  apply((cur) => addItem(cur, makeItem({ source: 'file', tabId, path, name: baseName(path) })));
  return queue().q.items.length > before;
}

const sameFile = (a: string, b: string) => a.replace(/\\/g, '/') === b.replace(/\\/g, '/');

/** Elige archivos `.tlaku` y los agrega. Devuelve cuántos. */
export async function addFilesFromDialog(): Promise<number> {
  const paths = await pickProjectPaths();
  return paths.reduce((n, p) => n + (addFile(p) ? 1 : 0), 0);
}

async function pickProjectPaths(): Promise<string[]> {
  if (getApiKind() !== 'tauri') {
    const one = await pickPath('openProject');
    return one ? [one] : [];
  }
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.open({ multiple: true, directory: false, filters: [{ name: 'Proyecto Tlaku', extensions: ['tlaku'] }] });
  if (Array.isArray(picked)) return picked;
  return typeof picked === 'string' ? [picked] : [];
}

// ---- estimado ---------------------------------------------------------------------------------------

/**
 * Segundos de UNA copia: el estimado de la vista previa de su pestaña (si ya está calculada) o, si no, lo que
 * tardó la última vez que ese archivo terminó bien (historial). `null` si no hay manera de saberlo.
 */
export function estimateOne(item: QueueItem): number | null {
  const s = app();
  if (item.tabId) {
    const preview = item.tabId === s.activeTabId ? s.preview : (s.tabs.find((e) => e.id === item.tabId)?.data?.preview ?? null);
    if (preview && preview.total_s > 0) return preview.total_s;
  }
  if (item.path) {
    const last = useJobLog.getState().entries.find((e) => e.result === 'finished' && e.documentPath === item.path && e.actualS > 0);
    if (last) return last.actualS;
  }
  return null;
}

/** El elemento depende de una pestaña que ya no existe y no tiene archivo con el que volver a abrirla. */
export function isMissing(item: QueueItem): boolean {
  if (item.path) return false;
  return !item.tabId || !app().tabs.some((e) => e.id === item.tabId);
}

// ---- edición ---------------------------------------------------------------------------------------

export const removeFromQueue = (id: string) => apply((q) => removeItem(q, id));
export const moveInQueue = (id: string, to: number) => apply((q) => moveItem(q, id, to));
export const setItemCopies = (id: string, copies: number) => apply((q) => setCopies(q, id, copies));
export const setItemNote = (id: string, note: string) => apply((q) => setNote(q, id, note));
export const skipInQueue = (id: string) => apply((q) => skipItem(q, id));
export const restoreInQueue = (id: string) => apply((q) => restoreItem(q, id));
export const clearFinishedFromQueue = () => apply((q) => clearFinished(q));

// ---- control ---------------------------------------------------------------------------------------

/** Empieza la cola: pide la confirmación del primer trabajo. No envía nada. */
export function startQueue(): void {
  apply(begin);
  const q = queue().q;
  if (q.run === 'awaiting') queue().setStepOpen(true);
  else if (q.run === 'done') app().notify('info', t('queue.notice.nothingToRun'));
}

export function pauseQueue(): void {
  apply(pause);
  if (queue().q.run === 'paused') queue().setStepOpen(false);
}

export function resumeQueue(): void {
  apply(resume);
  if (queue().q.run === 'awaiting') queue().setStepOpen(true);
}

/** Detiene la cola. Un trabajo que ya está en la máquina NO se toca: sigue hasta el final y la cola no continúa. */
export function stopQueue(): void {
  apply((q) => stop(q, 'user'));
  queue().setStepOpen(false);
}

/** Salta el trabajo que espera confirmación y pasa al siguiente (que también espera confirmación). */
export function skipCurrent(): void {
  const cur = currentItem(queue().q);
  if (!cur || queue().q.run !== 'awaiting') return;
  apply((q) => skipItem(q, cur.id));
  if (queue().q.run !== 'awaiting') queue().setStepOpen(false);
}

// ---- el paso de confirmación -------------------------------------------------------------------------

/**
 * La pestaña del elemento: la enlazada si sigue abierta; si no, se abre su archivo (queda activa). `null` si no
 * hay manera (pestaña cerrada sin archivo o archivo que no se pudo abrir).
 */
export async function ensureItemTab(item: QueueItem): Promise<string | null> {
  const s = app();
  if (item.tabId && s.tabs.some((e) => e.id === item.tabId)) return item.tabId;
  if (!item.path) return null;
  const open = findTabByPath(s, item.path);
  const tabId = open ?? ((await openProject(item.path)) ? app().activeTabId : null);
  apply((q) => bindTab(q, item.id, tabId));
  return tabId;
}

/** Encuadra el trabajo del paso (opcional): lleva su pestaña al frente y usa el encuadre de siempre. */
export async function frameStep(): Promise<boolean> {
  const item = currentItem(queue().q);
  if (!item || queue().q.run !== 'awaiting') return false;
  const tabId = await ensureItemTab(item);
  if (!tabId) {
    app().notify('error', t('queue.err.missing', { name: item.name }));
    return false;
  }
  app().activateTab(tabId);
  if (!app().connection.connected) {
    app().notifyKey('warning', 'reason.needMachine');
    return false;
  }
  await app().frameJob();
  return true;
}

/**
 * Una persona confirmó el paso (material cambiado, lista de comprobación): lleva el documento al frente y abre el
 * diálogo de inicio NORMAL. No inicia nada. Devuelve si el diálogo de inicio quedó abierto.
 */
export async function confirmStep(): Promise<boolean> {
  const before = queue().q;
  const item = currentItem(before);
  if (before.run !== 'awaiting' || !item) return false;
  const tabId = await ensureItemTab(item);
  // Mientras se abría el archivo la cola pudo cambiar (se pausó, se saltó): solo sigue si todo sigue igual.
  const now = queue().q;
  if (now.run !== 'awaiting' || now.currentId !== item.id) return false;
  if (!tabId) {
    apply((q) => stop(q, 'missing', t('queue.err.missing', { name: item.name })));
    queue().setStepOpen(false);
    app().notify('error', t('queue.err.missing', { name: item.name }));
    return false;
  }
  app().activateTab(tabId);
  queue().setStepOpen(false);
  app().requestStart();
  const opened = app().dialog?.kind === 'start';
  // Bloqueado (sin máquina, ocupada...): `requestStart` ya avisó; el paso sigue esperando.
  if (!opened) queue().setStepOpen(true);
  return opened;
}

// ---- observar la máquina -----------------------------------------------------------------------------

/**
 * Sigue los trabajos de la máquina: cuando empieza uno de la pestaña que la cola esperaba, pasa a `running`;
 * cuando termina, cuenta la copia y vuelve a PEDIR confirmación (o se detiene si falló o se canceló).
 * Devuelve cómo dejar de seguir.
 */
export function startQueueWatcher(): () => void {
  let wasBusy = isJobBusy(app());
  return useStore.subscribe((s) => {
    const busy = isJobBusy(s);
    if (busy === wasBusy) return;
    wasBusy = busy;
    if (busy) {
      apply((q) => jobStarted(q, s.jobOrigin?.tabId ?? null));
      if (queue().q.run === 'running') queue().setStepOpen(false);
      return;
    }
    const end = s.jobState;
    if (end !== 'finished' && end !== 'cancelled' && end !== 'error') return;
    const prev = queue().q;
    if (prev.run !== 'running') return;
    apply((q) => jobEnded(q, end, s.jobMessage));
    const next = queue().q;
    if (next.run === 'awaiting') {
      // Termina un trabajo: la cola pide cambiar el material y confirmar. NO avanza sola.
      queue().setStepOpen(true);
      app().notify('info', t('queue.notice.next', { name: currentItem(next)?.name ?? '' }));
    } else if (next.run === 'stopped') {
      queue().setStepOpen(false);
      app().notify('warning', t(end === 'error' ? 'queue.notice.stoppedFailed' : 'queue.notice.stoppedCancelled', { name: currentItem(next)?.name ?? '' }));
    } else if (next.run === 'done') {
      app().notify('success', t('queue.notice.done'));
    } else if (next.run === 'paused') {
      app().notify('info', t('queue.notice.paused'));
    }
  });
}

// ---- recordar la definición ------------------------------------------------------------------------

/**
 * Al abrir la app recupera la definición de la cola de la sesión anterior (solo archivos, copias y avisos; sin
 * copias hechas y detenida) y la va guardando cada vez que cambia la lista. Devuelve cómo dejar de guardar.
 */
export function startQueuePersistence(): () => void {
  if (queue().q.items.length === 0) {
    for (const saved of loadQueue()) {
      const tabId = saved.path ? findTabByPath(app(), saved.path) : null;
      apply((q) => addItem(q, { ...saved, tabId }));
    }
  }
  return useQueue.subscribe((s, prev) => {
    if (s.q.items !== prev.q.items) saveQueue(s.q.items);
  });
}
