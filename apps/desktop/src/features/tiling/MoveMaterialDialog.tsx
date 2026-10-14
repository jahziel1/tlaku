import { LocateFixed } from 'lucide-react';
import { useId } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { fmt } from '@/lib/platform';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { positionLaserBlockedReason } from '@/store/machine';
import { Modal } from '@/ui/Modal';
import { bedPoint, moveBetween, registrationMarks, tileCell } from './guide';
import { sendCurrent, skipCurrent } from './run';
import { useTilingUi } from './store';

/**
 * Pausa guiada del tiling (M47): cuando el siguiente tramo está en otra columna o fila y el
 * alimentador no puede llegar (solo mueve un eje), dice qué mover y ayuda a alinear el material con
 * las cruces ya grabadas (marcas de registro): lleva el puntero a donde la cruz debe quedar.
 */
export function MoveMaterialDialog() {
  const { session, guide, marks, config } = useTilingUi(useShallow((s) => ({ session: s.session, guide: s.guide, marks: s.marks, config: s.config })));
  const labelId = useId();
  const blocked = useStore((s) => positionLaserBlockedReason(s));
  const goToPointer = useStore((s) => s.goToPointerDocPoint);
  if (!session || !guide || session.last === undefined) return null;
  const n = session.current;
  const spec = session.specs[n]!;
  const cell = tileCell(session.specs, n);
  const kind = moveBetween(session.specs, session.last, n);
  const cross = registrationMarks(session.specs, session.status, marks, n);
  const close = () => useTilingUi.getState().set({ guide: false });
  const title =
    kind === 'row'
      ? t('tiling.move.titleRow', { row: cell.row, rows: cell.rows })
      : kind === 'column'
        ? t('tiling.move.titleCol', { col: cell.col, cols: cell.cols })
        : t('tiling.move.titleBoth', { col: cell.col, cols: cell.cols, row: cell.row, rows: cell.rows });

  return (
    <Modal labelId={labelId} onClose={close} portal backdropClassName="til-backdrop" cardClassName="til-modal">
      <h2 id={labelId} className="til-sub">{title}</h2>
      <p>{t('tiling.move.text', { n: n + 1, x: fmt(spec.rect[0][0], 1), y: fmt(spec.rect[0][1], 1), m: fmt(config.margin, 1) })}</p>
      {cross.length > 0 ? (
        <>
          <p>{t('tiling.move.align')}</p>
          <ul className="til-marks num">
            {cross.map((c, i) => {
              const [bx, by] = bedPoint(spec, c);
              return (
                <li key={i}>
                  <span>{t('tiling.move.mark', { i: i + 1, x: fmt(c[0], 1), y: fmt(c[1], 1) })}</span>
                  <button type="button" className="btn" aria-disabled={!!blocked} title={blocked ? t(blocked) : t('tiling.move.pointerHint', { x: fmt(bx, 1), y: fmt(by, 1) })} onClick={blocked ? undefined : () => void goToPointer(bx, by)}>
                    <LocateFixed size={14} aria-hidden="true" />
                    {t('tiling.move.pointer')}
                  </button>
                </li>
              );
            })}
          </ul>
        </>
      ) : (
        <p className="hint">{t(config.marks ? 'tiling.move.noMarksInOverlap' : 'tiling.move.marksOff')}</p>
      )}
      <div className="til-actions">
        <button type="button" className="btn btn--accent" data-autofocus onClick={() => { close(); void sendCurrent(); }}>{t('tiling.move.ready', { n: n + 1 })}</button>
        <button type="button" className="btn" onClick={() => { close(); skipCurrent(); }}>{t('tiling.skip', { n: n + 1 })}</button>
        <button type="button" className="btn" onClick={close}>{t('tiling.move.later')}</button>
      </div>
    </Modal>
  );
}
