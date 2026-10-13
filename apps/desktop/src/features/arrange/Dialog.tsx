import { useId, type ReactNode } from 'react';
import { t } from '@/i18n';
import { Modal } from '@/ui/Modal';
import './arrange.css';

interface Props {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer: ReactNode;
  /** Selector del elemento que recibe el foco al abrir. */
  initialFocus?: string;
  wide?: boolean;
}

/** Marco común de los diálogos de Organizar: título, cuerpo con formulario y pie con acciones. */
export function Dialog({ title, onClose, children, footer, initialFocus, wide }: Props) {
  const labelId = useId();
  return (
    <Modal
      labelId={labelId}
      onClose={onClose}
      backdropClassName="modal"
      cardClassName={`modal__card${wide ? ' modal__card--wide' : ''}`}
      initialFocus={initialFocus}
      guardOutside
    >
      <div className="modal__head">
        <h2 id={labelId} className="modal__title">{title}</h2>
      </div>
      <div className="modal__body ar-body">{children}</div>
      <div className="modal__foot">{footer}</div>
    </Modal>
  );
}

/** Pie estándar: Cancelar y la acción principal; si no se puede aplicar, el motivo se ve al lado. */
export function Footer({ onClose, onApply, disabled, reason, applyLabel }: { onClose: () => void; onApply: () => void; disabled: boolean; reason?: string | null; applyLabel?: string }) {
  return (
    <>
      {disabled && reason && <p className="ar-reason" role="status">{reason}</p>}
      <button type="button" className="btn" onClick={onClose}>{t('arrange.dlg.cancel')}</button>
      <button type="button" className="btn btn--accent" data-autofocus aria-disabled={disabled} title={disabled ? (reason ?? undefined) : undefined} onClick={() => !disabled && onApply()}>
        {applyLabel ?? t('arrange.dlg.apply')}
      </button>
    </>
  );
}

/** Grupo de opciones excluyentes con botones tipo "chip" (radio accesible). */
export function Choice<T extends string>({ legend, value, options, onChange }: { legend: string; value: T; options: readonly { value: T; label: string }[]; onChange: (v: T) => void }) {
  return (
    <div className="ar-choice" role="radiogroup" aria-label={legend}>
      <span className="field__label">{legend}</span>
      <div className="ar-choice__row">
        {options.map((o) => (
          <button key={o.value} type="button" role="radio" aria-checked={o.value === value} className="chip" onClick={() => onChange(o.value)}>
            {o.label}
          </button>
        ))}
      </div>
    </div>
  );
}

export function Check({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="ar-check">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>{label}</span>
    </label>
  );
}
