import { TriangleAlert, X } from 'lucide-react';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';
import { fmtClock } from '@/lib/platform';
import { useStore } from '@/store';

/** Franja bajo el lienzo con la reproducción del recorrido, el tiempo por capa y las advertencias. */
export function PreviewBar() {
  const preview = useStore((s) => s.preview);
  const status = useStore((s) => s.previewStatus);
  const error = useStore((s) => s.previewError);
  const scrub = useStore((s) => s.scrub);
  const setScrub = useStore((s) => s.setScrub);
  const layers = useStore((s) => s.doc.layers);
  const toggle = useCommand('view.preview');

  const total = preview?.total_s ?? 0;
  return (
    <section className="preview-bar" aria-label={t('preview.title')}>
      <div className="preview-bar__head">
        <h2 className="preview-bar__title">{t('preview.title')}</h2>
        {status === 'computing' && <span className="hint">{t('preview.computing')}</span>}
        {status === 'error' && <span className="hint hint--danger">{error}</span>}
        <button type="button" className="icon-btn" onClick={toggle.run} title={t('preview.close')} aria-label={t('preview.close')}>
          <X size={15} aria-hidden="true" />
        </button>
      </div>
      {preview && (
        <>
          <div className="preview-bar__scrub">
            <label htmlFor="scrub" className="sr-only">{t('preview.scrub')}</label>
            <input
              id="scrub"
              className="range"
              type="range"
              min={0}
              max={1000}
              value={Math.round(scrub * 1000)}
              onChange={(e) => setScrub(Number(e.target.value) / 1000)}
            />
            <span className="num preview-bar__time">{fmtClock(total * scrub)} / {fmtClock(total)}</span>
          </div>
          <ul className="preview-bar__layers">
            {preview.layers.map((l) => (
              <li key={l.color} title={l.explanations.join(' ')}>
                <span className="dot" style={{ background: l.color }} aria-hidden="true" />
                <span className="num">{fmtClock(l.seconds)}</span>
                <span className="preview-bar__summary">{l.summary}</span>
              </li>
            ))}
          </ul>
          {preview.order_note && <p className="hint">{preview.order_note}</p>}
          {preview.warnings.map((w) => (
            <p key={w} className="warn-line"><TriangleAlert size={14} aria-hidden="true" />{w}</p>
          ))}
        </>
      )}
      {!preview && status === 'idle' && layers.length === 0 && <p className="hint">{t('preview.empty')}</p>}
    </section>
  );
}
