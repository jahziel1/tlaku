import { useEffect } from 'react';
import { SaveVersionDialog } from './SaveVersionDialog';
import { startAutoSnapshots } from './autoSnapshot';
import { VersionsDialog } from './VersionsDialog';
import { useVersionsUi } from './versionsUi';

/**
 * Punto de montaje del historial de versiones (lo monta el shell una vez): arranca las instantáneas automáticas
 * y pinta los diálogos «Historial de versiones» y «Guardar versión…». Va antes de `PromptHost` para que las
 * confirmaciones (reemplazar, borrar) queden por encima del diálogo.
 */
export function VersionsHost() {
  const open = useVersionsUi((s) => s.open);
  const nameOpen = useVersionsUi((s) => s.nameOpen);
  useEffect(() => startAutoSnapshots(), []);
  return (
    <>
      {open && <VersionsDialog />}
      {nameOpen && <SaveVersionDialog />}
    </>
  );
}
