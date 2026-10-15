import { useId, useMemo } from 'react';
import type { DataSource } from '@/api/types';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { useVariablesUi, type CsvState } from './context';
import { variableTextOf } from './defaults';
import { columnSnippet } from './insert';

export interface ColumnChoice {
  id: string;
  /** Texto que se ve: la columna, con el nombre de la fuente si no es la principal. */
  label: string;
  /** Lo que se inserta: `{csv:Columna}` o `{csv.fuente:Columna}`. */
  snippet: string;
}

/** Columnas de todas las fuentes ya leídas, la principal primero. */
export function columnChoices(sources: readonly DataSource[], main: readonly string[] | undefined, extra: Record<string, CsvState>): ColumnChoice[] {
  const out: ColumnChoice[] = [];
  (main ?? []).forEach((h, i) => out.push({ id: `0:${i}`, label: h.trim() === '' ? String(i + 1) : h, snippet: columnSnippet(h, i) }));
  sources.slice(1).forEach((s, si) => {
    extra[s.name.toLowerCase()]?.table?.headers.forEach((h, i) => out.push({ id: `${si + 1}:${i}`, label: `${s.name}: ${h.trim() === '' ? i + 1 : h}`, snippet: columnSnippet(h, i, s.name) }));
  });
  return out;
}

/** Las columnas disponibles ahora, siguiendo al documento y a los CSV cargados. */
export function useColumnChoices(): ColumnChoice[] {
  const sources = useStore((s) => variableTextOf(s.doc).sources);
  const main = useVariablesUi((s) => s.csv.table?.headers);
  const extra = useVariablesUi((s) => s.extra);
  return useMemo(() => columnChoices(sources, main, extra), [sources, main, extra]);
}

/** Lista desplegable de columnas; al elegir una llama a `onPick` con su plantilla. No se dibuja si no hay columnas. */
export function ColumnPicker({ label, placeholder, onPick }: { label: string; placeholder?: string; onPick: (snippet: string) => void }) {
  const id = useId();
  const choices = useColumnChoices();
  if (choices.length === 0) return null;
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>{label}</label>
      <select
        id={id}
        className="select"
        value=""
        onChange={(e) => {
          const pick = choices.find((c) => c.id === e.target.value);
          if (pick) onPick(pick.snippet);
        }}
      >
        <option value="" disabled>{placeholder ?? t('variables.column.placeholder')}</option>
        {choices.map((c) => (
          <option key={c.id} value={c.id}>{c.label}  {c.snippet}</option>
        ))}
      </select>
    </div>
  );
}
