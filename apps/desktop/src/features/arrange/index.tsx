import { useArrangeUi } from './uiStore';
import { AlignDialog } from './AlignDialog';
import { ArrayDialog } from './ArrayDialog';
import { BooleanDialog } from './BooleanDialog';
import { CleanupDialog } from './CleanupDialog';
import { OffsetDialog } from './OffsetDialog';
import { SizeSelectDialog } from './SizeSelectDialog';

export { useArrangeUi } from './uiStore';

/**
 * Punto de montaje del área Organizar (lo monta el shell una sola vez): dibuja el diálogo abierto
 * en `useArrangeUi` (forma principal de una booleana, offset, limpieza de trazos, alinear, arreglos).
 */
export function ArrangeDialogHost() {
  const dialog = useArrangeUi((s) => s.dialog);
  if (!dialog) return null;
  switch (dialog.kind) {
    case 'boolean':
      return <BooleanDialog key={dialog.op} op={dialog.op} />;
    case 'offset':
      return <OffsetDialog />;
    case 'cleanup':
      return <CleanupDialog key={dialog.op} op={dialog.op} />;
    case 'align':
      return <AlignDialog />;
    case 'circles':
      return <SizeSelectDialog />;
    case 'grid':
    case 'circular':
      return <ArrayDialog key={dialog.kind} kind={dialog.kind} />;
  }
}
