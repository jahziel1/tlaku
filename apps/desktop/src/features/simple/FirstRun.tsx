import { Sparkles } from 'lucide-react';
import { useId } from 'react';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { needsFirstRun, setUiMode } from './mode';

/**
 * Primer arranque: "¿Es tu primera vez con un láser?". Sugiere el modo sencillo; se puede saltar
 * (se queda el modo completo) y el modo se cambia siempre desde la barra superior.
 */
export function FirstRunDialog() {
  const titleId = useId();
  const needsAsk = useStore(needsFirstRun);
  const setPref = useStore((s) => s.setPref);
  if (!needsAsk) return null;
  const skip = () => setPref('uiMode', 'full');
  return (
    <Modal labelId={titleId} onClose={skip} portal backdropClassName="simple-first__backdrop" cardClassName="simple-first" initialFocus="[data-autofocus]">
      <Sparkles size={34} aria-hidden="true" className="simple-first__icon" />
      <h2 id={titleId} className="simple-first__title">{t('simple.first.title')}</h2>
      <p>{t('simple.first.text')}</p>
      <div className="simple-first__buttons">
        <button type="button" className="btn btn--accent simple__btn simple__btn--big" data-autofocus onClick={() => setUiMode('simple')}>
          {t('simple.first.yes')}
        </button>
        <button type="button" className="btn simple__btn simple__btn--big" onClick={() => setUiMode('full')}>
          {t('simple.first.no')}
        </button>
      </div>
      <button type="button" className="simple-first__skip" onClick={skip}>{t('simple.first.skip')}</button>
      <p className="hint">{t('simple.first.later')}</p>
    </Modal>
  );
}
