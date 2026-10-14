import { CircleAlert, CircleCheck, Info, LoaderCircle, RefreshCw, ShieldCheck, TriangleAlert, X } from 'lucide-react';
import { useId } from 'react';
import type { Document } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { canFix, fixFinding, fixSafe, isSafe, selectFinding } from './fixes';
import { closeReview, useReviewUi } from './store';
import { findingTitle, findingWhy } from './text';
import type { Finding, Severity } from './types';
import { useReview } from './useReview';
import './review.css';

const SEVERITY_ICON: Record<Severity, typeof Info> = { serious: CircleAlert, warning: TriangleAlert, info: Info };

/** Un hallazgo: qué es, por qué importa y sus botones. */
function FindingRow({ finding, doc, busy, ignored }: { finding: Finding; doc: Pick<Document, 'layers'>; busy: boolean; ignored: boolean }) {
  const ui = useReviewUi.getState();
  const Icon = SEVERITY_ICON[finding.severity];
  const info = finding.severity === 'info' && !canFix(finding);
  return (
    <li className="rv-item" data-severity={finding.severity} data-kind={finding.kind} data-ignored={ignored || undefined}>
      <span className="rv-item__icon" aria-hidden="true">
        <Icon size={16} />
      </span>
      <div className="rv-item__body">
        <h3 className="rv-item__title">
          {findingTitle(finding, doc)}
          <span className="rv-item__sev">{t(`review.sev.${finding.severity}`)}</span>
        </h3>
        <p className="rv-item__why">{findingWhy(finding, doc)}</p>
        {info && <p className="hint">{t('review.onlyNotice')}</p>}
        <div className="rv-item__actions">
          {!ignored && canFix(finding) && (
            <button type="button" className="btn btn--accent" disabled={busy} onClick={() => void fixFinding(finding)}>
              {t(`review.fix.${finding.kind}` as I18nKey)}
            </button>
          )}
          <button type="button" className="btn" onClick={() => selectFinding(finding)}>
            {t('review.btn.select')}
          </button>
          {ignored ? (
            <button type="button" className="btn" onClick={() => ui.restore(finding.key)}>
              {t('review.btn.restore')}
            </button>
          ) : (
            <button type="button" className="btn" onClick={() => ui.ignore(finding.key)}>
              {t('review.btn.ignore')}
            </button>
          )}
        </div>
      </div>
    </li>
  );
}

/**
 * Panel «Revisar diseño»: lista de hallazgos con «Arreglar», «Seleccionar» e «Ignorar», y «Arreglar
 * todo lo seguro». Flota sobre el lienzo (no es modal) para poder seleccionar y mirar el diseño.
 * Se actualiza solo cuando el diseño cambia.
 */
export function ReviewPanel() {
  const titleId = useId();
  const doc = useStore((s) => s.doc);
  const { findings, status, error, refresh } = useReview(true);
  const ignoredKeys = useReviewUi((s) => s.ignored);
  const showIgnored = useReviewUi((s) => s.showIgnored);
  const busy = useReviewUi((s) => s.busy);

  const active = findings.filter((f) => !ignoredKeys.includes(f.key));
  const hidden = findings.filter((f) => ignoredKeys.includes(f.key));
  const safe = active.filter(isSafe);
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') closeReview();
  };

  return (
    <section className="rv-panel" role="dialog" aria-labelledby={titleId} onKeyDown={onKeyDown}>
      <header className="rv-head">
        <h2 id={titleId} className="panel-title">
          <ShieldCheck size={15} aria-hidden="true" />
          {t('review.title')}
        </h2>
        <div className="rv-head__tools">
          <button type="button" className="icon-btn" aria-label={t('review.rescan')} title={t('review.rescan')} onClick={refresh}>
            <RefreshCw size={15} aria-hidden="true" />
          </button>
          <button type="button" className="icon-btn" aria-label={t('review.close')} title={t('review.close')} onClick={closeReview}>
            <X size={16} aria-hidden="true" />
          </button>
        </div>
      </header>

      <div className="rv-body" aria-busy={status === 'running'}>
        {status === 'running' && (
          <p className="hint" role="status">
            <LoaderCircle size={14} className="rv-spin" aria-hidden="true" /> {t('review.running')}
          </p>
        )}
        {status === 'error' && (
          <p className="hint hint--danger" role="alert">
            {t('review.error', { message: error ?? '' })}
          </p>
        )}
        {status !== 'running' && status !== 'error' && active.length === 0 && (
          <div className="rv-ok" role="status">
            <CircleCheck size={28} aria-hidden="true" />
            <strong>{t('review.allGood')}</strong>
            <p className="hint">{hidden.length > 0 ? t('review.allGood.ignored', { n: hidden.length }) : t('review.allGood.help')}</p>
          </div>
        )}
        {active.length > 0 && (
          <>
            <p className="rv-summary" role="status">
              {active.length === 1 ? t('review.count.one') : t('review.count.other', { n: active.length })}
            </p>
            <p className="hint">{t('review.intro')}</p>
            <div className="rv-safe">
              <button type="button" className="btn btn--accent" disabled={busy || safe.length === 0} onClick={() => void fixSafe(active)}>
                <ShieldCheck size={14} aria-hidden="true" />
                {t('review.fixSafe')}
              </button>
              <p className="hint">{safe.length > 0 ? t('review.fixSafe.help') : t('review.fixSafe.none')}</p>
            </div>
            <ul className="rv-list">
              {active.map((f) => (
                <FindingRow key={f.key} finding={f} doc={doc} busy={busy} ignored={false} />
              ))}
            </ul>
          </>
        )}
        {hidden.length > 0 && (
          <div className="rv-ignored">
            <button type="button" className="btn" aria-expanded={showIgnored} onClick={() => useReviewUi.getState().set({ showIgnored: !showIgnored })}>
              {t('review.ignored', { n: hidden.length })}
            </button>
            {showIgnored && (
              <ul className="rv-list">
                {hidden.map((f) => (
                  <FindingRow key={f.key} finding={f} doc={doc} busy={busy} ignored />
                ))}
              </ul>
            )}
          </div>
        )}
      </div>
    </section>
  );
}
