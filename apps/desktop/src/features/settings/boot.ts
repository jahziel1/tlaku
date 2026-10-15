import { useEffect } from 'react';
import { getApi } from '@/api';
import { useJobLog } from '@/features/help/jobLog';
import { useRecents } from '@/features/history/recent';
import { useStore } from '@/store';
import { loadPrefsFromDisk, startPrefsPersistence, startPrefsWindowSync, startThemeSync } from './persist';

/**
 * Arranque de las preferencias (se monta una vez, en `SettingsHost`):
 *  1. lee `preferences.json` y lo aplica;
 *  2. guarda cada cambio (copia local al instante, archivo con una pausa corta);
 *  3. mantiene el tema de la vista igual a la preferencia;
 *  4. adopta los cambios que haga otra ventana de la app (preferencias compartidas);
 *  5. aplica el máximo de recientes y quita los archivos que ya no existen.
 */
export function usePrefsBoot(): void {
  useEffect(() => {
    const api = getApi();
    const persistence = startPrefsPersistence(api, useStore);
    const stopTheme = startThemeSync(useStore);
    const stopWindowSync = startPrefsWindowSync(useStore);
    // Un cambio del máximo de recientes recorta la lista al momento.
    const stopRecents = useStore.subscribe((s, prev) => {
      if (s.prefs.recentMax !== prev.prefs.recentMax) useRecents.getState().trim(s.prefs.recentMax);
    });
    const flush = () => void persistence.flush();
    const onHide = () => document.visibilityState === 'hidden' && flush();
    window.addEventListener('pagehide', flush);
    document.addEventListener('visibilitychange', onHide);

    let cancelled = false;
    void loadPrefsFromDisk(api, useStore).then(() => {
      if (cancelled) return;
      useRecents.getState().trim(useStore.getState().prefs.recentMax);
      void useRecents.getState().pruneMissing((paths) => api.pathsExist({ paths }));
      // El historial de trabajos (archivo de la carpeta de datos) y la migración del registro viejo de localStorage.
      void useJobLog.getState().load();
    });

    return () => {
      cancelled = true;
      window.removeEventListener('pagehide', flush);
      document.removeEventListener('visibilitychange', onHide);
      stopRecents();
      stopTheme();
      stopWindowSync();
      persistence.stop();
    };
  }, []);
}
