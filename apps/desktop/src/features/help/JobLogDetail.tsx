import { useEffect, useState } from 'react';
import { Copy, FolderOpen, Trash2, X } from 'lucide-react';
import { openProject } from '@/features/history/files';
import { askChoice } from '@/features/history/prompt';
import { t, type I18nKey } from '@/i18n';
import { fmt, fmtClock } from '@/lib/platform';
import { useJobLog, type JobLogEntry } from './jobLog';
import { fmtWhen, deltaLabel, thumbUrl } from './jobLogFormat';
import { repeatWithSettings } from './jobLogRepeat';

const MODE_KEYS: Record<string, I18nKey> = {
  line: 'layers.mode.line',
  fill: 'layers.mode.fill',
  image: 'layers.mode.image',
};

const num = (v: number | boolean | undefined, digits = 0): string => (typeof v === 'number' ? fmt(v, digits) : '—');

/** Detalle de un trabajo del historial: miniatura, datos, capas con sus ajustes, notas y acciones. */
export function JobLogDetail({ entry, onClose, onRepeated, onDeleted }: { entry: JobLogEntry; onClose: () => void; onRepeated: () => void; onDeleted: () => void }) {
  const patch = useJobLog((s) => s.patch);
  const remove = useJobLog((s) => s.remove);
  const [notes, setNotes] = useState(entry.notes ?? '');
  // Al elegir otra fila se muestran sus notas.
  useEffect(() => setNotes(entry.notes ?? ''), [entry.id, entry.notes]);
  const saveNotes = () => {
    if (notes !== (entry.notes ?? '')) patch(entry.id, { notes });
  };
  const delta = deltaLabel(entry);
  const canRepeat = (entry.layers?.length ?? 0) > 0 && entry.result !== 'running';

  const onDelete = async () => {
    const choice = await askChoice({
      title: t('help.jobLog.delete.title'),
      message: t('help.jobLog.delete.message', { name: entry.document ?? t('help.jobLog.untitled'), when: fmtWhen(entry.at) }),
      choices: [
        { id: 'ok', label: t('help.jobLog.delete.confirm'), tone: 'danger' },
        { id: 'cancel', label: t('help.jobLog.clear.cancel') },
      ],
      cancelId: 'cancel',
    });
    if (choice === 'ok') {
      remove([entry.id]);
      onDeleted();
    }
  };

  return (
    <section className="joblog__detail" aria-labelledby="joblog-detail-title" data-testid="joblog-detail">
      <div className="joblog__detail-head">
        <h3 id="joblog-detail-title" className="help__subtitle">{entry.document ?? t('help.jobLog.untitled')}</h3>
        <button type="button" className="icon-btn" aria-label={t('help.jobLog.detail.close')} title={t('help.jobLog.detail.close')} onClick={onClose}>
          <X size={14} aria-hidden="true" />
        </button>
      </div>
      <div className="joblog__detail-body">
        {entry.thumb && <img className="joblog__thumb-big" alt={t('help.jobLog.thumbAlt')} src={thumbUrl(entry.thumb)} width={192} />}
        <dl className="joblog__facts">
          <dt>{t('help.jobLog.col.date')}</dt>
          <dd>{fmtWhen(entry.at)}</dd>
          <dt>{t('help.jobLog.col.machine')}</dt>
          <dd>{entry.machine}</dd>
          {entry.documentPath && (
            <>
              <dt>{t('help.jobLog.detail.path')}</dt>
              <dd className="joblog__path">{entry.documentPath}</dd>
            </>
          )}
          {entry.tab && (
            <>
              <dt>{t('help.jobLog.detail.tab')}</dt>
              <dd>{entry.tab}</dd>
            </>
          )}
          <dt>{t('help.jobLog.col.material')}</dt>
          <dd>{entry.material || t('help.jobLog.noEstimate')}</dd>
          <dt>{t('help.jobLog.col.estimated')} / {t('help.jobLog.col.actual')}</dt>
          <dd>
            {entry.estimatedS === null ? t('help.jobLog.noEstimate') : fmtClock(entry.estimatedS)} / {fmtClock(entry.actualS)}
            {delta ? ` (${delta})` : ''}
          </dd>
          <dt>{t('help.jobLog.col.result')}</dt>
          <dd>
            {t(`help.jobLog.result.${entry.result}` as I18nKey)}
            {entry.message ? ` · ${entry.message}` : ''}
          </dd>
          {entry.areaMm2 != null && (
            <>
              <dt>{t('help.jobLog.detail.area')}</dt>
              <dd>{fmt(entry.areaMm2 / 100, 1)} cm²</dd>
            </>
          )}
          {entry.lengthMm != null && (
            <>
              <dt>{t('help.jobLog.detail.length')}</dt>
              <dd>{fmt(entry.lengthMm / 10, 1)} cm</dd>
            </>
          )}
        </dl>
      </div>

      {entry.layers && entry.layers.length > 0 ? (
        <table className="joblog joblog--layers" aria-label={t('help.jobLog.detail.layers')}>
          <thead>
            <tr>
              <th>{t('help.jobLog.detail.layer')}</th>
              <th>{t('help.jobLog.detail.mode')}</th>
              <th>{t('help.jobLog.field.speed')}</th>
              <th>{t('help.jobLog.field.power')}</th>
              <th>{t('help.jobLog.field.passes')}</th>
              <th>{t('help.jobLog.field.interval')}</th>
              <th>{t('help.jobLog.col.material')}</th>
            </tr>
          </thead>
          <tbody>
            {entry.layers.map((l) => (
              <tr key={l.color}>
                <td>
                  <span className="mc-swatch" style={{ background: l.color }} aria-hidden="true" /> {l.name || l.color}
                </td>
                <td>{MODE_KEYS[l.mode] ? t(MODE_KEYS[l.mode]!) : l.mode}</td>
                <td className="num">{num(l.settings.speed)}</td>
                <td className="num">{num(l.settings.power, 1)} %</td>
                <td className="num">{num(l.settings.passes)}</td>
                <td className="num">{l.mode === 'line' ? '—' : `${num(l.settings.interval, 3)} mm`}</td>
                <td>{l.material ?? '—'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <p className="hint">{t('help.jobLog.detail.noLayers')}</p>
      )}

      <label className="field joblog__notes">
        <span className="field__label">{t('help.jobLog.notes')}</span>
        <textarea className="input" rows={3} maxLength={2000} value={notes} placeholder={t('help.jobLog.notes.placeholder')} onChange={(e) => setNotes(e.target.value)} onBlur={saveNotes} />
      </label>

      <div className="joblog__actions">
        <button
          type="button"
          className="btn btn--accent"
          disabled={!canRepeat}
          title={canRepeat ? t('help.jobLog.repeat.hint') : t('help.jobLog.repeat.noLayers')}
          onClick={() => {
            void repeatWithSettings(entry).then((applied) => applied && onRepeated());
          }}
        >
          <Copy size={14} aria-hidden="true" />
          {t('help.jobLog.repeat')}
        </button>
        <button type="button" className="btn" disabled={!entry.documentPath} onClick={() => entry.documentPath && void openProject(entry.documentPath)}>
          <FolderOpen size={14} aria-hidden="true" />
          {t('help.jobLog.openDoc')}
        </button>
        <button type="button" className="btn" disabled={entry.result === 'running'} onClick={() => void onDelete()}>
          <Trash2 size={14} aria-hidden="true" />
          {t('help.jobLog.delete')}
        </button>
      </div>
    </section>
  );
}
