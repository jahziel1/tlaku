import type { LaserApi } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { readPref, writePref } from '@/lib/platform';
import { tabViews, type AppState } from '@/store';

/**
 * Sesión de pestañas: al cerrar, se recuerda qué proyectos (con archivo guardado) había abiertos y cuál estaba
 * activo; al abrir Tlaku se reabren (preferencia `restoreTabs`, encendida de fábrica). Las pestañas «Sin título» no
 * se reabren: si tenían trabajo sin guardar, lo ofrece la recuperación (`features/recovery`). Solo la ventana
 * principal guarda y restaura; las ventanas extra («Nueva ventana», «Mover a ventana nueva») son efímeras.
 */
export const SESSION_KEY = 'tabs.session';
/** Tope de pestañas que se reabren (un archivo dañado de sesión no debe abrir cientos). */
export const SESSION_MAX_TABS = 24;

export interface TabSession {
  v: 1;
  paths: string[];
  /** Índice en `paths` de la pestaña activa (o -1 si la activa no tenía archivo). */
  active: number;
}

type SessionStore = {
  getState: () => AppState;
  subscribe: (listener: (state: AppState, prev: AppState) => void) => () => void;
};

/** La sesión que describe el estado de ahora. */
export function sessionOf(s: Pick<AppState, 'tabs' | 'activeTabId' | 'filePath' | 'dirty'>): TabSession {
  const withFile = tabViews(s).filter((v) => v.filePath !== null);
  const paths = withFile.map((v) => v.filePath!);
  return { v: 1, paths, active: withFile.findIndex((v) => v.active) };
}

/** Lee y sanea la sesión guardada (`null` si no hay o está dañada). */
export function parseSession(raw: string | null): TabSession | null {
  if (!raw) return null;
  try {
    const v = JSON.parse(raw) as Partial<TabSession>;
    if (v.v !== 1 || !Array.isArray(v.paths)) return null;
    const paths = [...new Set(v.paths.filter((p): p is string => typeof p === 'string' && p.length > 0))].slice(0, SESSION_MAX_TABS);
    const active = typeof v.active === 'number' && v.active >= 0 && v.active < paths.length ? Math.floor(v.active) : -1;
    return { v: 1, paths, active };
  } catch {
    return null;
  }
}

export const sameSession = (a: TabSession, b: TabSession): boolean => a.active === b.active && a.paths.length === b.paths.length && a.paths.every((p, i) => p === b.paths[i]);

/** Guarda la sesión cada vez que cambian las pestañas, sus archivos o la activa. Devuelve cómo dejar de guardar. */
export function startSessionSaving(store: SessionStore, write: (key: string, value: string) => void = writePref): () => void {
  let last: TabSession | null = null;
  const save = (s: AppState) => {
    const now = sessionOf(s);
    if (last && sameSession(last, now)) return;
    last = now;
    write(SESSION_KEY, JSON.stringify(now));
  };
  save(store.getState());
  return store.subscribe((s, prev) => {
    if (s.tabs !== prev.tabs || s.activeTabId !== prev.activeTabId || s.filePath !== prev.filePath) save(s);
  });
}

export interface RestoreDeps {
  api: Pick<LaserApi, 'loadProject'>;
  store: SessionStore;
  read?: (key: string) => string | null;
}

/** Reabre las pestañas de la sesión anterior. Devuelve cuántas se abrieron. */
export async function restoreSession({ api, store, read = readPref }: RestoreDeps): Promise<number> {
  const state = () => store.getState();
  if (!state().prefs.restoreTabs) return 0;
  const session = parseSession(read(SESSION_KEY));
  if (!session || session.paths.length === 0) return 0;
  let opened = 0;
  let activeTab: string | null = null;
  for (const [i, path] of session.paths.entries()) {
    try {
      const doc = await api.loadProject({ path });
      const id = state().openInTab(doc, path);
      opened++;
      if (i === session.active) activeTab = id;
      void state().computePreview();
    } catch (e) {
      state().notify('warning', t('tabs.restoreFailed', { name: path.split(/[\\/]/).pop() || path, error: errorMessage(e) }));
    }
  }
  if (activeTab && activeTab !== state().activeTabId) state().activateTab(activeTab);
  if (opened > 0) state().fitContent();
  return opened;
}
