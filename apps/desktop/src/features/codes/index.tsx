/** Área `codes`: diálogo "Insertar código" (QR y códigos de barras como formas vectoriales). */
import { CodesDialog } from './CodesDialog';
import { useCodesDialog } from './codesStore';

export { useCodesDialog } from './codesStore';
export { CodesDialog } from './CodesDialog';

/**
 * Dibuja el diálogo cuando está abierto. Lo monta el panel de texto (que ya vive en el shell)
 * una sola vez, así el área no necesita tocar el shell.
 */
export function CodesDialogHost() {
  const open = useCodesDialog((s) => s.open);
  return open ? <CodesDialog /> : null;
}
