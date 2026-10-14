import { X } from 'lucide-react';
import type { ReactNode } from 'react';
import { t } from '@/i18n';
import { Modal } from '@/ui/Modal';

/** Marco común: fondo, tarjeta, título y botón de cerrar. */
export function Frame({ id, title, wide, onClose, children, head }: {
  id: string;
  title: string;
  wide?: boolean;
  onClose: () => void;
  children: ReactNode;
  head?: ReactNode;
}) {
  return (
    <Modal
      labelId={id}
      onClose={onClose}
      backdropClassName="help"
      cardClassName={`help__card${wide ? ' help__card--wide' : ''}`}
      initialFocus="[data-autofocus], input"
      closeOnBackdrop
      guardOutside
    >
      <div className="help__head">
        <h2 id={id} className="help__title">{title}</h2>
        {head}
        <button type="button" className="icon-btn" aria-label={t('help.close')} title={t('help.close')} onClick={onClose}>
          <X size={16} aria-hidden="true" />
        </button>
      </div>
      {children}
    </Modal>
  );
}
