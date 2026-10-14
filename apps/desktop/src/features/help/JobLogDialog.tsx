import { useEffect } from 'react';
import { askChoice } from '@/features/history/prompt';
import { t, type I18nKey } from '@/i18n';
import { fmtClock } from '@/lib/platform';
import { Modal } from '@/ui/Modal';
import { X } from 'lucide-react';
import { useStore } from '@/store';
import { useHelpUi } from './helpStore';
import { useJobLog, type JobLogEntry } from './jobLog';

/** Fecha y hora local corta, en el idioma de la interfaz. */
function fmtWhen(ms: number): string {
  return new Date(ms).toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' });
}

/** Cuánto más (o menos) tardó el trabajo que lo estimado, para un vistazo rápido: «+12 %». */
export function deltaLabel(e: Pick<JobLogEntry, 'estimatedS' | 'actualS'>): string | null {
  if (e.estimatedS === null || e.estimatedS <= 0) return null;
  const pct = Math.round(((e.actualS - e.estimatedS) / e.estimatedS) * 100);
  return `${pct > 0 ? '+' : ''}${pct} %`;
}

export function JobRows({ entries, resumableId = null, onContinue }: { entries: JobLogEntry[]; resumableId?: string | null; onContinue?: () => void }) {
  return (
    <tbody>
      {entries.map((e) => {
        const delta = deltaLabel(e);
        return (
          <tr key={e.id} data-result={e.result}>
            <td>{fmtWhen(e.at)}</td>
            <td>{e.machine}</td>
            <td>{e.document ?? t('help.jobLog.untitled')}</td>
            <td className="num">{e.estimatedS === null ? t('help.jobLog.noEstimate') : fmtClock(e.estimatedS)}</td>
            <td className="num" title={delta ?? undefined}>{fmtClock(e.actualS)}</td>
            <td>
              <span className={`joblog__result joblog__result--${e.result}`} title={e.message || undefined}>
                {t(`help.jobLog.result.${e.result}` as I18nKey)}
              </span>
              {e.interrupted && (
                <>
                  {' '}
                  <span className="joblog__interrupted">{t('machine.resume.log.interrupted', { percent: Math.round(e.interrupted.percent) })}</span>
                  {onContinue && resumableId === e.interrupted.id && (
                    <button type="button" className="btn joblog__resume" title={t('machine.resume.log.continueTitle')} onClick={onContinue}>
                      {t('machine.resume.log.continue')}
                    </button>
                  )}
                </>
              )}
            </td>
          </tr>
        );
      })}
    </tbody>
  );
}

/** Registro local de trabajos enviados a la máquina, con un botón para borrarlo. */
export function JobLogDialog() {
  const close = useHelpUi((s) => s.close);
  const entries = useJobLog((s) => s.entries);
  const refresh = useJobLog((s) => s.refresh);
  const clear = useJobLog((s) => s.clear);
  // «Continuar» solo mientras siga existiendo el punto de control del trabajo interrumpido.
  const interrupted = useStore((s) => s.interrupted);
  const checkInterrupted = useStore((s) => s.checkInterrupted);
  const openResume = useStore((s) => s.openResume);
  const canResume = interrupted?.can_resume ? interrupted.id : null;
  useEffect(() => {
    void checkInterrupted();
  }, [checkInterrupted]);

  // Otra ventana de la app puede haber anotado un trabajo mientras esta estaba cerrada.
  useEffect(() => {
    refresh();
    window.addEventListener('storage', refresh);
    return () => window.removeEventListener('storage', refresh);
  }, [refresh]);

  const onClear = async () => {
    const choice = await askChoice({
      title: t('help.jobLog.clear.title'),
      message: t('help.jobLog.clear.message', { n: entries.length }),
      choices: [
        { id: 'ok', label: t('help.jobLog.clear.confirm'), tone: 'danger' },
        { id: 'cancel', label: t('help.jobLog.clear.cancel') },
      ],
      cancelId: 'cancel',
    });
    if (choice === 'ok') clear();
  };

  return (
    <Modal labelId="help-joblog-title" onClose={close} backdropClassName="help" cardClassName="help__card help__card--wide" initialFocus="[data-autofocus]" closeOnBackdrop guardOutside>
      <div className="help__head">
        <h2 id="help-joblog-title" className="help__title">{t('help.jobLog.title')}</h2>
        <button type="button" className="icon-btn" aria-label={t('help.close')} title={t('help.close')} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </div>
      <div className="help__scroll help__prose">
        <p>{t('help.jobLog.intro')}</p>
        {entries.length === 0 ? (
          <p className="hint" data-autofocus tabIndex={-1}>{t('help.jobLog.empty')}</p>
        ) : (
          <table className="joblog" data-autofocus tabIndex={-1} aria-label={t('a11y.table.log')}>
            <thead>
              <tr>
                <th>{t('help.jobLog.col.date')}</th>
                <th>{t('help.jobLog.col.machine')}</th>
                <th>{t('help.jobLog.col.document')}</th>
                <th>{t('help.jobLog.col.estimated')}</th>
                <th>{t('help.jobLog.col.actual')}</th>
                <th>{t('help.jobLog.col.result')}</th>
              </tr>
            </thead>
            <JobRows
              entries={entries}
              resumableId={canResume}
              onContinue={() => {
                close();
                openResume(false);
              }}
            />
          </table>
        )}
      </div>
      <div className="help__foot">
        <span className="hint">{t('help.jobLog.count', { n: entries.length })}</span>
        <button type="button" className="btn" disabled={entries.length === 0} onClick={() => void onClear()}>
          {t('help.jobLog.clear')}
        </button>
      </div>
    </Modal>
  );
}
