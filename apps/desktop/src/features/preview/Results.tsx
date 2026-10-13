import { useState } from 'react';
import { ChevronRight, Crosshair, TriangleAlert } from 'lucide-react';
import { t, type I18nKey } from '@/i18n';
import { fmt, fmtClock } from '@/lib/platform';
import { useStore } from '@/store';
import type { PreviewLayer } from '@/bindings/PreviewLayer';
import { fmtDistance, fmtSeconds } from './format';
import type { Distances, PreviewModel } from './model';

const ROWS = [
  ['cut', 'preview.legend.cut'],
  ['engrave', 'preview.legend.engrave'],
  ['travel', 'preview.legend.travel'],
  ['overscan', 'preview.legend.overscan'],
] as const satisfies readonly (readonly [keyof Distances, I18nKey])[];

function DistanceRows({ distances, approximate }: { distances: Distances; approximate: boolean }) {
  return (
    <>
      {ROWS.map(([key, label]) =>
        distances[key].mm > 0 ? (
          <div key={key} className="pv__row">
            <dt><span className={`pv__swatch pv__swatch--${key}`} aria-hidden="true" />{t(label)}</dt>
            <dd className="num">
              {approximate && '≈ '}
              {t('preview.stats.distance', { dist: fmtDistance(distances[key].mm), time: fmtSeconds(distances[key].seconds) })}
            </dd>
          </div>
        ) : null,
      )}
    </>
  );
}

/** Tiempo total, distancias y tiempos por tipo de movimiento. */
export function Stats({ model, totalS }: { model: PreviewModel; totalS: number }) {
  return (
    <div className="pv__stats">
      <h3 className="pv__h">{t('preview.stats.title')}</h3>
      <div className="pv__total">
        <span className="pv__total-label">{t('preview.stats.total')}</span>
        <span className="num pv__total-value">{fmtClock(totalS)}</span>
        <span className="pv__badge" title={t('preview.stats.estimate')}>{t('preview.stats.estimateBadge')}</span>
      </div>
      <p className="hint">{t('preview.stats.estimate')}</p>
      {model.approximate && <p className="hint hint--warn">{t('preview.time.approx')}</p>}
      <dl className="pv__dl">
        <DistanceRows distances={model.distances} approximate={model.approximate} />
      </dl>
    </div>
  );
}

function LayerItem({ layer, index, model, totalS }: { layer: PreviewLayer; index: number; model: PreviewModel; totalS: number }) {
  const [open, setOpen] = useState(false);
  const highlighted = useStore((s) => s.previewView.highlightLayer === index);
  const setView = useStore((s) => s.setPreviewView);
  // Los límites de capa solo son fiables con los tiempos exactos del motor.
  const canHighlight = model.layerStarts !== null && !model.approximate;
  const toggleHighlight = () => setView({ highlightLayer: highlighted ? null : index });
  const share = totalS > 0 ? Math.min(100, (layer.seconds / totalS) * 100) : 0;
  const dist = model.approximate ? undefined : model.layerDistances?.[index];
  const modeKey = `preview.mode.${layer.mode}` as I18nKey;
  const modeLabel = layer.mode === 'line' || layer.mode === 'fill' || layer.mode === 'image' ? t(modeKey) : layer.mode;
  const detailsId = `pv-layer-${index}`;

  return (
    <li className="pv__layer" data-highlighted={highlighted}>
      <div className="pv__layer-head">
        <button
          type="button"
          className="pv__layer-toggle"
          aria-expanded={open}
          aria-controls={detailsId}
          title={open ? t('preview.layer.collapse') : t('preview.layer.expand')}
          onClick={() => setOpen((v) => !v)}
        >
          <ChevronRight size={14} className="pv__chev" aria-hidden="true" />
          <span className="dot" style={{ background: layer.color }} aria-hidden="true" />
          <span className="pv__layer-summary" title={layer.summary}>{layer.summary}</span>
        </button>
        <span className="pv__mode">{modeLabel}</span>
        <span className="num pv__layer-time" title={t('preview.layer.share', { pct: fmt(share, 0) })}>{fmtSeconds(layer.seconds)}</span>
        {canHighlight && (
          <button
            type="button"
            className="icon-btn"
            aria-pressed={highlighted}
            title={highlighted ? t('preview.layer.unhighlight') : t('preview.layer.highlight')}
            aria-label={highlighted ? t('preview.layer.unhighlight') : t('preview.layer.highlight')}
            onClick={toggleHighlight}
          >
            <Crosshair size={15} aria-hidden="true" />
          </button>
        )}
      </div>
      <div className="pv__bar" aria-hidden="true"><span style={{ width: `${share}%`, background: layer.color }} /></div>
      {layer.travel_saved_pct !== null && layer.travel_saved_pct > 0 && (
        <p className="pv__saved">{t('preview.layer.saved', { pct: fmt(layer.travel_saved_pct, 0) })}</p>
      )}
      {open && (
        <div id={detailsId} className="pv__details">
          {dist && (
            <dl className="pv__dl">
              <DistanceRows distances={dist} approximate={false} />
            </dl>
          )}
          {layer.explanations.length === 0 ? (
            <p className="hint">{t('preview.layer.noExplain')}</p>
          ) : (
            <ul className="pv__explain">
              {layer.explanations.map((text) => (
                <li key={text}>
                  {canHighlight ? (
                    <button type="button" className="pv__explain-btn" title={t('preview.layer.clickToHighlight')} onClick={toggleHighlight}>
                      {text}
                    </button>
                  ) : (
                    text
                  )}
                </li>
              ))}
            </ul>
          )}
          {canHighlight && <p className="hint">{t('preview.layer.noFigures')}</p>}
        </div>
      )}
    </li>
  );
}

export function LayerList({ layers, model, totalS }: { layers: PreviewLayer[]; model: PreviewModel; totalS: number }) {
  if (layers.length === 0) return null;
  return (
    <div className="pv__layers">
      <h3 className="pv__h">{t('preview.layers.title')}</h3>
      <ul className="pv__layer-list">
        {layers.map((l, i) => (
          <LayerItem key={`${l.color}-${i}`} layer={l} index={i} model={model} totalS={totalS} />
        ))}
      </ul>
    </div>
  );
}

export function Notes({ orderNote, warnings }: { orderNote: string | null; warnings: string[] }) {
  return (
    <>
      {orderNote && (
        <div className="pv__note">
          <h3 className="pv__h">{t('preview.order.title')}</h3>
          <p className="hint">{orderNote}</p>
        </div>
      )}
      {warnings.length > 0 && (
        <div className="pv__warnings" role="status" aria-label={t('preview.warnings.title')}>
          {warnings.map((w) => (
            <p key={w} className="warn-line"><TriangleAlert size={14} aria-hidden="true" />{w}</p>
          ))}
        </div>
      )}
    </>
  );
}
