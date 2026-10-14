import { History, TriangleAlert } from 'lucide-react';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { Modal } from './Modal';

/** Fecha y hora local corta. */
function when(ms: number): string {
  return new Date(ms).toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' });
}

/**
 * «El último trabajo se interrumpió al N % (línea L de T). ¿Continuar?». Se abre al iniciar la app y cuando la
 * conexión se pierde a mitad de un trabajo. Con G-code ofrece Continuar, Elegir el punto y Descartar; con Ruida,
 * K40 o galvo solo explica qué pasa (el motor trae el texto en `note`).
 */
export function ResumeNotice() {
  const job = useStore((s) => s.interrupted);
  const close = useStore((s) => s.closeDialog);
  const openResume = useStore((s) => s.openResume);
  const discard = useStore((s) => s.discardInterrupted);
  const setDockTab = useStore((s) => s.setDockTab);
  if (!job) return null;
  const where = t('machine.resume.notice.when', {
    machine: job.device_name,
    document: job.document ? (job.document.split(/[\\/]/).pop() ?? job.document) : t('machine.resume.notice.untitled'),
    when: when(job.updated_at_ms),
  });
  const progressText = job.progress_known
    ? t('machine.resume.notice.text', { percent: fmt(job.percent, 0), line: job.done_lines, total: job.total_lines })
    : t('machine.resume.notice.unknown');

  return (
    <Modal
      title={t('machine.resume.notice.title')}
      subtitle={where}
      tone="warn"
      onClose={close}
      footer={
        job.can_resume ? (
          <>
            <button type="button" className="btn" onClick={close}>{t('machine.resume.notice.later')}</button>
            <button type="button" className="btn btn--danger" onClick={() => void discard()}>{t('machine.resume.notice.discard')}</button>
            <button type="button" className="btn" onClick={() => openResume(true)}>{t('machine.resume.notice.choose')}</button>
            <button type="button" className="btn btn--accent" data-autofocus onClick={() => openResume(false)}>{t('machine.resume.notice.continue')}</button>
          </>
        ) : (
          <>
            <button type="button" className="btn btn--danger" onClick={() => void discard()}>{t('machine.resume.notice.discard')}</button>
            {job.kind === 'ruida' && (
              <button
                type="button"
                className="btn"
                onClick={() => {
                  close();
                  setDockTab('machine');
                }}
              >
                {t('machine.resume.notice.openMachine')}
              </button>
            )}
            <button type="button" className="btn btn--accent" data-autofocus onClick={close}>{t('machine.resume.notice.dismiss')}</button>
          </>
        )
      }
    >
      <p className="mc-resume__lead">
        <History size={16} aria-hidden="true" />
        <span>{job.can_resume ? progressText : job.note}</span>
      </p>
      {job.can_resume && (
        <div className="mc-resume__bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(job.percent)} aria-label={t('machine.resume.notice.title')}>
          <span style={{ width: `${Math.min(100, Math.max(0, job.percent))}%` }} />
        </div>
      )}
      {job.can_resume && job.note && (
        <p className="warn-line">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{job.note}</span>
        </p>
      )}
      {job.can_resume && <p className="hint">{t('machine.resume.notice.hint')}</p>}
    </Modal>
  );
}
