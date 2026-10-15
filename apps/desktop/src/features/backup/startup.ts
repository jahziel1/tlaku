import { useEffect } from 'react';
import { useStore } from '@/store';
import { runBackupStartup } from './actions';
import { useBackup } from './store';

/** Espera tras arrancar para no competir con la carga de la aplicación. */
export const STARTUP_DELAY_MS = 3000;

let started = false;
/** Para las pruebas. */
export const resetBackupStartup = (): void => {
  started = false;
};

/**
 * Una vez por arranque, ya cargadas las preferencias y las máquinas: revisa si hay un respaldo más
 * nuevo de otra computadora (y lo ofrece) o si toca el respaldo programado.
 */
export function useBackupStartup(): void {
  const ready = useStore((s) => s.prefsLoaded && s.devices.length > 0);
  useEffect(() => {
    if (!ready || started) return;
    const timer = setTimeout(() => {
      if (started) return;
      started = true;
      void runBackupStartup()
        .then(() => {
          if (useBackup.getState().newer) useBackup.setState({ promptOpen: true });
        })
        .catch(() => undefined);
    }, STARTUP_DELAY_MS);
    return () => clearTimeout(timer);
  }, [ready]);
}
