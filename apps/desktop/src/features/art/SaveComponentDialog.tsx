import { useId, useState } from 'react';
import { t } from '@/i18n';
import { Modal } from '@/ui/Modal';
import { useArtPanel } from './artStore';
import { saveSelectionAsComponent } from './componentActions';
import { MAX_COMPONENT_NAME, cleanName } from './components';

/** Un nombre que no choca con los que ya hay: `Componente`, `Componente 2`... */
function suggestName(taken: readonly string[]): string {
  const base = t('art.component.dialog.defaultName');
  const have = new Set(taken.map((n) => n.toLowerCase()));
  if (!have.has(base.toLowerCase())) return base;
  for (let k = 2; ; k++) if (!have.has(`${base} ${k}`.toLowerCase())) return `${base} ${k}`;
}

function Dialog({ ids }: { ids: string[] }) {
  const labelId = useId();
  const nameId = useId();
  const components = useArtPanel((s) => s.components);
  const setSaving = useArtPanel((s) => s.setSaving);
  const [name, setName] = useState(() => suggestName(components.map((c) => c.name)));
  const [busy, setBusy] = useState(false);
  const clean = cleanName(name);
  const exists = components.some((c) => c.name.toLowerCase() === clean.toLowerCase());
  const close = () => setSaving(null);

  const save = async () => {
    if (!clean || busy) return;
    setBusy(true);
    const saved = await saveSelectionAsComponent(clean, ids);
    setBusy(false);
    if (saved) close();
  };

  return (
    <Modal labelId={labelId} onClose={close} backdropClassName="modal" cardClassName="modal__card" portal guardOutside initialFocus={`#${CSS.escape(nameId)}`}>
      <div className="modal__head">
        <h2 id={labelId} className="modal__title">{t('art.component.dialog.title')}</h2>
      </div>
      <div className="modal__body art-save">
        <p className="hint">{t('art.component.dialog.hint', { n: ids.length })}</p>
        <div className="field">
          <label htmlFor={nameId} className="field__label">{t('art.component.dialog.name')}</label>
          <input
            id={nameId}
            className="input"
            value={name}
            maxLength={MAX_COMPONENT_NAME}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void save();
            }}
          />
        </div>
        {exists && <p className="hint">{t('art.component.dialog.exists')}</p>}
      </div>
      <div className="modal__foot">
        <button type="button" className="btn" onClick={close}>{t('art.component.dialog.cancel')}</button>
        <button type="button" className="btn btn--accent" disabled={!clean || busy} onClick={() => void save()}>
          {t('art.component.dialog.save')}
        </button>
      </div>
    </Modal>
  );
}

/** Monta el diálogo «Guardar como componente» cuando está abierto (clic derecho o menú de la biblioteca). */
export function SaveComponentHost() {
  const ids = useArtPanel((s) => s.saving);
  return ids ? <Dialog ids={ids} /> : null;
}
