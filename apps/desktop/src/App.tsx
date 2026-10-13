import { useEffect } from 'react';
import { getApi, getApiKind } from '@/api';
import { commands } from '@/commands';
import { useShortcuts } from '@/commands/useShortcuts';
import { usePreviewSync } from '@/features/preview';
import { AppShell } from '@/features/shell';
import { bindMachineEvents, useStore } from '@/store';
import { applyThemeToDocument } from '@/store/view';
import { importDesign } from '@/features/history/files';

let demoLoaded = false;

export function App() {
  useShortcuts(commands);
  usePreviewSync();

  useEffect(() => {
    applyThemeToDocument(useStore.getState().theme);
    const api = getApi();
    let off: (() => void) | undefined;
    let cancelled = false;
    void bindMachineEvents(api).then((fn) => (cancelled ? fn() : (off = fn)));
    void useStore.getState().loadDevices();
    // En modo mock arranca con el diseño de ejemplo para poder trabajar la interfaz sin importar nada.
    if (getApiKind() === 'mock' && import.meta.env.VITE_DEMO !== 'off' && !demoLoaded) {
      demoLoaded = true; // StrictMode monta dos veces en desarrollo
      void importDesign();
    }
    return () => {
      cancelled = true;
      off?.();
    };
  }, []);

  return <AppShell />;
}
