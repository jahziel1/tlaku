import type { LaserApi } from '@/api';
import { errorMessage } from '@/lib/errors';
import { t } from '@/i18n';
import type { AppState } from '@/store';
import { CACHE_STORAGE_KEY, prefsEqual, readCache, writeCache } from '@/store/prefs';
import { buildFile } from './bundle';

/** Lo mínimo que se necesita del store: leerlo, escribirlo y escucharlo (sirve el store real y el de las pruebas). */
export interface PrefsStore {
  getState: () => AppState;
  subscribe: (listener: (state: AppState, prev: AppState) => void) => () => void;
}

export const SAVE_DEBOUNCE_MS = 400;

/**
 * Lee `preferences.json` y lo aplica al estado. Devuelve `true` si ya se puede escribir.
 *
 * Si falla, se avisa y se reintenta una vez: un archivo dañado lo aparta el motor y la segunda
 * lectura ya no lo ve. Una versión desconocida falla las dos veces y entonces NO se escribe nada
 * este arranque, para no pisar el archivo de una versión más nueva de la app.
 */
export async function loadPrefsFromDisk(api: Pick<LaserApi, 'loadPreferences'>, store: PrefsStore): Promise<boolean> {
  const state = () => store.getState();
  let file;
  try {
    file = await api.loadPreferences();
  } catch (e) {
    state().notify('error', t('prefs.notice.loadFailed', { error: errorMessage(e) }));
    try {
      file = await api.loadPreferences();
    } catch {
      return false;
    }
  }
  if (file) state().replacePrefs({ prefs: file.prefs, shortcuts: file.shortcuts }, true);
  else state().replacePrefs({ prefs: state().prefs, shortcuts: state().shortcutOverrides }, true);
  return true;
}

/**
 * Guarda las preferencias cuando cambian: de inmediato la copia local y, tras una pausa corta (para
 * no escribir en cada tecla de un campo), el archivo. Hasta que se haya leído el archivo no escribe.
 * Devuelve `{ stop, flush }`.
 */
export function startPrefsPersistence(
  api: Pick<LaserApi, 'savePreferences'>,
  store: PrefsStore,
  debounceMs = SAVE_DEBOUNCE_MS,
): { stop: () => void; flush: () => Promise<void> } {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let failing = false;

  const write = async () => {
    timer = null;
    const s = store.getState();
    if (!s.prefsLoaded) return;
    try {
      await api.savePreferences({ data: buildFile(s.prefs, s.shortcutOverrides) });
      failing = false;
    } catch (e) {
      if (!failing) s.notify('error', t('prefs.notice.saveFailed', { error: errorMessage(e) }));
      failing = true;
    }
  };
  const schedule = () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => void write(), debounceMs);
  };

  const off = store.subscribe((s, prev) => {
    const changed = s.prefs !== prev.prefs || s.shortcutOverrides !== prev.shortcutOverrides;
    if (changed) writeCache({ prefs: s.prefs, shortcuts: s.shortcutOverrides });
    if (s.prefsLoaded && (changed || !prev.prefsLoaded)) schedule();
  });
  return {
    stop: () => {
      off();
      if (timer) clearTimeout(timer);
    },
    flush: async () => {
      if (timer) {
        clearTimeout(timer);
        await write();
      }
    },
  };
}

/**
 * Mantiene iguales la preferencia `theme` y el tema de la vista (que también cambia con el comando
 * «Cambiar tema»). Quien cambie primero arrastra al otro.
 */
export function startThemeSync(store: PrefsStore): () => void {
  const reconcile = (s: AppState, prev: AppState | null) => {
    if (prev && s.prefs.theme !== prev.prefs.theme && s.prefs.theme !== s.theme) s.setTheme(s.prefs.theme);
    else if (prev && s.theme !== prev.theme && s.theme !== s.prefs.theme) s.setPref('theme', s.theme);
  };
  const initial = store.getState();
  if (initial.prefs.theme !== initial.theme) initial.setTheme(initial.prefs.theme);
  return store.subscribe(reconcile);
}

/**
 * Varias ventanas de la app comparten las preferencias: cuando otra ventana guarda la copia local
 * (`localStorage`), esta la adopta si es distinta. Cada ventana escribe `preferences.json` por su
 * cuenta con el mismo contenido, así que no hay nada más que sincronizar. Devuelve cómo dejar de escuchar.
 */
export function startPrefsWindowSync(store: PrefsStore, target: Pick<Window, 'addEventListener' | 'removeEventListener'> = window): () => void {
  const onStorage = (e: Event) => {
    const key = (e as StorageEvent).key;
    // `key === null` es «se vació el almacenamiento»: no es un cambio de preferencias.
    if (key !== CACHE_STORAGE_KEY) return;
    const s = store.getState();
    if (!s.prefsLoaded) return;
    const incoming = readCache();
    if (prefsEqual(incoming.prefs, s.prefs) && JSON.stringify(incoming.shortcuts) === JSON.stringify(s.shortcutOverrides)) return;
    s.replacePrefs({ prefs: incoming.prefs, shortcuts: incoming.shortcuts });
  };
  target.addEventListener('storage', onStorage);
  return () => target.removeEventListener('storage', onStorage);
}
