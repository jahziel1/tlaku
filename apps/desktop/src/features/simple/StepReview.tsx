import { Clock, Frame } from 'lucide-react';
import { useCommand } from '@/commands/useCommand';
import { summarizeJob } from '@/features/machine/jobSummary';
import { t } from '@/i18n';
import { fmtClock } from '@/lib/platform';
import { useStore } from '@/store';
import { SimpleReview } from '@/features/review';
import type { SimpleData } from './hooks';
import { ProblemList } from './Problems';

/** Paso 4: vista previa con el tiempo estimado, y el botón Encuadrar (el láser recorre el borde con el haz apagado). */
export function StepReview({ data }: { data: SimpleData }) {
  const doc = useStore((s) => s.doc);
  const preview = useStore((s) => s.preview);
  const status = useStore((s) => s.previewStatus);
  const error = useStore((s) => s.previewError);
  const stale = useStore((s) => s.previewStale);
  const frame = useCommand('machine.frame');
  const ready = status === 'ready' && preview !== null && !stale;
  const summary = ready ? summarizeJob(doc, preview) : null;
  return (
    <>
      <div className="simple__time" role="status" aria-live="polite">
        <Clock size={26} aria-hidden="true" />
        {summary ? (
          <div>
            <span className="simple__time-label">{t('simple.review.time')}</span>
            <strong className="simple__time-value num">{fmtClock(summary.totalSeconds)}</strong>
          </div>
        ) : status === 'error' ? (
          <span className="hint hint--danger">{error}</span>
        ) : (
          <span>{t('simple.review.computing')}</span>
        )}
      </div>
      <p className="hint">{t('simple.review.canvas')}</p>
      {summary && (
        <ul className="simple__summary">
          {summary.layers.map((l) => (
            <li key={l.color}>
              <span className="simple__dot" style={{ background: l.color }} aria-hidden="true" />
              <span className="simple__summary-name">{l.name}</span>
              <span className="num">{fmtClock(l.seconds)}</span>
            </li>
          ))}
        </ul>
      )}
      <button type="button" className="btn simple__btn simple__btn--big" disabled={!frame.enabled} title={frame.tooltip} onClick={frame.run}>
        <Frame size={18} aria-hidden="true" />
        {t('simple.review.frame')}
      </button>
      <p className="hint">{frame.enabled ? t('simple.review.frame.help') : (frame.reason ?? '')}</p>
      <ProblemList step="review" problems={data.problems} />
      <SimpleReview shown={data.problems.map((p) => p.id)} />
    </>
  );
}
