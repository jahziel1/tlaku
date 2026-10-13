import { Trash2 } from 'lucide-react';
import { useEffect, useId, useRef } from 'react';
import type { GuideAxis } from '@/api/types';
import { t } from '@/i18n';
import { selectActiveDevice, useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { ExprField } from './ExprField';
import { useGuideUi } from './guides';
import { useInteraction } from './interaction';

/** Diálogo de guías: posición exacta de cada una, agregar de cada eje y borrar. */
function GuidesDialog() {
  const guides = useStore((s) => s.doc.guides) ?? [];
  const bed = useStore((s) => selectActiveDevice(s).bed);
  const focus = useGuideUi((s) => s.focus);
  const close = useGuideUi((s) => s.close);
  const labelId = useId();
  const rows = useRef<(HTMLLIElement | null)[]>([]);

  // Al abrir con doble clic sobre una guía, su fila queda a la vista y resaltada.
  useEffect(() => {
    if (focus !== null) rows.current[focus]?.scrollIntoView?.({ block: 'nearest' });
  }, [focus]);
  // Mientras el diálogo está abierto, la guía enfocada se resalta en el lienzo.
  useEffect(() => {
    useInteraction.setState({ guideHover: focus });
    return () => useInteraction.setState({ guideHover: null });
  }, [focus]);

  const add = (axis: GuideAxis) => {
    const s = useStore.getState();
    if (!s.showGuides) s.toggleGuides();
    s.addGuide({ axis, pos: axis === 'x' ? bed[0] / 2 : bed[1] / 2 });
  };

  return (
    <Modal labelId={labelId} onClose={close} backdropClassName="modal" cardClassName="modal__card" portal guardOutside initialFocus="[data-autofocus]">
      <div className="modal__head">
        <h2 id={labelId} className="modal__title">{t('canvas.guide.title')}</h2>
      </div>
      <div className="modal__body guides-dlg">
        <p className="hint">{t('canvas.guide.hint')}</p>
        {guides.length === 0 ? (
          <p className="guides-dlg__empty">{t('canvas.guide.empty')}</p>
        ) : (
          <ul className="guides-dlg__list">
            {guides.map((g, i) => (
              <li key={i} ref={(el) => { rows.current[i] = el; }} className="guides-dlg__row" data-focus={focus === i || undefined}>
                <span className="guides-dlg__axis">{t(g.axis === 'x' ? 'canvas.guide.vertical' : 'canvas.guide.horizontal')}</span>
                <ExprField
                  label={t(g.axis === 'x' ? 'canvas.guide.posX' : 'canvas.guide.posY')}
                  value={g.pos}
                  unit="mm"
                  onApply={(pos) => useStore.getState().updateGuide(i, { pos: Math.round(pos * 1000) / 1000 })}
                />
                <button type="button" className="icon-btn" title={t('canvas.guide.remove')} aria-label={t('canvas.guide.remove')} onClick={() => useStore.getState().removeGuide(i)}>
                  <Trash2 size={15} aria-hidden="true" />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      <div className="modal__foot">
        <button type="button" className="btn" onClick={() => add('x')}>{t('canvas.guide.addVertical')}</button>
        <button type="button" className="btn" onClick={() => add('y')}>{t('canvas.guide.addHorizontal')}</button>
        <button type="button" className="btn" disabled={guides.length === 0} onClick={() => useStore.getState().setGuides([], 'canvas.history.guideClear')}>
          {t('canvas.guide.clear')}
        </button>
        <button type="button" className="btn btn--accent" data-autofocus onClick={close}>{t('canvas.guide.close')}</button>
      </div>
    </Modal>
  );
}

/** Monta el diálogo de guías cuando se pide (menú Ver, doble clic en una regla o en una guía). */
export function GuidesDialogHost() {
  const open = useGuideUi((s) => s.open);
  return open ? <GuidesDialog /> : null;
}
