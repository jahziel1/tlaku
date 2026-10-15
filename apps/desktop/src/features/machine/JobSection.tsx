import { Calculator, Crosshair, Pause, Play, Settings2, Spline, Square, TriangleAlert, X } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';
import { fmt, fmtClock } from '@/lib/platform';
import { selectJobPreview, selectionBlockedReason, useStore } from '@/store';
import { jobElapsedSeconds } from '@/store/machine';
import { useJobPlacement, useNow } from './hooks';
import { remainingSeconds } from './jobSummary';
import { FrameOptions } from './FrameOptions';
import { SelectionOptions } from './SelectionOptions';
import { LiveToggle } from '@/features/live';
import { TilingOffer } from '@/features/tiling/TilingOffer';
import { placementSummary, START_FROM_TITLE, StartFromSection } from './StartFromSection';
import { PanelSection } from '@/ui/PanelSection';

function Progress() {
  const progress = useStore((s) => s.progress);
  const jobState = useStore((s) => s.jobState);
  const estimate = useStore((s) => selectJobPreview(s)?.total_s ?? null);
  const timing = useStore(useShallow((s) => ({ jobStartedAt: s.jobStartedAt, jobPausedAt: s.jobPausedAt, jobPausedMs: s.jobPausedMs, jobEndedAt: s.jobEndedAt })));
  const running = jobState === 'running';
  const live = running || jobState === 'paused';
  const now = useNow(running);

  if (!live && jobState !== 'finished') return null;
  const pct = jobState === 'finished' ? 1 : progress && progress.total > 0 ? progress.acked / progress.total : 0;
  const elapsed = jobElapsedSeconds(timing, now);
  const remaining = live ? remainingSeconds(pct, estimate, elapsed) : 0;
  const percent = Math.round(pct * 100);

  return (
    <div className="mc-progress">
      <div className="progress" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} aria-label={t('machine.progress')}>
        <div className="progress__bar" style={{ width: `${pct * 100}%` }} />
        <span className="progress__text num">{percent} %</span>
      </div>
      <dl className="mc-progress__meta num">
        <div>
          <dt>{t('machine.progress.lines')}</dt>
          <dd>{progress ? `${fmt(progress.acked, 0)} / ${fmt(progress.total, 0)}` : '—'}</dd>
        </div>
        <div>
          <dt>{t('machine.progress.elapsed')}</dt>
          <dd>{fmtClock(elapsed)}</dd>
        </div>
        <div>
          <dt>{t('machine.progress.remaining')}</dt>
          <dd>{remaining === null ? '—' : fmtClock(remaining)}</dd>
        </div>
      </dl>
      {jobState === 'paused' && <p className="hint">{t('machine.progress.paused')}</p>}
    </div>
  );
}

/** Resumen de «Iniciar desde» para la sección cerrada: el modo o, si el trabajo no cabe o falta algo, el problema. */
function StartFromSummary() {
  const startFrom = useStore((s) => s.jobOptions.start_from);
  const jobOrigin = useStore((s) => s.jobOptions.job_origin);
  const placement = useJobPlacement();
  const summary = placementSummary(placement, startFrom, jobOrigin);
  if (summary.problem) return <span className="hint--danger">{summary.text}</span>;
  return <>{t(START_FROM_TITLE[startFrom])}</>;
}

/** Resumen de «Qué se envía» para la sección cerrada. */
function SelectionSummary() {
  const only = useStore((s) => s.cutSelectionOnly);
  const blocked = useStore((s) => selectionBlockedReason(s) !== null);
  return <>{only && !blocked ? t('machine.selection.only') : t('machine.selection.summaryAll')}</>;
}

/**
 * Trabajo: encuadrar, vista previa, iniciar, pausar, detener, vista en vivo y progreso. De dónde sale
 * el trabajo, qué se envía y las opciones del encuadre van en secciones cerradas dentro de esta.
 */
export function JobSection() {
  const frame = useCommand('machine.frame');
  const hull = useCommand('machine.frameHull');
  const optimization = useCommand('preview.optimization');
  const costs = useCommand('costs.open');
  const start = useCommand('machine.start');
  const pause = useCommand('machine.pause');
  const stop = useCommand('machine.stop');
  const jobError = useStore((s) => s.jobError);
  const dismiss = useStore((s) => s.dismissJobError);
  const jobState = useStore((s) => s.jobState);
  const jobMessage = useStore((s) => s.jobMessage);
  const frameLaserOn = useStore((s) => s.frameLaser.on);

  return (
    <PanelSection id="machine.job" title={t('machine.job')} defaultOpen>
      <TilingOffer />
      <div className="row">
        <button type="button" className="btn grow" disabled={!frame.enabled} title={frame.tooltip} onClick={frame.run}>
          <Crosshair size={15} aria-hidden="true" />{frame.label}
        </button>
        <button type="button" className="btn grow" disabled={!hull.enabled} title={hull.tooltip} onClick={hull.run}>
          <Spline size={15} aria-hidden="true" />{hull.label}
        </button>
      </div>
      <div className="row">
        <button type="button" className="btn grow" onClick={optimization.run} title={optimization.tooltip}>
          <Settings2 size={15} aria-hidden="true" />{optimization.label}
        </button>
        <button type="button" className="btn grow" onClick={costs.run} title={costs.tooltip}>
          <Calculator size={15} aria-hidden="true" />{costs.label}
        </button>
      </div>
      <div className="row">
        <button type="button" className="btn btn--accent grow" disabled={!start.enabled} title={start.tooltip} onClick={start.run}>
          <Play size={15} aria-hidden="true" />{start.label}
        </button>
      </div>
      <div className="row">
        <button type="button" className="btn grow" disabled={!pause.enabled} title={pause.tooltip} onClick={pause.run}>
          <Pause size={15} aria-hidden="true" />{pause.label}
        </button>
        <button type="button" className="btn btn--danger grow" disabled={!stop.enabled} title={stop.tooltip} onClick={stop.run}>
          <Square size={15} aria-hidden="true" />{stop.label}
        </button>
      </div>
      <LiveToggle />
      {jobError && (
        <div className="mc-joberror" role="alert">
          <TriangleAlert size={15} aria-hidden="true" />
          <div>
            <p className="mc-joberror__title">{t('machine.job.rejected')}</p>
            <pre className="mc-joberror__text">{jobError}</pre>
          </div>
          <button type="button" className="icon-btn" title={t('machine.dlg.close')} aria-label={t('machine.dlg.close')} onClick={dismiss}>
            <X size={14} aria-hidden="true" />
          </button>
        </div>
      )}
      {jobState === 'cancelled' && <p className="hint">{jobMessage || t('machine.job.cancelled')}</p>}
      {jobState === 'error' && !jobError && <p className="hint hint--danger" role="alert">{jobMessage || t('machine.job.failed')}</p>}
      <Progress />
      <PanelSection id="machine.job.startFrom" title={t('machine.startFrom')} summary={<StartFromSummary />} nested>
        <StartFromSection />
      </PanelSection>
      <PanelSection id="machine.job.selection" title={t('machine.selection')} summary={<SelectionSummary />} nested>
        <SelectionOptions />
      </PanelSection>
      <PanelSection id="machine.job.frame" title={t('machine.job.frameOpts')} summary={frameLaserOn ? t('machine.frame.laserOn') : undefined} nested>
        <FrameOptions />
      </PanelSection>
    </PanelSection>
  );
}
