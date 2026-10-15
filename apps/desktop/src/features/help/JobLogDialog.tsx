import { useEffect, useMemo, useState } from 'react';
import { askChoice } from '@/features/history/prompt';
import { t, type I18nKey } from '@/i18n';
import { fmtClock } from '@/lib/platform';
import { Modal } from '@/ui/Modal';
import { Download, X } from 'lucide-react';
import { useStore, usePref } from '@/store';
import { errorMessage } from '@/lib/errors';
import { useHelpUi } from './helpStore';
import { useJobLog, type JobLogEntry } from './jobLog';
import { JobLogDetail } from './JobLogDetail';
import { deltaLabel, fmtWhen, thumbUrl } from './jobLogFormat';
import { saveJobsCsv, jobsCsv } from './jobLogCsv';
import { EMPTY_FILTER, distinctMachines, distinctMaterials, filterJobs, isFilterEmpty, materialsOf, type JobFilter } from './jobLogFilter';

export { deltaLabel, fmtWhen };

export function resultLabel(e: Pick<JobLogEntry, 'result'>): string {
  return t(`help.jobLog.result.${e.result}` as I18nKey);
}

interface RowsProps {
  entries: JobLogEntry[];
  resumableId?: string | null;
  onContinue?: () => void;
  selectedId?: string | null;
  onSelect?: (id: string) => void;
}

