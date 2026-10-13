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
}

/** Campo numérico que confirma con Enter o al salir; Escape lo revierte. */
export function NumberField({ label, value, onCommit, min, max, step = 1, unit, disabled }: Props) {
  const id = useId();
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);

  const commit = () => {
    const n = Number(text.replace(',', '.'));
    if (!Number.isFinite(n)) return setText(String(value));
    const clamped = Math.min(max ?? Infinity, Math.max(min ?? -Infinity, n));
    setText(String(clamped));
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
            else if (e.key === 'Escape') setText(String(value));
            else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
              e.preventDefault();
              const n = Number(text.replace(',', '.')) || 0;
              const next = Math.min(max ?? Infinity, Math.max(min ?? -Infinity, n + (e.key === 'ArrowUp' ? step : -step)));
              setText(String(Math.round(next * 1000) / 1000));
            }
          }}
        />
        {unit && <span className="field__unit">{unit}</span>}
      </div>
    </div>
  );
}
