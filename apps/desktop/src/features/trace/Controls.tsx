import { RotateCcw } from 'lucide-react';
import { useId } from 'react';
import type { ImageObject, TraceSettings } from '@/api/types';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { speckleMm, TRACE_LIMITS } from './settings';

interface SliderProps {
  label: string;
  value: number;
  valueText: string;
  min: number;
  max: number;
  step: number;
  help?: string;
  disabled?: boolean;
  autoFocus?: boolean;
  onChange: (v: number) => void;
}

function Slider({ label, value, valueText, min, max, step, help, disabled, autoFocus, onChange }: SliderProps) {
  const id = useId();
  return (
    <div className="tr-field" data-disabled={disabled || undefined}>
      <div className="tr-field__row">
        <label className="tr-field__label" htmlFor={id}>{label}</label>
        <span className="tr-field__value num" aria-hidden="true">{valueText}</span>
      </div>
      <input
        id={id}
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        data-autofocus={autoFocus ? '' : undefined}
        aria-valuetext={valueText}
        aria-describedby={help ? `${id}-help` : undefined}
        onChange={(e) => onChange(Number(e.target.value))}
      />
      {help && <p id={`${id}-help`} className="hint">{help}</p>}
    </div>
  );
}

interface Props {
  image: Pick<ImageObject, 'size_mm' | 'width_px'>;
  settings: TraceSettings;
  onChange: (patch: Partial<TraceSettings>) => void;
  onReset: () => void;
}

/** Deslizadores y opciones del trazado. Cada cambio dispara un recálculo (con debounce). */
export function Controls({ image, settings, onChange, onReset }: Props) {
  const invertId = useId();
  const modeName = useId();
  const modeLabel = useId();
  const polygon = settings.mode === 'polygon';
  const mm = speckleMm(image, settings.speckle_px);
  return (
    <div className="tr-side">
      <Slider
        label={t('trace.set.threshold')}
        value={settings.threshold}
        valueText={`${Math.round(settings.threshold * 100)} %`}
        min={0}
        max={1}
        step={0.01}
        help={t('trace.set.threshold.help')}
        autoFocus
        onChange={(threshold) => onChange({ threshold })}
      />
      <Slider
        label={t('trace.set.smoothing')}
        value={settings.smoothing}
        valueText={`${Math.round(settings.smoothing * 100)} %`}
        min={0}
        max={1}
        step={0.05}
        help={t('trace.set.smoothing.help')}
        onChange={(smoothing) => onChange({ smoothing })}
      />
      <Slider
        label={t('trace.set.speckle')}
        value={settings.speckle_px}
        valueText={
          settings.speckle_px <= 1
            ? t('trace.set.speckle.off')
            : t('trace.set.speckle.value', { px: settings.speckle_px, mm: fmt(mm, mm < 1 ? 2 : 1) })
        }
        min={TRACE_LIMITS.speckle_px.min}
        max={TRACE_LIMITS.speckle_px.max}
        step={1}
        help={t('trace.set.speckle.help')}
        onChange={(speckle_px) => onChange({ speckle_px })}
      />
      <Slider
        label={t('trace.set.corner')}
        value={settings.corner_threshold_deg}
        valueText={`${settings.corner_threshold_deg}°`}
        min={TRACE_LIMITS.corner_threshold_deg.min}
        max={TRACE_LIMITS.corner_threshold_deg.max}
        step={5}
        disabled={polygon}
        help={polygon ? t('trace.set.corner.polygonOnly') : t('trace.set.corner.help')}
        onChange={(corner_threshold_deg) => onChange({ corner_threshold_deg })}
      />
      <Slider
        label={t('trace.set.blur')}
        value={settings.blur_px}
        valueText={`${fmt(settings.blur_px, 1)} px`}
        min={TRACE_LIMITS.blur_px.min}
        max={TRACE_LIMITS.blur_px.max}
        step={0.1}
        help={t('trace.set.blur.help')}
        onChange={(blur_px) => onChange({ blur_px })}
      />

      <div className="tr-field">
        <span id={modeLabel} className="tr-field__label">{t('trace.set.mode')}</span>
        <div className="tr-seg" role="radiogroup" aria-labelledby={modeLabel}>
          {(['spline', 'polygon'] as const).map((m) => (
            <label key={m}>
              <input type="radio" name={modeName} checked={settings.mode === m} onChange={() => onChange({ mode: m })} />
              <span>{t(`trace.mode.${m}`)}</span>
            </label>
          ))}
        </div>
      </div>

      <label className="tr-check" htmlFor={invertId}>
        <input id={invertId} type="checkbox" checked={settings.invert} onChange={(e) => onChange({ invert: e.target.checked })} />
        <span>{t('trace.set.invert')}</span>
      </label>

      <button type="button" className="btn" onClick={onReset}>
        <RotateCcw size={13} aria-hidden="true" />
        {t('trace.set.reset')}
      </button>
    </div>
  );
}
