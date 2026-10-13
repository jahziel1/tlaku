import { X } from 'lucide-react';
import { useId, type ReactNode } from 'react';
import { t } from '@/i18n';
import { Modal as BaseModal } from '@/ui/Modal';

interface Props {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  /** `lg` para el gestor de dispositivos. */
  size?: 'sm' | 'md' | 'lg';
  /** Color de la franja superior: peligro, aviso o neutro. */
  tone?: 'danger' | 'warn' | 'accent';
  /** Subtítulo corto bajo el título. */
  subtitle?: string;
}

/**
 * Marco de los diálogos de Máquina (cabecera, cuerpo y pie). El comportamiento accesible (foco
 * atrapado, Esc, foco inicial en `data-autofocus` y retorno del foco) es el común de `@/ui/Modal`.
 * Los atajos de la ventana no llegan mientras está abierto (así una `F` o `Mod+Enter` no mueve la
 * máquina por detrás), salvo Detener (`Mod+.`): un paro no se bloquea.
 */
export function Modal({ title, subtitle, onClose, children, footer, size = 'md', tone }: Props) {
  const titleId = useId();
  return (
    <BaseModal
      labelId={titleId}
      onClose={onClose}
      portal
      closeOnBackdrop
      passKey={(e) => (e.metaKey || e.ctrlKey) && e.key === '.'}
      backdropClassName="mc-backdrop"
      cardClassName="mc-modal"
      cardData={{ 'data-size': size, 'data-tone': tone }}
    >
      <header className="mc-modal__head">
        <div>
          <h2 id={titleId} className="mc-modal__title">{title}</h2>
          {subtitle && <p className="hint">{subtitle}</p>}
        </div>
        <button type="button" className="icon-btn" title={t('machine.dlg.close')} aria-label={t('machine.dlg.close')} onClick={onClose}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>
      <div className="mc-modal__body">{children}</div>
      {footer && <footer className="mc-modal__foot">{footer}</footer>}
    </BaseModal>
  );
}
