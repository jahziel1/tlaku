import { useArrangeUi } from './uiStore';
import { AlignDialog } from './AlignDialog';
import { ArrayDialog } from './ArrayDialog';
import { BooleanDialog } from './BooleanDialog';
import { OffsetDialog } from './OffsetDialog';

export { useArrangeUi } from './uiStore';

/**
 * Punto de montaje del área Organizar (lo monta el shell una sola vez): dibuja el diálogo abierto
 * en `useArrangeUi` (forma principal de una booleana, offset, alinear, arreglos).
 */
export function ArrangeDialogHost() {
  const dialog = useArrangeUi((s) => s.dialog);
  if (!dialog) return null;
  switch (dialog.kind) {
    case 'boolean':
      return <BooleanDialog key={dialog.op} op={dialog.op} />;
    case 'offset':
      return <OffsetDialog />;
    case 'align':
      return <AlignDialog />;
    case 'grid':
    case 'circular':
      return <ArrayDialog key={dialog.kind} kind={dialog.kind} />;
  }
}
