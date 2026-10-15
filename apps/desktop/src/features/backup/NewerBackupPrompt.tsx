import { useId } from 'react';
import { useBundleUi } from '@/features/bundle/uiStore';
import { t, locale } from '@/i18n';
import { Modal } from '@/ui/Modal';
import { markBackupSeen } from './actions';
import { useBackup } from './store';

/**
 * Al abrir la aplicación: hay en la carpeta de respaldo uno más nuevo que lo de esta computadora, hecho
 * en otra. Se ofrece revisarlo con la vista previa de siempre; nada se aplica solo. «Ahora no» lo deja
 * para después (el diálogo de respaldo lo sigue mostrando) y «Omitir» lo da por visto.
 */
export function NewerBackupPrompt() {
  const newer = useBackup((s) => s.newer);
  const open = useBackup((s) => s.promptOpen);
  const openImportFrom = useBundleUi((s) => s.openImportFrom);
  const titleId = useId();
  if (!newer || !open) return null;
  const close = () => useBackup.setState({ promptOpen: false });
  const date = new Date(newer.created_ms).toLocaleString(locale());
  const review = () => {
    close();
    openImportFrom({ path: newer.path, createdMs: newer.created_ms });
  };
  const skip = () => {
    close();
    void markBackupSeen(newer).catch(() => undefined);
  };
  return (
    <Modal labelId={titleId} onClose={close} backdropClassName="modal" cardClassName="modal__card bnd-card" initialFocus="[data-autofocus]" fallbackFocus="first" guardOutside>
      <div className="modal__head">
        <h2 id={titleId} className="modal__title">{t('bundle.backup.newerTitle')}</h2>
      </div>
      <div className="bnd-body">
        <p>{t('bundle.backup.newerBody', { date })}</p>
        <p className="hint">{t('bundle.backup.newerHint')}</p>
      </div>
      <div className="modal__foot bnd-foot">
        <button type="button" className="btn" onClick={skip}>{t('bundle.backup.newerSkip')}</button>
        <button type="button" className="btn" onClick={close}>{t('bundle.backup.newerLater')}</button>
        <button type="button" className="btn btn--accent" data-autofocus onClick={review}>{t('bundle.backup.review')}</button>
      </div>
    </Modal>
  );
}
