import { useEffect, useRef, useState } from 'react';
import { t, type I18nKey } from '@/i18n';
import { useStore } from '@/store';

/** El avance se dice cada 25 %: más seguido saturaría al lector de pantalla. */
const PROGRESS_STEP = 25;

const JOB_TEXT: Partial<Record<string, I18nKey>> = {
  running: 'a11y.job.running',
  paused: 'a11y.job.paused',
  finished: 'a11y.job.finished',
  cancelled: 'a11y.job.cancelled',
  error: 'a11y.job.error',
};

/**
 * Anuncia el trabajo a lectores de pantalla: cada cambio de estado (en marcha, pausa, terminado,
 * cancelado, error) y el avance por tramos de 25 %. Región viva visualmente oculta; los errores van
 * por `role="alert"` para que se lean de inmediato.
 */
export function LiveAnnouncer() {
  const jobState = useStore((s) => s.jobState);
  const progress = useStore((s) => s.progress);
  const [text, setText] = useState('');
  const [urgent, setUrgent] = useState('');
  const lastStep = useRef(-1);

  useEffect(() => {
    const key = JOB_TEXT[jobState];
    if (jobState === 'running') lastStep.current = 0;
    if (!key) return;
    if (jobState === 'error') setUrgent(t(key));
    else {
      setUrgent('');
      setText(t(key));
    }
  }, [jobState]);

  const pct = progress && progress.total > 0 ? Math.floor((progress.acked / progress.total) * 100) : 0;
  const step = Math.floor(pct / PROGRESS_STEP) * PROGRESS_STEP;
  useEffect(() => {
    if (jobState !== 'running' || step <= lastStep.current || step >= 100) return;
    lastStep.current = step;
    setText(t('a11y.job.progress', { pct: step }));
  }, [jobState, step]);

  return (
    <>
      <p className="sr-only" role="status" aria-live="polite" aria-atomic="true">{text}</p>
      <p className="sr-only" role="alert" aria-atomic="true">{urgent}</p>
    </>
  );
}
