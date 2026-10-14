import { useEffect, useId, useState, type ReactNode } from 'react';
import { t, type I18nKey } from '@/i18n';
import { fieldText, formatLength, parseLength, stepInMm, unitSymbol } from '@/lib/units';
import { stepFromKey } from '@/ui/stepKey';
import { usePref, useStore, PREF_SPECS, type PrefKey, type Prefs } from '@/store';

type Keys<V> = { [K in PrefKey]: Prefs[K] extends V ? K : never }[PrefKey];

/** Casilla de una preferencia booleana. */
export function PrefCheck({ k, label }: { k: Keys<boolean>; label: I18nKey }) {
  const value = usePref(k);
  const setPref = useStore((s) => s.setPref);
  return (
    <label className="set-check">
      <input type="checkbox" checked={value} onChange={(e) => setPref(k, e.target.checked as never)} />
      {t(label)}
    </label>
  );
}

export interface ChoiceOption {
  value: string;
  label: string;
  disabled?: boolean;
}

/** Lista desplegable de una preferencia de opciones. */
export function PrefSelect({ k, label, options, hint }: { k: Keys<string>; label: I18nKey; options: ChoiceOption[]; hint?: ReactNode }) {
  const id = useId();
  const value = usePref(k);
  const setPref = useStore((s) => s.setPref);
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>{t(label)}</label>
      <select id={id} className="select" value={value} onChange={(e) => setPref(k, e.target.value as never)}>
        {options.map((o) => (
          <option key={o.value} value={o.value} disabled={o.disabled}>{o.label}</option>
        ))}
      </select>
      {hint && <p className="hint">{hint}</p>}
    </div>
  );
}

/** Campo numérico de una preferencia (grados, píxeles, segundos...). Confirma con Enter o al salir; Esc revierte. */
export function PrefNumber({ k, label, unit }: { k: Keys<number>; label: I18nKey; unit: I18nKey }) {
  const id = useId();
  const value = usePref(k);
  const setPref = useStore((s) => s.setPref);
  const spec = PREF_SPECS[k];
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);

  const commit = () => {
    const n = Number(text.replace(',', '.'));
    if (!text.trim() || !Number.isFinite(n)) return setText(String(value));
    setPref(k, n as never);
    setText(String(useStore.getState().prefs[k]));
  };
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>{t(label)}</label>
      <div className="field__box">
        <input
          id={id}
          className="field__input num"
          inputMode="decimal"
          value={text}
          min={'min' in spec ? spec.min : undefined}
          onChange={(e) => setText(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === 'Enter') commit();
            else if (e.key === 'Escape' && text !== String(value)) {
              e.stopPropagation(); // revierte el campo sin cerrar la ventana
              setText(String(value));
            } else {
              const n = Number(text.replace(',', '.'));
              const next = stepFromKey(
                e,
                text.trim() && Number.isFinite(n) ? n : value,
                'step' in spec ? spec.step : 1,
                'min' in spec ? spec.min : undefined,
                'max' in spec ? spec.max : undefined,
              );
              if (next === null) return;
              e.preventDefault();
              setText(String(next));
            }
          }}
        />
        <span className="field__unit">{t(unit)}</span>
      </div>
    </div>
  );
}

/**
 * Longitud (en mm en el estado) mostrada y escrita en la unidad elegida en las preferencias.
 * Acepta sufijos al escribir (`5 mm`, `0,25 in`, `1/8"`), vía `parseLength`.
 */
export function PrefLength({ k, label }: { k: Keys<number>; label: I18nKey }) {
  const id = useId();
  const value = usePref(k);
  const units = usePref('units');
  const setPref = useStore((s) => s.setPref);
  const shown = formatLength(value, units, { symbol: false, trim: true });
  const [text, setText] = useState(shown);
  useEffect(() => setText(shown), [shown]);

  const commit = () => {
    if (text === shown) return;
    const mm = parseLength(text, units);
    if (mm === null) return setText(shown);
    setPref(k, mm as never);
    setText(formatLength(useStore.getState().prefs[k], units, { symbol: false, trim: true }));
  };
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>{t(label)}</label>
      <div className="field__box">
        <input
          id={id}
          className="field__input num"
          inputMode="decimal"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === 'Enter') commit();
            else if (e.key === 'Escape' && text !== shown) {
              e.stopPropagation();
              setText(shown);
            } else {
              const spec = PREF_SPECS[k];
              const cur = parseLength(text, units) ?? value;
              const next = stepFromKey(
                e,
                cur,
                stepInMm('step' in spec ? spec.step : 1, units),
                'min' in spec ? spec.min : undefined,
                'max' in spec ? spec.max : undefined,
              );
              if (next === null) return;
              e.preventDefault();
              setText(fieldText(next, units));
            }
          }}
        />
        <span className="field__unit">{unitSymbol(units)}</span>
      </div>
    </div>
  );
}
