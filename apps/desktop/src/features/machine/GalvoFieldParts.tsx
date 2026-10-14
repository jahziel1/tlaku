import { useId, type ReactNode } from 'react';
import { t, type I18nKey } from '@/i18n';

/** Campo con etiqueta, ayuda breve y error (un `I18nKey` de `galvo.err.*`) enlazados al control. */
export function GField({ label, help, error, children }: { label: string; help: I18nKey; error?: I18nKey; children: (ids: { id: string; describedBy: string; invalid: boolean }) => ReactNode }) {
  const id = useId();
  const helpId = `${id}-help`;
  const errId = `${id}-err`;
  return (
    <div className="mc-field" data-invalid={error || undefined}>
      <label htmlFor={id} className="field__label">{label}</label>
      {children({ id, describedBy: error ? `${errId} ${helpId}` : helpId, invalid: !!error })}
      {error && <p id={errId} className="hint hint--danger" role="alert">{t(error)}</p>}
      <p id={helpId} className="hint mc-help">{t(help)}</p>
    </div>
  );
}

export function Check({ label, checked, onChange, disabled }: { label: string; checked: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <label className="mc-check">
      <input type="checkbox" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
      <span>{label}</span>
    </label>
  );
}

/** Subsección con título y una línea de ayuda. */
export function Sub({ title, help, children }: { title: string; help: I18nKey; children: ReactNode }) {
  return (
    <div className="mc-galvo-sub" role="group" aria-label={title}>
      <h4 className="mc-sub">{title}</h4>
      <p className="hint">{t(help)}</p>
      {children}
    </div>
  );
}
