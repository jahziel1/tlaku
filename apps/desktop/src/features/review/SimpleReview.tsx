import { CircleAlert, TriangleAlert } from 'lucide-react';
import type { Document } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { canFix, fixFinding, selectFinding } from './fixes';
import { useReviewUi } from './store';
import { findingSimple } from './text';
import type { Finding, FindingKind } from './types';
import { useReview } from './useReview';

/**
 * Qué hallazgos se muestran en el modo sencillo. Se dejan fuera las capas sin salida (ahí «Ignorar»
 * un color es una decisión del usuario) y lo que el modo sencillo ya avisa por su cuenta.
 */
export function simpleFindings(findings: readonly Finding[], alreadyShown: readonly string[]): Finding[] {
  const skip = new Set<FindingKind>(['silent_layer']);
  // `validate` ya avisa de los textos sin convertir y de lo que se sale de la cama.
  if (alreadyShown.includes('texts')) skip.add('text');
  if (alreadyShown.includes('outside')) {
    skip.add('outside_all');
    skip.add('outside_part');
  }
  return findings.filter((f) => !skip.has(f.kind));
}

function Row({ finding, doc, busy }: { finding: Finding; doc: Pick<Document, 'layers'>; busy: boolean }) {
  const serious = finding.severity === 'serious';
  const fixable = canFix(finding);
  return (
    <li className="simple__problem" data-level={serious ? 'block' : 'warn'} data-problem={`review:${finding.kind}`}>
      <span className="simple__problem-icon" aria-hidden="true">
        {serious ? <CircleAlert size={18} /> : <TriangleAlert size={18} />}
      </span>
      <div className="simple__problem-body">
        <p>{findingSimple(finding, doc)}</p>
        <div className="simple__row">
          {fixable && (
            <button type="button" className="btn simple__btn" disabled={busy} onClick={() => void fixFinding(finding)}>
              {t(`review.simple.fix.${finding.kind}` as I18nKey)}
            </button>
          )}
          <button type="button" className="btn simple__btn" onClick={() => selectFinding(finding)}>
            {t('review.simple.select')}
          </button>
        </div>
      </div>
    </li>
  );
}

/** Los hallazgos de «Revisar diseño» en el paso Revisar del modo sencillo, con la misma apariencia que los demás avisos. */
export function SimpleReview({ shown }: { shown: readonly string[] }) {
  const doc = useStore((s) => s.doc);
  const busy = useReviewUi((s) => s.busy);
  const { findings, status } = useReview(true);
  const list = simpleFindings(findings, shown);
  if (status !== 'ready' || list.length === 0) return null;
  return (
    <ul className="simple__problems" aria-label={t('review.title')}>
      {list.map((f) => (
        <Row key={f.key} finding={f} doc={doc} busy={busy} />
      ))}
    </ul>
  );
}
