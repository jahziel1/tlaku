import { useId, useMemo, type RefObject } from 'react';
import { t, getLanguage } from '@/i18n';
import { useStore } from '@/store';
import { variableTextOf } from './defaults';
import { useVariablesUi } from './context';
import { insertOptions, insertSnippet } from './insert';

interface Props {
  /** Texto actual, para insertar donde está el cursor. */
  value: string;
  areaRef: RefObject<HTMLTextAreaElement | null>;
  onChange: (text: string) => void;
}

/** Menú "Insertar variable" del panel de texto: pone la variable elegida donde está el cursor. */
export function InsertVariable({ value, areaRef, onChange }: Props) {
  const id = useId();
  const headers = useVariablesUi((s) => s.csv.table?.headers);
  const extra = useVariablesUi((s) => s.extra);
  const sources = useStore((s) => variableTextOf(s.doc).sources);
  const lang = getLanguage();
  const options = useMemo(() => {
    // Las fuentes secundarias con su nombre del documento y los encabezados ya leídos.
    const named = sources.slice(1).flatMap((s) => {
      const st = extra[s.name.toLowerCase()];
      return st?.table && st.path === s.path ? [{ name: s.name, headers: st.table.headers }] : [];
    });
    return insertOptions(lang, headers ?? [], named);
  }, [lang, headers, extra, sources]);
  const groups: { key: 'serial' | 'csv' | 'time' | 'other'; label: Parameters<typeof t>[0] }[] = [
    { key: 'serial', label: 'variables.insert.group.serial' },
    { key: 'csv', label: 'variables.insert.group.csv' },
    { key: 'time', label: 'variables.insert.group.time' },
    { key: 'other', label: 'variables.insert.group.other' },
  ];

  const pick = (optionId: string) => {
    const opt = options.find((o) => o.id === optionId);
    if (!opt) return;
    const el = areaRef.current;
    const start = el?.selectionStart ?? value.length;
    const end = el?.selectionEnd ?? value.length;
    const r = insertSnippet(value, start, end, opt.snippet);
    onChange(r.text);
    // El cursor queda después de lo insertado cuando React ya pintó el texto nuevo.
    requestAnimationFrame(() => {
      el?.focus({ preventScroll: true });
      el?.setSelectionRange(r.caret, r.caret);
    });
  };

  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>{t('variables.insert.label')}</label>
      <select id={id} className="select" value="" onChange={(e) => pick(e.target.value)}>
        <option value="" disabled>{t('variables.insert.placeholder')}</option>
        {groups.map((g) => (
          <optgroup key={g.key} label={t(g.label)}>
            {options
              .filter((o) => o.group === g.key)
              .map((o) => (
                <option key={o.id} value={o.id}>
                  {t(o.label, { column: o.column ?? '', source: o.source ?? '' })}  {o.snippet}
                </option>
              ))}
          </optgroup>
        ))}
      </select>
    </div>
  );
}
