import { useId, useRef, useState } from 'react';
import { t } from '@/i18n';
import { formatValue, resolveInput } from './expr';

interface Props {
  label: string;
  /** Valor común, `null` si ninguno lo fija (automático) o `'mixed'` si la selección difiere. */
  value: number | null | 'mixed';
  /** Se muestra cuando el valor es `null` (por ejemplo "Auto" o "100"). */
  placeholder: string;
  onApply: (value: number | null) => void;
  unit?: string;
  title?: string;
}

/**
 * Campo numérico opcional: vacío = automático (`null`). Con selección mixta muestra "Varios".
 * Acepta las mismas expresiones que los demás campos (`+1`, `*2`...); Intro o salir confirman,
 * Escape revierte y las flechas suman 1 (Mayús 10).
 */
export function OptionalField({ label, value, placeholder, onApply, unit, title }: Props) {
  const id = useId();
  const [text, setText] = useState<string | null>(null);
  const [invalid, setInvalid] = useState(false);
  const pending = useRef<string | null>(null);
  const edit = (v: string | null) => {
    pending.current = v;
    setText(v);
  };
  const base = typeof value === 'number' ? value : 0;

  const commit = () => {
    const typed = pending.current;
    if (typed === null) return;
    edit(null);
    if (typed.trim() === '') {
      setInvalid(false);
      if (value !== null) onApply(null);
      return;
    }
    const v = resolveInput(typed, base, 'length');
    if (v === null) {
      setInvalid(true);
      return;
    }
    setInvalid(false);
    if (v !== value) onApply(v);
  };

  return (
    <div className="nbar-field" title={invalid ? t('canvas.bar.invalid') : title}>
      <label htmlFor={id} className="nbar-field__label">{label}</label>
      <div className="nbar-field__box">
        <input
          id={id}
          className="nbar-field__input num"
          inputMode="decimal"
          autoComplete="off"
          spellCheck={false}
          value={text ?? (typeof value === 'number' ? formatValue(value) : '')}
          placeholder={value === 'mixed' ? t('canvas.obj.mixed') : placeholder}
          aria-invalid={invalid || undefined}
          onFocus={(e) => {
            setInvalid(false);
            e.currentTarget.select();
          }}
          onChange={(e) => {
            edit(e.target.value);
            setInvalid(false);
          }}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              commit();
              e.currentTarget.blur();
            } else if (e.key === 'Escape') {
              edit(null);
              setInvalid(false);
              e.currentTarget.blur();
              e.stopPropagation();
            } else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
              e.preventDefault();
              const cur = pending.current === null ? base : (resolveInput(pending.current, base, 'length') ?? base);
              edit(null);
              onApply(cur + (e.key === 'ArrowUp' ? 1 : -1) * (e.shiftKey ? 10 : 1));
            }
          }}
        />
        {unit && <span className="nbar-field__unit" aria-hidden="true">{unit}</span>}
      </div>
    </div>
  );
}
