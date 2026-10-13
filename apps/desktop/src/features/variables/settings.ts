import type { VariableText } from '@/api/types';
import { useStore } from '@/store';
import { variableTextOf } from './defaults';

export { DEFAULT_VARIABLE_TEXT, variableTextOf } from './defaults';

/** Cambia los ajustes del texto variable con un paso de deshacer; escribir seguido se funde en uno. */
export function editVariableText(patch: Partial<VariableText>, field: string): void {
  useStore.getState().applyDocChange(
    'variables.history.edit',
    (doc) => {
      const cur = variableTextOf(doc);
      const next = { ...cur, ...patch };
      const same = doc.variable_text !== undefined && (Object.keys(next) as (keyof VariableText)[]).every((k) => Object.is(next[k], doc.variable_text?.[k]));
      return same ? doc : { ...doc, variable_text: next };
    },
    `variables:${field}`,
  );
}
