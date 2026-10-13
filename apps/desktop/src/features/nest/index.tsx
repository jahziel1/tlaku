import { NestDialog } from './NestDialog';
import { useNestUi } from './nestStore';

export { useNestUi } from './nestStore';

/** Punto de montaje del nesting (lo monta el shell una sola vez): dibuja el diálogo si está abierto. */
export function NestDialogHost() {
  const open = useNestUi((s) => s.open);
  return open ? <NestDialog /> : null;
}
