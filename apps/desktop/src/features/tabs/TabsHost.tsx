import { useEffect } from 'react';
import { getApi, getApiKind } from '@/api';
import { cancelGesture } from '@/features/canvas/interaction';
import { useStore } from '@/store';
import { receiveHandoff } from './handoff';
import { restoreSession, startSessionSaving } from './session';

/** Evita repetir el arranque si React monta dos veces en desarrollo. */
let started = false;

/** ¿Es la ventana principal? Solo ella guarda y restaura la sesión de pestañas. */
export async function isMainWindow(): Promise<boolean> {
  if (getApiKind() !== 'tauri') return true;
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    return getCurrentWindow().label === 'main';
  } catch {
    return true;
  }
}

/** Espera a que las preferencias del disco estén aplicadas (o `ms` como máximo): la sesión depende de `restoreTabs`. */
function prefsReady(ms: number): Promise<void> {
  if (useStore.getState().prefsLoaded) return Promise.resolve();
  return new Promise((resolve) => {
    const timer = setTimeout(done, ms);
    const off = useStore.subscribe((s) => {
      if (s.prefsLoaded) done();
    });
    function done() {
      clearTimeout(timer);
      off();
      resolve();
    }
  });
}

/**
 * Arranque de las pestañas (lo monta el shell una vez): recoge la pestaña que otra ventana le mandó, reabre la sesión
 * anterior (ventana principal) y empieza a guardarla; además, cancela el gesto del lienzo al cambiar de pestaña.
 */
export function TabsHost() {
  useEffect(() => {
    if (!started) {
      started = true;
      // El arranque no depende de que el componente siga montado (en desarrollo React monta dos veces).
      void (async () => {
        const handed = await receiveHandoff();
        if (handed || !(await isMainWindow())) return;
        await prefsReady(3000);
        await restoreSession({ api: getApi(), store: useStore });
        stopSaving = startSessionSaving(useStore);
      })();
    }
    return useStore.subscribe((s, prev) => {
      // Un arrastre a medias no debe seguir en otro documento.
      if (s.tabSwitches !== prev.tabSwitches) cancelGesture();
    });
  }, []);
  return null;
}

let stopSaving: () => void = () => {};

/** Solo para pruebas: deja el arranque listo para repetirse. */
export function resetTabsHostForTests(): void {
  stopSaving();
  stopSaving = () => {};
  started = false;
}
