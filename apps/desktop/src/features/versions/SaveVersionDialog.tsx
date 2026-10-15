import { useId, useState } from 'react';
import { t } from '@/i18n';
import { Modal } from '@/ui/Modal';
import { saveNamedVersion } from './actions';
import { useVersionsUi } from './versionsUi';

/** «Guardar versión…»: pide el nombre y guarda el documento de la pestaña activa tal como está. */
export function SaveVersionDialog() {
  const close = useVersionsUi((s) => s.closeName);
  const titleId = useId();
  const inputId = useId();
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const valid = name.trim().length > 0;

  const submit = async () => {
    if (!valid || busy) return;
    setBusy(true);
    const ok = await saveNamedVersion(name);
    setBusy(false);
    if (ok) close();
  };

  return (
    <Modal labelId={titleId} onClose={close} backdropClassName="modal" cardClassName="modal__card" initialFocus="[data-autofocus]" guardOutside>
      <div className="modal__head">
        <h2 id={titleId} className="modal__title">
          {t('versions.name.title')}
        </h2>
      </div>
      <form
        className="ver-name"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <div className="field">
          <label htmlFor={inputId} className="field__label">
            {t('versions.name.label')}
          </label>
          <input
            id={inputId}
            className="input"
            data-autofocus=""
            value={name}
            maxLength={120}
            placeholder={t('versions.name.placeholder')}
            onChange={(e) => setName(e.target.value)}
          />
        </div>
        <p className="hint">{t('versions.name.hint')}</p>
        <div className="modal__foot ver-name__foot">
          <button type="button" className="btn" onClick={close}>
            {t('versions.name.cancel')}
          </button>
          <button type="submit" className="btn btn--accent" disabled={!valid || busy}>
            {t('versions.name.save')}
          </button>
        </div>
      </form>
    </Modal>
  );
}
