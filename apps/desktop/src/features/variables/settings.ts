import type { DataSource, Document, ShortRows, VariableText } from '@/api/types';
import { useStore } from '@/store';
import { variableTextOf } from './defaults';
import { addSource, makeMain, removeSource, renameInTemplate, renameSource, setOnShort, setSourcePath } from './sources';

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

/**
 * Cambia la lista de fuentes (y `csv_path`, que repite la ruta de la principal). Si cambia el
 * archivo de la fuente principal la fila actual vuelve a la primera.
 */
export function editSources(change: (sources: readonly DataSource[]) => DataSource[], field = 'sources'): void {
  const cur = variableTextOf(useStore.getState().doc);
  const sources = change(cur.sources);
  const main = sources[0]?.path ?? null;
  editVariableText({ sources, csv_path: main, ...(main !== cur.csv_path ? { csv_row: 0 } : {}) }, field);
}

export const addDataSource = (path: string) => editSources((s) => addSource(s, path), 'sources');
export const replaceDataSource = (index: number, path: string) => editSources((s) => setSourcePath(s, index, path), 'sources');
export const removeDataSource = (index: number) => editSources((s) => removeSource(s, index), 'sources');
export const makeMainSource = (index: number) => editSources((s) => makeMain(s, index), 'sources');
export const setSourceOnShort = (index: number, value: ShortRows) => editSources((s) => setOnShort(s, index, value), 'sources');

/** Rutas que se reescriben en un documento al renombrar una fuente: todas las plantillas que la nombran. */
export function renameSourceInDoc(doc: Document, index: number, wanted: string): Document {
  const cur = variableTextOf(doc);
  const old = cur.sources[index]?.name;
  if (old === undefined) return doc;
  const sources = renameSource(cur.sources, index, wanted);
  const name = sources[index]!.name;
  if (name === old) return doc;
  const fix = (tpl: string) => renameInTemplate(tpl, old, name);
  return {
    ...doc,
    variable_text: { ...cur, sources },
    objects: doc.objects.map((o) => {
      const k = o.kind;
      if (k.type === 'text' && k.text.includes('.')) return { ...o, kind: { ...k, text: fix(k.text) } };
      if (k.type === 'image' && k.variable) return { ...o, kind: { ...k, variable: { ...k.variable, source: fix(k.variable.source) } } };
      if (k.type === 'vector' && k.code) return { ...o, kind: { ...k, code: { ...k.code, data: fix(k.code.data) } } };
      return o;
    }),
  };
}

/** Renombra una fuente con un paso de deshacer, y con ella las plantillas que la usaban. */
export function renameDataSource(index: number, wanted: string): void {
  useStore.getState().applyDocChange('variables.history.edit', (doc) => renameSourceInDoc(doc, index, wanted), 'variables:rename');
}
