import { useId, useRef, useState } from 'react';
import { t } from '@/i18n';
import { fromDisplay, toDisplay, unitSymbol, type LengthUnit } from '@/lib/units';
import { usePref } from '@/store';
import { formatValue, resolveInput, type ExprMode } from './expr';

interface Props {
  label: string;
  value: number;
  onApply: (value: number) => void;
  mode?: ExprMode;
  /**
   * Etiqueta de la unidad. Con `'mm'` (y modo longitud) el campo es una longitud: `value` va en mm
   * pero se muestra y se escribe en la unidad de las preferencias (mm o pulgadas).
   */
  unit?: string;
  disabled?: boolean;
  /** Texto de ayuda o motivo de deshabilitado. */
  title?: string;
}

/**
 * Campo numérico con expresiones (`100/3`, `+5`, `*2`, `50%`, `5in`; en pulgadas, también `3/8"`). Confirma con Enter (y suelta el foco) o al salir
 * (Tab); Escape revierte; las flechas suman 1 (Mayús 10, Alt 0,1) y aplican al momento.
 */
export function ExprField({ label, value, onApply, mode = 'length', unit, disabled, title }: Props) {
  const id = useId();
  const prefUnit = usePref('units');
  const isLength = mode === 'length' && unit === 'mm';
  // Unidad en que se muestra y se escribe; los demás campos (grados, %, conteos) no cambian.
  const shownUnit: LengthUnit = isLength ? prefUnit : 'mm';
  const shown = (mm: number) => formatValue(toDisplay(mm, shownUnit), shownUnit === 'in' ? 4 : 3);
  // `null` = no se está editando: se muestra el valor del documento tal cual.
  const [text, setText] = useState<string | null>(null);
  const [invalid, setInvalid] = useState(false);
  // Copia síncrona del texto: Intro confirma y suelta el foco, y el `onBlur` posterior no debe confirmar de nuevo.
  const pending = useRef<string | null>(null);
  const edit = (v: string | null) => {
    pending.current = v;
    setText(v);
  };

  const commit = () => {
    const typed = pending.current;
    if (typed === null) return;
    edit(null);
    const v = resolveInput(typed, value, mode, shownUnit);
    if (v === null) {
      setInvalid(true);
      return;
    }
    setInvalid(false);
    if (Math.abs(v - value) > 1e-9) onApply(v);
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
          value={text ?? shown(value)}
          disabled={disabled}
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
              // Tras aplicar se suelta el foco: así Ctrl/Cmd+Z y las teclas de herramienta vuelven a actuar.
              commit();
              e.currentTarget.blur();
            } else if (e.key === 'Escape') {
              edit(null);
              setInvalid(false);
              e.currentTarget.blur();
              e.stopPropagation();
            } else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
              e.preventDefault();
              const base = pending.current === null ? value : (resolveInput(pending.current, value, mode, shownUnit) ?? value);
              // Pasos en la unidad que se ve: 1 / 10 / 0,1 mm, o 0,1 / 1 / 0,01 in.
              const step = fromDisplay((e.shiftKey ? 10 : e.altKey ? 0.1 : 1) * (shownUnit === 'in' ? 0.1 : 1), shownUnit);
              edit(null);
              onApply(base + (e.key === 'ArrowUp' ? step : -step));
            }
          }}
        />
        {unit && <span className="nbar-field__unit" aria-hidden="true">{isLength ? unitSymbol(shownUnit) : unit}</span>}
      </div>
    </div>
  );
}
