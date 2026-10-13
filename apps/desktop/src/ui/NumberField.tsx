import { useEffect, useId, useState } from 'react';

interface Props {
  label: string;
  value: number;
  onCommit: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  unit?: string;
  disabled?: boolean;
  /** Lee lo escrito y devuelve el valor, o `null` si no se entiende. Por omisión, un número con coma o punto. */
  parse?: (text: string) => number | null;
  /** Texto de un valor. Por omisión, el número tal cual. Debe ser estable (`useCallback`) entre renderizados. */
  format?: (value: number) => string;
}

const plain = (text: string): number | null => {
  const n = Number(text.replace(',', '.'));
  return Number.isFinite(n) ? n : null;
};

/** Campo numérico que confirma con Enter o al salir; Escape lo revierte. */
export function NumberField({ label, value, onCommit, min, max, step = 1, unit, disabled, parse = plain, format }: Props) {
  const id = useId();
  const show = (v: number) => (format ? format(v) : String(v));
  const [text, setText] = useState(show(value));
  // eslint-disable-next-line react-hooks/exhaustive-deps -- `show` solo cambia con `format`
  useEffect(() => setText(show(value)), [value, format]);

  const commit = () => {
    const n = parse(text);
    if (n === null) return setText(show(value));
    const clamped = Math.min(max ?? Infinity, Math.max(min ?? -Infinity, n));
    setText(show(clamped));
    if (clamped !== value) onCommit(clamped);
  };

  return (
    <div className="field">
      <label htmlFor={id} className="field__label">{label}</label>
      <div className="field__box">
        <input
          id={id}
          className="field__input num"
          inputMode="decimal"
          value={text}
          disabled={disabled}
          step={step}
          onChange={(e) => setText(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === 'Enter') commit();
            else if (e.key === 'Escape') setText(show(value));
            else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
              e.preventDefault();
              const n = parse(text) ?? 0;
              const next = Math.min(max ?? Infinity, Math.max(min ?? -Infinity, n + (e.key === 'ArrowUp' ? step : -step)));
              setText(format ? format(next) : String(Math.round(next * 1000) / 1000));
            }
          }}
        />
        {unit && <span className="field__unit">{unit}</span>}
      </div>
    </div>
  );
}
