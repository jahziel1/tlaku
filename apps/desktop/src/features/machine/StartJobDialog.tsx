import { Clock, Layers, TriangleAlert, Zap } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { t } from '@/i18n';
import { fmt, fmtClock } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { summarizeJob } from './jobSummary';
import { useJobPlacement } from './hooks';
import { Modal } from './Modal';
import { OriginIllustration } from './OriginIllustration';
import { placementSummary } from './StartFromSection';

/**
 * "¿Láser listo y área despejada?": última parada antes de que el láser se encienda. Resume tiempo,
 * capas, potencia máxima y dónde quedará el trabajo; "Cancelar" es el foco inicial.
 */
export function StartJobDialog() {
  const close = useStore((s) => s.closeDialog);
  const confirm = useStore((s) => s.confirmStart);
  const doc = useStore((s) => s.doc);
  const preview = useStore((s) => s.preview);
  const previewStatus = useStore((s) => s.previewStatus);
  const previewError = useStore((s) => s.previewError);
  const options = useStore((s) => s.jobOptions);
  const device = useStore(selectActiveDevice);
  const head = useStore(useShallow((s) => (s.connection.connected && s.status ? s.status.work_pos : null)));
  const placement = useJobPlacement();

  const ready = previewStatus === 'ready' && preview !== null;
  const summary = ready ? summarizeJob(doc, preview) : null;
  const where = placementSummary(placement, options.start_from, options.job_origin);
  const canStart = ready && placement.status === 'ok' && placement.fits;

  return (
    <Modal
      title={t('machine.start.title')}
      subtitle={device.name}
      tone="warn"
      size="md"
      onClose={close}
      footer={
        <>
          <button type="button" className="btn" data-autofocus onClick={close}>{t('machine.dlg.cancel')}</button>
          <button type="button" className="btn btn--accent" disabled={!canStart} title={canStart ? undefined : t('machine.start.blocked')} onClick={() => void confirm()}>
            {t('machine.start.confirm')}
          </button>
        </>
      }
    >
      <ul className="mc-list mc-list--check">
        {(['machine.start.check1', 'machine.start.check2', 'machine.start.check3', 'machine.start.check4'] as const).map((k) => (
          <li key={k}>{t(k)}</li>
        ))}
      </ul>

      {previewStatus === 'computing' && <p className="hint" role="status">{t('machine.start.computing')}</p>}
      {previewStatus === 'error' && (
        <p className="warn-line" role="alert">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{previewError}</span>
        </p>
      )}

      {summary && (
        <dl className="mc-summary">
          <div>
            <dt><Clock size={13} aria-hidden="true" />{t('machine.start.time')}</dt>
            <dd className="num">{fmtClock(summary.totalSeconds)}</dd>
          </div>
          <div>
            <dt><Layers size={13} aria-hidden="true" />{t('machine.start.layers', { n: summary.layers.length })}</dt>
            <dd>
              <ul className="mc-layers">
                {summary.layers.map((l) => (
                  <li key={l.color}>
                    <span className="mc-swatch" style={{ background: l.color }} aria-hidden="true" />
                    <span className="mc-layers__name">{l.name}</span>
                    <span className="num mc-layers__meta">{fmt(l.power, 0)} % · {fmt(l.speed, 0)} mm/min{l.passes > 1 ? ` · ${l.passes}×` : ''}</span>
                  </li>
                ))}
              </ul>
            </dd>
          </div>
          <div>
            <dt><Zap size={13} aria-hidden="true" />{t('machine.start.maxPower')}</dt>
            <dd className="num">{fmt(summary.maxPower, 0)} %</dd>
          </div>
        </dl>
      )}

      <div className="mc-start-where">
        <OriginIllustration device={device} placement={placement} jobOrigin={options.job_origin} head={head} userOrigin={options.user_origin} legend={false} />
        <p className={where.problem ? 'hint hint--danger' : 'hint'}>{where.text}</p>
      </div>

      {summary && summary.warnings.length > 0 && (
        <ul className="mc-warnings">
          {summary.warnings.map((w, i) => (
            <li key={i} className="warn-line">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{w}</span>
            </li>
          ))}
        </ul>
      )}
    </Modal>
  );
}