export function JobRows({ entries, resumableId = null, onContinue, selectedId = null, onSelect }: RowsProps) {
  return (
    <tbody>
      {entries.map((e) => {
        // Solo un trabajo terminado se compara con su estimado (uno cancelado a los 3 s no «tardó -99 %»).
        const delta = e.result === 'finished' ? deltaLabel(e) : null;
        const name = e.document ?? t('help.jobLog.untitled');
        const material = materialsOf(e).join(', ');
        return (
          <tr key={e.id} data-result={e.result} data-selected={selectedId === e.id ? 'true' : undefined} className={selectedId === e.id ? 'joblog__row joblog__row--selected' : 'joblog__row'} onClick={onSelect ? () => onSelect(e.id) : undefined}>
            <td className="joblog__thumbcell">{e.thumb ? <img className="joblog__thumb" alt="" src={thumbUrl(e.thumb)} width={48} /> : null}</td>
            <td>{fmtWhen(e.at)}</td>
            <td>{e.machine}</td>
            <td>
              {onSelect ? (
                <button type="button" className="joblog__open" aria-pressed={selectedId === e.id} title={t('help.jobLog.showDetail')} onClick={(ev) => { ev.stopPropagation(); onSelect(e.id); }}>
                  {name}
                </button>
              ) : (
                name
              )}
            </td>
            <td>{material || t('help.jobLog.noEstimate')}</td>
            <td className="num">{e.estimatedS === null ? t('help.jobLog.noEstimate') : fmtClock(e.estimatedS)}</td>
            <td className="num" title={delta ?? undefined}>{e.result === 'running' ? t('help.jobLog.noEstimate') : fmtClock(e.actualS)}</td>
            <td>
              <span className={`joblog__result joblog__result--${e.result}`} title={e.message || undefined}>
                {resultLabel(e)}
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

const CAP_CHOICES = [100, 250, 500, 1000, 2500, 5000, 20000];

/** Barra de búsqueda y filtros. */
function FilterBar({ entries, filter, onChange }: { entries: JobLogEntry[]; filter: JobFilter; onChange: (f: JobFilter) => void }) {
  const machines = useMemo(() => distinctMachines(entries), [entries]);
  const materials = useMemo(() => distinctMaterials(entries), [entries]);
  const set = <K extends keyof JobFilter>(k: K, v: JobFilter[K]) => onChange({ ...filter, [k]: v });
  return (
    <div className="joblog__filters" role="search" aria-label={t('help.jobLog.filter.label')}>
      <input type="search" className="input joblog__search" value={filter.text} placeholder={t('help.jobLog.filter.search')} aria-label={t('help.jobLog.filter.search')} onChange={(e) => set('text', e.target.value)} />
      <select className="select" value={filter.machine} aria-label={t('help.jobLog.filter.machine')} onChange={(e) => set('machine', e.target.value)}>
        <option value="">{t('help.jobLog.filter.anyMachine')}</option>
        {machines.map((m) => (
          <option key={m} value={m}>{m}</option>
        ))}
      </select>
      <select className="select" value={filter.material} aria-label={t('help.jobLog.filter.material')} onChange={(e) => set('material', e.target.value)}>
        <option value="">{t('help.jobLog.filter.anyMaterial')}</option>
        {materials.map((m) => (
          <option key={m} value={m}>{m}</option>
        ))}
      </select>
      <select className="select" value={filter.result} aria-label={t('help.jobLog.filter.result')} onChange={(e) => set('result', e.target.value as JobFilter['result'])}>
        <option value="">{t('help.jobLog.filter.anyResult')}</option>
        {(['finished', 'cancelled', 'error', 'interrupted'] as const).map((r) => (
          <option key={r} value={r}>{t(`help.jobLog.result.${r}` as I18nKey)}</option>
        ))}
      </select>
      <label className="joblog__date">
        <span>{t('help.jobLog.filter.from')}</span>
        <input type="date" className="input" value={filter.from} max={filter.to || undefined} onChange={(e) => set('from', e.target.value)} />
      </label>
      <label className="joblog__date">
        <span>{t('help.jobLog.filter.to')}</span>
        <input type="date" className="input" value={filter.to} min={filter.from || undefined} onChange={(e) => set('to', e.target.value)} />
      </label>
      <button type="button" className="btn" disabled={isFilterEmpty(filter)} onClick={() => onChange(EMPTY_FILTER)}>
        {t('help.jobLog.filter.clear')}
      </button>
    </div>
  );
}

/** Historial local de trabajos enviados a la máquina: búsqueda y filtros, detalle, repetir con los mismos ajustes y CSV. */
export function JobLogDialog() {
  const close = useHelpUi((s) => s.close);
  const entries = useJobLog((s) => s.entries);
  const saveError = useJobLog((s) => s.saveError);
  const load = useJobLog((s) => s.load);
  const clear = useJobLog((s) => s.clear);
  const notify = useStore((s) => s.notify);
  const setPref = useStore((s) => s.setPref);
  const cap = usePref('jobHistoryMax');
  const [filter, setFilter] = useState<JobFilter>(EMPTY_FILTER);
  const [selectedId, setSelectedId] = useState<string | null>(null);
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
    void load();
  }, [load]);

  const shown = useMemo(() => filterJobs(entries, filter), [entries, filter]);
  const selected = entries.find((e) => e.id === selectedId) ?? null;
  const caps = CAP_CHOICES.includes(cap) ? CAP_CHOICES : [...CAP_CHOICES, cap].sort((a, b) => a - b);

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
    if (choice === 'ok') {
      clear();
      setSelectedId(null);
    }
  };

  const onExport = async () => {
    try {
      const where = await saveJobsCsv(jobsCsv(shown));
      if (where) notify('success', t('help.jobLog.csv.saved', { n: shown.length, where }));
    } catch (e) {
      notify('error', t('help.jobLog.csv.failed', { error: errorMessage(e) }));
    }
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
        {saveError && <p className="warn-line" role="alert">{t('help.jobLog.saveError', { error: saveError })}</p>}
        {entries.length === 0 ? (
          <p className="hint" data-autofocus tabIndex={-1}>{t('help.jobLog.empty')}</p>
        ) : (
          <>
            <FilterBar entries={entries} filter={filter} onChange={setFilter} />
            {shown.length === 0 ? (
              <p className="hint" data-autofocus tabIndex={-1} role="status">{t('help.jobLog.noMatch')}</p>
            ) : (
              <table className="joblog" data-autofocus tabIndex={-1} aria-label={t('a11y.table.log')}>
                <thead>
                  <tr>
                    <th aria-label={t('help.jobLog.col.thumb')} />
                    <th>{t('help.jobLog.col.date')}</th>
                    <th>{t('help.jobLog.col.machine')}</th>
                    <th>{t('help.jobLog.col.document')}</th>
                    <th>{t('help.jobLog.col.material')}</th>
                    <th>{t('help.jobLog.col.estimated')}</th>
                    <th>{t('help.jobLog.col.actual')}</th>
                    <th>{t('help.jobLog.col.result')}</th>
                  </tr>
                </thead>
                <JobRows
                  entries={shown}
                  resumableId={canResume}
                  selectedId={selectedId}
                  onSelect={setSelectedId}
                  onContinue={() => {
                    close();
                    openResume(false);
                  }}
                />
              </table>
            )}
            {selected && (
              <JobLogDetail
                entry={selected}
                onClose={() => setSelectedId(null)}
                onRepeated={close}
                onDeleted={() => setSelectedId(null)}
              />
            )}
          </>
        )}
      </div>
      <div className="help__foot joblog__foot">
        <span className="hint">{isFilterEmpty(filter) ? t('help.jobLog.count', { n: entries.length }) : t('help.jobLog.countFiltered', { shown: shown.length, n: entries.length })}</span>
        <label className="joblog__cap">
          <span>{t('help.jobLog.cap')}</span>
          <select className="select" value={cap} aria-label={t('help.jobLog.cap')} title={t('help.jobLog.cap.hint')} onChange={(e) => setPref('jobHistoryMax', Number(e.target.value))}>
            {caps.map((n) => (
              <option key={n} value={n}>{n}</option>
            ))}
          </select>
        </label>
        <button type="button" className="btn" disabled={shown.length === 0} onClick={() => void onExport()}>
          <Download size={14} aria-hidden="true" />
          {t('help.jobLog.csv.export')}
        </button>
        <button type="button" className="btn" disabled={entries.length === 0} onClick={() => void onClear()}>
          {t('help.jobLog.clear')}
        </button>
      </div>
    </Modal>
  );
}
