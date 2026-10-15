/** Área `variables`: texto variable (serie, CSV, fecha y hora) en los textos del diseño. */
import { VariablesDialog } from './VariablesDialog';
import { useVariablesUi } from './context';
import { useVariablesSync } from './useVariablesSync';

export { VariablesDialog } from './VariablesDialog';
export { InsertVariable } from './InsertVariable';
export { ImageVariableField } from './ImageVariableField';
export { ColumnPicker } from './ColumnPicker';
export { useVariablesUi } from './context';
export { advanceAfterJob } from './advance';
export { displayContext } from './context';
export { issueMessage } from './messages';
export { resolveTemplate } from './resolve';
export { variableCodeOf } from './varObjects';
export { resolveDocForOutput, resolveJobArgs, VariableError } from './jobResolve';

/**
 * Mantiene el CSV y el reloj al día y dibuja el diálogo "Texto variable" cuando está abierto. Lo
 * monta el panel de texto (que el shell ya incluye siempre), igual que el diálogo de códigos.
 */
export function VariablesHost() {
  useVariablesSync();
  const open = useVariablesUi((s) => s.open);
  return open ? <VariablesDialog /> : null;
}
