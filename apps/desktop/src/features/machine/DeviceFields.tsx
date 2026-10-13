import { useEffect, useId, useState, type ReactNode } from 'react';
import { t, type I18nKey } from '@/i18n';
import type { DeviceIssue, DialectIssue } from '@/lib/device';

/** Texto de un error: los de las plantillas del dialecto viven en `devices.dialect.err`, el resto en `machine.dev.err`. */
const DIALECT_ISSUES: ReadonlySet<string> = new Set<DialectIssue>(['empty', 'multiline', 'missingS', 'badPlaceholder', 'range', 'lineTooShort', 'airPair']);
function issueText(error: DeviceIssue | DialectIssue): string {
  return DIALECT_ISSUES.has(error) ? t(`devices.dialect.err.${error as DialectIssue}`) : t(`machine.dev.err.${error as DeviceIssue}`);
}

/** Campo con etiqueta, ayuda breve y mensaje de error enlazados al control con `aria-describedby`. */
export function Field({
  label,
  help,
  error,
  children,
}: {
  label: string;
  help: I18nKey;
  error?: DeviceIssue | DialectIssue;
  children: (ids: { id: string; describedBy: string; invalid: boolean }) => ReactNode;
}) {
  const id = useId();
  const helpId = `${id}-help`;
  const errId = `${id}-err`;
  return (
    <div className="mc-field" data-invalid={error || undefined}>
      <label htmlFor={id} className="field__label">{label}</label>
      {children({ id, describedBy: error ? `${errId} ${helpId}` : helpId, invalid: !!error })}
      {error && <p id={errId} className="hint hint--danger" role="alert">{issueText(error)}</p>}
      <p id={helpId} className="hint mc-help">{t(help)}</p>
    </div>
  );
}

const parse = (text: string) => Number(text.trim().replace(',', '.'));

/** Número que se confirma al salir del campo o con Intro; Esc lo revierte. Acepta coma decimal. */
export function NumInput({
  id,
  value,
  onCommit,
  unit,
  label,
  describedBy,
  invalid,
  readOnly,
}: {
  id?: string;
  value: number;
  onCommit: (v: number) => void;
  unit?: string;
  label?: string;
  describedBy?: string;
  invalid?: boolean;
  readOnly?: boolean;
}) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  const commit = () => {
    const n = parse(text);
    if (text.trim() === '' || !Number.isFinite(n)) return setText(String(value));
    if (n !== value) onCommit(n);
  };
  return (
    <div className="field__box">
      <input
        id={id}
        className="field__input num"
        inputMode="decimal"
        aria-label={label}
        aria-describedby={describedBy}
        aria-invalid={invalid || undefined}
        readOnly={readOnly}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            commit();
          } else if (e.key === 'Escape' && text !== String(value)) {
            e.stopPropagation();
            setText(String(value));
          }
        }}
      />
      {unit && <span className="field__unit">{unit}</span>}
    </div>
  );
}
