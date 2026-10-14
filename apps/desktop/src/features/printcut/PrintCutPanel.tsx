import { RotateCcw, X } from 'lucide-react';
import { useEffect, useId } from 'react';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { acceptRegistration, resetRegistration } from './actions';
import { computeRegistration } from './flow';
import { usePrintCut, type PrintCutStep } from './printcutStore';
import { StepCapture } from './StepCapture';
import { marksBlockedReason, StepMarks, useMarksMoved } from './StepMarks';
import { StepResult } from './StepResult';
import { closestPair, MIN_MARK_DISTANCE_MM } from './marks';
import './printcut.css';

const STEPS: PrintCutStep[] = ['marks', 'capture', 'result'];

/** Registro activo en las opciones del trabajo: aviso con el error medido y el botón para quitarlo. */
function ActiveBanner() {
  const transform = useStore((s) => s.jobOptions.transform);
  const applied = usePrintCut((s) => s.applied);
  if (!transform) return null;
  const same = applied !== null && applied.transform.every((v, i) => v === transform[i]);
  return (
    <div className="pc-active" role="status">
      <p>
        {same
          ? t('printcut.active', { rms: fmt(applied.rmsMm, 2), deg: fmt(applied.rotationDeg, 2), scale: fmt(applied.scale * 100, 2) })
          : t('printcut.active.generic')}
      </p>
      <button type="button" className="btn" title={t('printcut.reset.hint')} onClick={resetRegistration}>
        <RotateCcw size={14} aria-hidden="true" />{t('printcut.reset')}
      </button>
    </div>
  );
}

/** Asistente flotante de Print & Cut: no bloquea el lienzo, porque hay que elegir marcas y mover el láser. */
export function PrintCutPanel() {
  const titleId = useId();
  const step = usePrintCut((s) => s.step);
  const marks = usePrintCut((s) => s.marks);
  const physical = usePrintCut((s) => s.physical);
  const mode = usePrintCut((s) => s.mode);
  const patch = usePrintCut((s) => s.patch);
  const close = usePrintCut((s) => s.close);
  const restart = usePrintCut((s) => s.restart);
  const moved = useMarksMoved();
  const index = STEPS.indexOf(step);

  // Al cerrar el panel no debe quedar activo el modo de posicionar el láser con el puntero.
  useEffect(() => () => useStore.getState().setPositioningLaser(false), []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !(e.target instanceof HTMLInputElement)) close();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [close]);

  const spread = closestPair(marks.map((m) => m.point));
  const blockedMarks = marksBlockedReason(marks.length, spread, moved);
  const allMeasured = marks.length > 0 && physical.length >= marks.length && physical.every((p) => p !== null);
  const result = step === 'result' ? computeRegistration(marks, physical, mode) : null;
  const canAccept = result?.ok === true && !moved;

  const next = () => {
    if (step === 'marks' && !blockedMarks) patch({ step: 'capture', current: 0 });
    else if (step === 'capture' && allMeasured) patch({ step: 'result', picking: false });
  };
  const nextBlocked = step === 'marks' ? blockedMarks : step === 'capture' && !allMeasured ? 'printcut.capture.missing' : null;

  return (
    <aside className="pc-panel" role="dialog" aria-labelledby={titleId}>
      <header className="pc-head">
        <h2 id={titleId} className="pc-title">{t('printcut.title')}</h2>
        <button type="button" className="icon-btn" title={t('printcut.close')} aria-label={t('printcut.close')} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>
      <ActiveBanner />
      <ol className="pc-steps" aria-label={t('printcut.steps')}>
        {STEPS.map((s, i) => (
          <li key={s} aria-current={s === step ? 'step' : undefined} data-done={i < index}>
            <span className="pc-steps__n">{i + 1}</span>
            {t(`printcut.step.${s}`)}
          </li>
        ))}
      </ol>
      <div className="pc-body">
        {step === 'marks' && <StepMarks />}
        {step === 'capture' && <StepCapture />}
        {step === 'result' && <StepResult />}
      </div>
      <footer className="pc-foot">
        {nextBlocked && step !== 'result' && marks.length > 0 && (
          <p className="pc-reason" role="status">{t(nextBlocked, { min: MIN_MARK_DISTANCE_MM })}</p>
        )}
        {step !== 'marks' && (
          <button type="button" className="btn" onClick={() => (step === 'capture' ? restart() : patch({ step: 'capture' }))}>
            {step === 'capture' ? t('printcut.back') : t('printcut.remeasure')}
          </button>
        )}
        {step !== 'result' ? (
          <button type="button" className="btn btn--accent" aria-disabled={nextBlocked !== null} title={nextBlocked ? t(nextBlocked, { min: MIN_MARK_DISTANCE_MM }) : undefined} onClick={next}>
            {t('printcut.next')}
          </button>
        ) : (
          <button
            type="button"
            className="btn btn--accent"
            aria-disabled={!canAccept}
            onClick={() => {
              if (acceptRegistration()) close();
            }}
          >
            {t('printcut.accept')}
          </button>
        )}
      </footer>
    </aside>
  );
}

/** Monta el panel cuando está abierto. Lo coloca el shell una sola vez. */
export function PrintCutHost() {
  const open = usePrintCut((s) => s.open);
  return open ? <PrintCutPanel /> : null;
}
