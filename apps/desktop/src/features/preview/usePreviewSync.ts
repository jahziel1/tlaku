import { useEffect } from 'react';
import { useStore } from '@/store';

const DEBOUNCE_MS = 400;

/**
 * Recalcula el trabajo (estimación de tiempo y vista previa) cuando cambian el documento,
 * el dispositivo o las opciones. Con debounce: no se calcula en cada pulsación de flecha.
 * TODO(oleada B): con el motor real, calcular solo si la vista previa está visible o hace
 * falta la estimación, y cancelar el cálculo anterior si el motor lo permite.
 */
export function usePreviewSync(): void {
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const schedule = () => {
      clearTimeout(timer);
      timer = setTimeout(() => void useStore.getState().computePreview(), DEBOUNCE_MS);
    };
    const unsub = useStore.subscribe((s, prev) => {
      if (s.doc !== prev.doc || s.activeDeviceId !== prev.activeDeviceId || s.jobOptions !== prev.jobOptions) schedule();
    });
    schedule();
    return () => {
      clearTimeout(timer);
      unsub();
    };
  }, []);
}
