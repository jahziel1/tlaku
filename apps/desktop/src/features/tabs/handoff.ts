import { getApi } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { tabDoc, tabLabel, tabViews, useStore } from '@/store';

/**
 * «Mover a ventana nueva»: la pestaña viaja como una copia de recuperación con id propio (el motor ya sabe
 * guardar documentos grandes de forma atómica). La ventana nueva la lee al arrancar (`?handoff=...` en su
 * dirección), la abre en una pestaña y borra la copia. Mientras tanto la copia es «viva» de la ventana
 * de origen, así que ninguna otra ventana la ofrece para recuperar; si la ventana nueva nunca la recoge,
 * queda como copia huérfana y se ofrece al volver a abrir Tlaku (no se pierde el trabajo).
 */
export interface Handoff {
  id: string;
  path: string | null;
  dirty: boolean;
}

/** Parámetro que se pasa a `new_window`: JSON en una dirección, solo con caracteres seguros. */
export function encodeHandoff(h: Handoff): string {
  return encodeURIComponent(JSON.stringify(h));
}

/** Lee `?handoff=...` de la dirección de la ventana. `null` si no hay o está mal formado. */
export function parseHandoff(search: string): Handoff | null {
  const raw = new URLSearchParams(search).get('handoff');
  if (!raw) return null;
  try {
    const v = JSON.parse(decodeURIComponent(raw)) as Partial<Handoff>;
    if (typeof v.id !== 'string' || !/^[A-Za-z0-9_-]{1,100}$/.test(v.id)) return null;
    return { id: v.id, path: typeof v.path === 'string' ? v.path : null, dirty: v.dirty === true };
  } catch {
    return null;
  }
}

function newHandoffId(): string {
  const bytes = new Uint8Array(6);
  globalThis.crypto.getRandomValues(bytes);
  return `handoff-${Date.now().toString(36)}-${[...bytes].map((b) => b.toString(16).padStart(2, '0')).join('')}`;
}

/** Abre la pestaña `tabId` en una ventana nueva y la cierra aquí (sin preguntar: el documento sigue vivo allá). */
export async function sendTabToNewWindow(tabId: string): Promise<boolean> {
  const s = useStore.getState();
  const view = tabViews(s).find((v) => v.id === tabId);
  const doc = tabDoc(s, tabId);
  if (!view || !doc || s.tabs.length < 2) return false;
  const api = getApi();
  const handoff: Handoff = { id: newHandoffId(), path: view.filePath, dirty: view.dirty };
  try {
    await api.autosaveWrite({ id: handoff.id, name: tabLabel(view, true), path: view.filePath, doc });
    await api.newWindow({ handoff: encodeHandoff(handoff) });
  } catch (e) {
    await api.autosaveDiscard({ id: handoff.id }).catch(() => undefined);
    useStore.getState().notify('error', t('tabs.moveFailed', { error: errorMessage(e) }));
    return false;
  }
  // La pestaña se cierra al final: si algo falla antes, el documento sigue aquí.
  useStore.getState().removeTab(tabId);
  useStore.getState().notify('success', t('tabs.moved'));
  return true;
}

/** En una ventana recién abierta por `sendTabToNewWindow`: recoge la pestaña. `true` si había una. */
export async function receiveHandoff(search: string = typeof location === 'undefined' ? '' : location.search): Promise<boolean> {
  const h = parseHandoff(search);
  if (!h) return false;
  const api = getApi();
  try {
    const doc = await api.autosaveRead({ id: h.id });
    const s = useStore.getState();
    s.openInTab(doc, h.path, { unsaved: h.dirty });
    s.fitContent();
    void s.computePreview();
    await api.autosaveDiscard({ id: h.id }).catch(() => undefined);
    // Recargar la ventana no debe volver a abrir la pestaña.
    if (typeof history !== 'undefined') history.replaceState(null, '', location.pathname);
    return true;
  } catch (e) {
    useStore.getState().notify('error', t('tabs.handoffFailed', { error: errorMessage(e) }));
    return false;
  }
}
