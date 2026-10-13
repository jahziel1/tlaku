import { useEffect, useId, useState, type ReactNode } from 'react';
import { t } from '@/i18n';
import { useLayerUi } from './layerUi';
import { FIELD_SPECS, formatNumber, parseDecimal, validateField, type FieldContext, type FieldError, type NumericField } from './fields';

/** Ayuda breve bajo un parámetro; se puede ocultar con el interruptor del encabezado. */
function Help({ id, children }: { id: string; children: ReactNode }) {
  const show = useLayerUi((s) => s.showHelp);
  if (!show) return null;
  return (
    <p id={id} className="lyr-help">
      {children}
    </p>
  );
}

interface NumProps {
  field: NumericField;
  label: string;
  value: number;
  /** Se llama solo con valores válidos. */
  onApply: (value: number) => void;
  help?: ReactNode;
  ctx?: FieldContext;
  disabled?: boolean;
  /** Motivo visible por el que está deshabilitado. */
  reason?: string;
  /** Validación extra que depende del documento (por ejemplo, posición ya usada). */
  extra?: (value: number) => FieldError | null;
  /** Texto bajo el campo, siempre visible (por ejemplo la conversión en vivo). */
  live?: ReactNode;
  /** Mostrar el valor en esta unidad en lugar de la del campo. */
  unit?: string;
  /** Sin ayuda propia: la comparte un grupo. */
  compact?: boolean;
  /** Convierte el valor guardado al que se muestra y viceversa (intervalo ⇄ DPI). */
  toView?: (v: number) => number;
}

/**
 * Campo numérico con validación visible. Un valor válido se aplica al escribir (y las flechas
 * suben o bajan un paso: Mayús ×10, Alt ÷10); uno inválido se marca y no se aplica. Esc
 * recupera el valor guardado.
 */
