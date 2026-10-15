import { ArrowRight, LoaderCircle, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import type { FontAxisInfo, FontCapabilities, FontFeatureInfo, TextObject } from '@/api/types';
import { t } from '@/i18n';
import { NumberField } from '@/ui/NumberField';
import {
  axisLabel,
  axisStep,
  axisValue,
  changedFeatureCount,
  featureLabel,
  featureOn,
  groupFeatures,
  groupLabel,
  matchingInstance,
  NUMBERED_FEATURES,
  storedFeature,
  variationsOfInstance,
  withAxis,
  withFeature,
} from './advanced';
import { useFontCapabilities } from './capabilities';
import { FeaturePreview, previewBox } from './FeaturePreview';

type Change = (patch: Partial<TextObject>, field: string) => void;

interface Props {
  kind: TextObject;
  /** Familia que se manda al motor (ya con el reemplazo si la pedida no está instalada). */
  family: string;
  /** Fuente de una línea: no tiene funciones ni ejes. */
  lineFont: boolean;
  onChange: Change;
}

/**
 * Funciones OpenType y ejes de fuente variable del texto. Solo ofrece lo que la fuente declara
 * (lo lee el motor de sus tablas GSUB, GPOS y fvar) y cada función muestra una vista previa
 * apagada y encendida con letras que sí cambian. Se pide la información al abrir la sección.
 */
export function OpenTypeSection({ kind, family, lineFont, onChange }: Props) {
  const [open, setOpen] = useState(false);
  const { caps, loading, error } = useFontCapabilities(kind, family, open && !lineFont);
  const active = changedFeatureCount(kind.features) + (kind.variations?.length ?? 0);

  return (
    <details className="ot" open={open}>
      {/* El estado lo lleva React (no el navegador), así también funciona donde no existe el evento `toggle`. */}
      <summary
        className="ot__summary"
        onClick={(e) => {
          e.preventDefault();
          setOpen((o) => !o);
        }}
      >
        {t('text.ot.section')}
        {active > 0 && <span className="ot__count">{t('text.ot.changed', { n: active })}</span>}
      </summary>
      {open && (
        <div className="ot__body">
          {lineFont ? (
            <p className="hint">{t('text.ot.lineFont')}</p>
          ) : (
            <>
              {error && (
                <p className="warn-line" role="alert">
                  <TriangleAlert size={14} aria-hidden="true" />
                  <span>{t('text.ot.error', { message: error })}</span>
                </p>
              )}
              {loading && !caps && (
                <p className="hint ot__loading" role="status">
                  <LoaderCircle size={13} className="text-panel__spin" aria-hidden="true" />
                  {t('text.ot.loading')}
                </p>
              )}
              {caps && <Body kind={kind} caps={caps} onChange={onChange} />}
            </>
          )}
        </div>
      )}
    </details>
  );
}

function Body({ kind, caps, onChange }: { kind: TextObject; caps: FontCapabilities; onChange: Change }) {
  const groups = groupFeatures(caps.features);
  const empty = groups.length === 0 && caps.axes.length === 0;
  return (
    <>
      {empty && <p className="hint">{t('text.ot.none')}</p>}
      {caps.axes.length > 0 && <Axes kind={kind} caps={caps} onChange={onChange} />}
      {groups.map(({ group, items }) => (
        <fieldset key={group} className="ot__group">
          <legend className="ot__legend">{groupLabel(group)}</legend>
          <ul className="ot__list">
            {items.map((f) => (
              <FeatureRow key={f.tag} kind={kind} feature={f} onChange={onChange} />
            ))}
          </ul>
        </fieldset>
      ))}
      {(kind.features?.length ?? 0) > 0 && (
        <button type="button" className="btn" onClick={() => onChange({ features: undefined }, 'ot-reset')}>
          {t('text.ot.reset')}
        </button>
      )}
      {groups.length > 0 && <p className="hint ot__hint">{t('text.ot.hint')}</p>}
    </>
  );
}

function FeatureRow({ kind, feature, onChange }: { kind: TextObject; feature: FontFeatureInfo; onChange: Change }) {
  const on = featureOn(kind.features, feature.tag, feature.default_on);
  const alt = storedFeature(kind.features, feature.tag) ?? 1;
  const box = previewBox(feature.on, feature.off);
  const set = (next: boolean, value?: number) =>
    onChange({ features: withFeature(kind.features, feature.tag, next, feature.default_on, value) }, `ot-${feature.tag}`);
  return (
    <li className="ot-row">
      <label className="ot-row__main">
        <input type="checkbox" checked={on} onChange={(e) => set(e.target.checked, alt)} />
        <span className="ot-row__name">{featureLabel(feature.tag)}</span>
        <code className="ot-row__tag">{feature.tag}</code>
      </label>
      <span className="ot-row__previews">
        <FeaturePreview paths={feature.off} box={box} title={t('text.ot.previewOff')} muted />
        <ArrowRight size={12} aria-hidden="true" />
        <FeaturePreview paths={feature.on} box={box} title={t('text.ot.previewOn')} />
      </span>
      {on && NUMBERED_FEATURES.includes(feature.tag) && (
        <NumberField label={t('text.ot.alternative', { tag: feature.tag })} value={alt} min={1} max={20} step={1} onCommit={(v) => set(true, v)} />
      )}
      {!feature.changes && <p className="hint ot-row__note">{t('text.ot.noEffect')}</p>}
    </li>
  );
}

function Axes({ kind, caps, onChange }: { kind: TextObject; caps: FontCapabilities; onChange: Change }) {
  const current = matchingInstance(kind.variations, caps.instances, caps.axes);
  return (
    <fieldset className="ot__group">
      <legend className="ot__legend">{t('text.var.section')}</legend>
      <p className="hint ot__hint">{t('text.var.hint')}</p>
      {caps.instances.length > 0 && (
        <div className="field">
          <label className="field__label" htmlFor="ot-instance">{t('text.var.instance')}</label>
          <select
            id="ot-instance"
            className="select"
            value={current ? current.name : ''}
            onChange={(e) => {
              const inst = caps.instances.find((i) => i.name === e.target.value);
              if (inst) onChange({ variations: variationsOfInstance(inst, caps.axes) }, 'ot-instance');
            }}
          >
            {!current && <option value="">{t('text.var.custom')}</option>}
            {caps.instances.map((i) => (
              <option key={i.name} value={i.name}>{i.name}</option>
            ))}
          </select>
        </div>
      )}
      {caps.axes.map((axis) => (
        <AxisSlider key={axis.tag} axis={axis} value={axisValue(kind.variations, axis)} onValue={(v) => onChange({ variations: withAxis(kind.variations, axis, v) }, `ot-axis-${axis.tag}`)} />
      ))}
      {(kind.variations?.length ?? 0) > 0 && (
        <button type="button" className="btn" onClick={() => onChange({ variations: undefined }, 'ot-axes-reset')}>
          {t('text.var.reset')}
        </button>
      )}
    </fieldset>
  );
}

function AxisSlider({ axis, value, onValue }: { axis: FontAxisInfo; value: number; onValue: (v: number) => void }) {
  const label = axisLabel(axis);
  const shown = String(Math.round(value * 100) / 100);
  return (
    <div className="ot-axis">
      <label className="field__label" htmlFor={`ot-axis-${axis.tag}`}>
        {t('text.var.axisValue', { axis: label, value: shown })} <code className="ot-row__tag">{axis.tag}</code>
      </label>
      <input
        id={`ot-axis-${axis.tag}`}
        type="range"
        className="ot-axis__range"
        min={axis.min}
        max={axis.max}
        step={axisStep(axis)}
        value={value}
        aria-valuetext={shown}
        onChange={(e) => onValue(Number(e.target.value))}
      />
    </div>
  );
}
