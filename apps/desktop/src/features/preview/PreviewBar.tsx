import { Calculator, RotateCw, Settings2, TriangleAlert, X } from 'lucide-react';
import { useMemo } from 'react';
import { useCommand } from '@/commands/useCommand';
import { SelectionOptions } from '@/features/machine/SelectionOptions';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { cleanErrorMessage, previewErrorHint } from './errors';
import { getModel } from './model';
import { LayerList, Notes, Stats } from './Results';
import { Transport } from './Transport';
import { usePlayback } from './usePlayback';
import './preview.css';

function Legend() {
  const view = useStore((s) => s.previewView);
  const setView = useStore((s) => s.setPreviewView);
  const items = [
    { key: 'travel', label: t('preview.legend.travel'), on: view.showTravel, toggle: () => setView({ showTravel: !view.showTravel }) },
    { key: 'cut', label: t('preview.legend.cut') },
    { key: 'engrave', label: t('preview.legend.engrave') },
    { key: 'overscan', label: t('preview.legend.overscan'), on: view.showOverscan, toggle: () => setView({ showOverscan: !view.showOverscan }) },
  ] as const;
  return (
    <div className="pv__legend" role="group" aria-label={t('preview.legend.title')} title={t('preview.legend.power')}>
      {items.map((it) =>
        'toggle' in it ? (
          <button
            key={it.key}
            type="button"
            className="pv__chip"
            aria-pressed={it.on}
            title={it.on ? t('preview.legend.hide', { name: it.label }) : t('preview.legend.show', { name: it.label })}
            onClick={it.toggle}
          >
            <span className={`pv__swatch pv__swatch--${it.key}`} aria-hidden="true" />
            {it.label}
          </button>
        ) : (
          <span key={it.key} className="pv__chip pv__chip--static">
            <span className={`pv__swatch pv__swatch--${it.key}`} aria-hidden="true" />
            {it.label}
          </span>
        ),
      )}
      <span className="pv__power" aria-hidden="true" />
      <span className="hint">{t('preview.legend.power')}</span>
    </div>
  );
}

function ErrorBlock({ message, code }: { message: string; code: string | null }) {
  const compute = useStore((s) => s.computePreview);
  const clean = cleanErrorMessage(message) || t('preview.error.generic');
  const hint = previewErrorHint(code);
  return (
    <div className="pv__error" role="alert">
      <TriangleAlert size={16} aria-hidden="true" />
      <div>
        <p className="pv__error-title">{t('preview.error.title')}</p>
        <p>{clean}</p>
        {hint && <p className="hint">{t(hint)}</p>}
      </div>
      <button type="button" className="btn" onClick={() => void compute()}>
        <RotateCw size={14} aria-hidden="true" />
        {t('preview.error.retry')}
      </button>
    </div>
  );
}

/** Panel bajo el lienzo: reproducción del recorrido, leyenda, resultados del cálculo y advertencias. */
export function PreviewBar() {
  usePlayback();
  const preview = useStore((s) => s.preview);
  const status = useStore((s) => s.previewStatus);
  const stale = useStore((s) => s.previewStale);
  const error = useStore((s) => s.previewError);
  const errorCodeValue = useStore((s) => s.previewErrorCode);
  const hasLayers = useStore((s) => s.doc.layers.length > 0);
  const toggle = useCommand('view.preview');
  const optimization = useCommand('preview.optimization');
  const costs = useCommand('costs.open');

  const model = useMemo(() => (preview ? getModel(preview) : null), [preview?.moves]); // eslint-disable-line react-hooks/exhaustive-deps
  const layerNames = useMemo(() => preview?.layers.map((l) => l.summary) ?? [], [preview?.layers]);
  const busy = status === 'computing' || (stale && status !== 'error');

  return (
    <section className="pv" aria-label={t('preview.title')} aria-busy={busy}>
      <div className="pv__head">
        <h2 className="pv__title">{t('preview.title')}</h2>
        {busy && <span className="hint" role="status">{preview ? t('preview.updating') : t('preview.computing')}</span>}
        <button type="button" className="btn pv__opt" onClick={optimization.run} title={optimization.tooltip}>
          <Settings2 size={14} aria-hidden="true" />{optimization.label}
        </button>
        <button type="button" className="btn pv__opt" onClick={costs.run} title={costs.tooltip}>
          <Calculator size={14} aria-hidden="true" />{costs.label}
        </button>
        <button type="button" className="icon-btn" onClick={toggle.run} title={toggle.tooltip} aria-label={t('preview.close')}>
          <X size={15} aria-hidden="true" />
        </button>
      </div>
      <SelectionOptions compact />
      {status === 'error' && <ErrorBlock message={error ?? ''} code={errorCodeValue} />}
      {preview && model && (
        <>
          {model.count > 0 && <Transport model={model} layerNames={layerNames} />}
          <Legend />
          <div className="pv__body">
            <Stats model={model} totalS={preview.total_s} />
            <LayerList layers={preview.layers} model={model} totalS={preview.total_s} />
          </div>
          <Notes orderNote={preview.order_note} warnings={preview.warnings} />
        </>
      )}
      {!preview && status === 'idle' && !hasLayers && <p className="hint">{t('preview.empty')}</p>}
    </section>
  );
}
