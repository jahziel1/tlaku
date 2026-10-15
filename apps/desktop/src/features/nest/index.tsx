import { NestDialog } from './NestDialog';
import { useNestUi } from './nestStore';
import { useRemnants } from './remnantStore';
import { RemnantsDialog } from './RemnantsDialog';

export { useNestUi } from './nestStore';
export { setRemnantsPanel } from './remnantStore';

/**
 * Punto de montaje del nesting (lo monta el shell una sola vez): dibuja el diálogo si está abierto y,
 * encima, el inventario de retazos (se abre desde el diálogo o desde Organizar > Retazos guardados).
 */
export function NestDialogHost() {
  const open = useNestUi((s) => s.open);
  const remnants = useRemnants((s) => s.panelOpen);
  return (
    <>
      {open ? <NestDialog /> : null}
      {remnants ? <RemnantsDialog /> : null}
    </>
  );
}
