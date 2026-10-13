import { useEffect } from 'react';
import { RecoveryDialog } from './RecoveryDialog';
import { startAutosave } from './autosave';
import { checkRecovery, useRecovery } from './recover';

export { discardOnClose } from './autosave';

/** Evita revisar dos veces al montar en modo estricto de desarrollo. */
let checked = false;

/**
 * Punto de montaje del autoguardado y la recuperación (lo monta el shell una sola vez): arranca el
 * autoguardado, revisa si quedaron copias de una sesión anterior y muestra el diálogo si las hay.
 */
export function RecoveryHost() {
  const open = useRecovery((s) => s.open);
  useEffect(() => {
    if (!checked) {
      checked = true;
      void checkRecovery();
    }
    return startAutosave();
  }, []);
  return open ? <RecoveryDialog /> : null;
}
