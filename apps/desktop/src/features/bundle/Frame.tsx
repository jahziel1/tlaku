import { X } from 'lucide-react';
import { useId } from 'react';
import { t } from '@/i18n';
import { Modal } from '@/ui/Modal';

/** Marco común: cabecera con título y cierre, cuerpo y pie. */
export function Frame({ title, onClose, children, foot }: { title: string; onClose: () => void; children: React.ReactNode; foot: React.ReactNode }) {
  const titleId = useId();
  return (
    <Modal labelId={titleId} onClose={onClose} backdropClassName="modal" cardClassName="modal__card bnd-card" initialFocus="[data-autofocus]" fallbackFocus="first" guardOutside>
      <div className="modal__head">
        <h2 id={titleId} className="modal__title">{title}</h2>
        <button type="button" className="icon-btn" aria-label={t('bundle.close')} title={t('bundle.close')} onClick={onClose}>
          <X size={16} aria-hidden="true" />
        </button>
      </div>
      {children}
      <div className="modal__foot bnd-foot">{foot}</div>
    </Modal>
  );
}
