import { useEffect } from 'react';
import { RecoveryDialog } from './RecoveryDialog';
import { startAutosave } from './autosave';
import { useStore } from '@/store';
import { checkRecovery, useRecovery } from './recover';

export { discardOnClose } from './autosave';

/** Evita revisar dos veces al montar en modo estricto de desarrollo. */
let checked = false;

/**
 * Tras revisar las copias del diseño: si el último trabajo enviado a la máquina se interrumpió (corte de luz, app
 * cerrada), ofrece continuarlo. Espera a que se cierre el diálogo de recuperación para no apilar dos avisos.
 */
function offerInterruptedJob(): void {
  const ask = () => void useStore.getState().checkInterrupted({ open: true });
  if (!useRecovery.getState().open) return ask();
  const off = useRecovery.subscribe((s) => {
    if (!s.open) {
      off();
      ask();
    }
  });
}

/**
 * Punto de montaje del autoguardado y la recuperación (lo monta el shell una sola vez): arranca el
 * autoguardado, revisa si quedaron copias de una sesión anterior y muestra el diálogo si las hay.
 */
export function RecoveryHost() {
  const open = useRecovery((s) => s.open);
  useEffect(() => {
    if (!checked) {
      checked = true;
      void checkRecovery().then(offerInterruptedJob);
    }
    return startAutosave();
  }, []);
  return open ? <RecoveryDialog /> : null;
}