export function NumField({ field, label, value, onApply, help, ctx, disabled, reason, extra, live, unit, compact, toView }: NumProps) {
  const id = useId();
  const spec = FIELD_SPECS[field];
  const shown = toView ? toView(value) : value;
  const format = (v: number) => formatNumber(v, spec.decimals);
  const [text, setText] = useState(format(shown));
  // Si el valor guardado cambia desde fuera (deshacer, otro campo), el texto lo sigue. Mientras el
  // texto ya lo representa (aunque con otro formato, "5." o "33.33") no se toca, para no pisar al usuario.
  useEffect(() => {
    const tol = toView ? Math.abs(shown) * 1e-3 : 0;
    setText((prev) => {
      const p = parseDecimal(prev);
      return p !== null && Math.abs(p - shown) <= tol ? prev : format(shown);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shown]);

  const result = validateField(field, text, ctx);
  const extraErr = result.ok && extra ? extra(result.value) : null;
  const error = !result.ok ? result.error : extraErr;
  const dirty = text !== format(shown);

  const apply = (raw: string) => {
    const r = validateField(field, raw, ctx);
    if (r.ok && !(extra && extra(r.value))) onApply(r.value);
  };
  const onChange = (raw: string) => {
    setText(raw);
    apply(raw);
  };

  const errId = `${id}-err`;
  const helpId = `${id}-help`;
  return (
    <div className="lyr-field" data-invalid={error !== null} data-compact={compact}>
      <div className="lyr-field__top">
        <label htmlFor={id} className="lyr-field__label">
          {label}
        </label>
        <div className="lyr-field__box">
          <input
            id={id}
            className="input num lyr-field__input"
            inputMode="decimal"
            value={text}
            disabled={disabled}
            aria-invalid={error !== null}
            aria-describedby={[error ? errId : '', help ? helpId : ''].filter(Boolean).join(' ') || undefined}
            onChange={(e) => onChange(e.target.value)}
            onBlur={() => {
              // Un texto válido se normaliza; uno inválido se deja a la vista, marcado.
              if (!error) setText(format(shown));
            }}
            onKeyDown={(e) => {
              if (e.key === 'Escape' && dirty) {
                e.stopPropagation();
                setText(format(shown));
              } else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
                e.preventDefault();
                const base = parseDecimal(text) ?? shown;
                const k = e.shiftKey ? 10 : e.altKey ? 0.1 : 1;
                const stepped = base + (e.key === 'ArrowUp' ? 1 : -1) * spec.step * k;
                const clamped = Math.min(spec.max, Math.max(spec.min, stepped));
                onChange(format(clamped));
              }
            }}
          />
          {(unit ?? spec.unit) && <span className="lyr-field__unit">{unit ?? spec.unit}</span>}
        </div>
      </div>
      {error && (
        <p id={errId} className="lyr-err" role="alert">
          {t(error.key, error.params)}
        </p>
      )}
      {disabled && reason && <p className="lyr-live">{reason}</p>}
      {live && <p className="lyr-live num">{live}</p>}
      {help && <Help id={helpId}>{help}</Help>}
    </div>
  );
}

interface ToggleProps {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  help?: ReactNode;
  disabled?: boolean;
  /** Motivo visible por el que está deshabilitado. */
  reason?: string;
}

export function Toggle({ label, checked, onChange, help, disabled, reason }: ToggleProps) {
  const id = useId();
  return (
    <div className="lyr-field">
      <label className="lyr-check" htmlFor={id}>
        <input
          id={id}
          type="checkbox"
          checked={checked}
          disabled={disabled}
          aria-describedby={help ? `${id}-help` : undefined}
          onChange={(e) => onChange(e.target.checked)}
        />
        <span>{label}</span>
      </label>
      {disabled && reason && <p className="lyr-live">{reason}</p>}
      {help && <Help id={`${id}-help`}>{help}</Help>}
    </div>
  );
}

interface SelectProps<T extends string> {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  help?: ReactNode;
  disabled?: boolean;
  /** Motivo visible por el que está deshabilitado. */
  reason?: string;
}

export function SelectField<T extends string>({ label, value, options, onChange, help, disabled, reason }: SelectProps<T>) {
  const id = useId();
  return (
    <div className="lyr-field">
      <div className="lyr-field__top">
        <label htmlFor={id} className="lyr-field__label">
          {label}
        </label>
        <select
          id={id}
          className="select lyr-select"
          value={value}
          disabled={disabled}
          aria-describedby={help ? `${id}-help` : undefined}
          onChange={(e) => onChange(e.target.value as T)}
        >
          {options.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      </div>
      {disabled && reason && <p className="lyr-live">{reason}</p>}
      {help && <Help id={`${id}-help`}>{help}</Help>}
    </div>
  );
}

/**
 * Sección plegable (`<details>`): el contenido siempre está en el documento, así que un lector de
 * pantalla y la búsqueda lo encuentran aunque esté cerrada. Abierta de inicio si trae algo en uso.
 */
export function Fold({ title, badge, defaultOpen = false, children }: { title: string; badge?: string; defaultOpen?: boolean; children: ReactNode }) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <details className="lyr-fold" open={open} onToggle={(e) => setOpen(e.currentTarget.open)}>
      <summary className="lyr-fold__title">
        <span>{title}</span>
        {badge && <span className="lyr-feature">{badge}</span>}
      </summary>
      <div className="lyr-fold__body">{children}</div>
    </details>
  );
}

/** Interruptor que, encendido, muestra sus campos debajo (con sangría). */
export function FeatureBlock({
  label,
  checked,
  onChange,
  help,
  disabled,
  reason,
  children,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  help?: ReactNode;
  disabled?: boolean;
  reason?: string;
  children: ReactNode;
}) {
  return (
    <div className="lyr-block">
      <Toggle label={label} checked={checked} onChange={onChange} help={help} disabled={disabled} reason={reason} />
      {checked && <div className="lyr-block__body">{children}</div>}
    </div>
  );
}

export function Group({ title, children }: { title: string; children: ReactNode }) {
  const id = useId();
  return (
    <section className="lyr-group" aria-labelledby={id}>
      <h4 id={id} className="lyr-group__title">
        {title}
      </h4>
      {children}
    </section>
  );
}
