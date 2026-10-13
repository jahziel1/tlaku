import { ArrowLeft, Image as ImageIcon, PaintBucket, Spline } from 'lucide-react';
import { useId, useRef, useEffect, useState, type KeyboardEvent } from 'react';
import type { ImageDitherMode, ImageSettings, LayerMode, LayerSettings } from '@/api/types';
import { t } from '@/i18n';
import { dpiToInterval, intervalToDpi } from '@/lib/layers';
import { colorName, isGuideColor, readableOn } from '@/lib/palette';
import { selectActiveDevice, useStore } from '@/store';
import { Group, NumField, SelectField, Toggle } from './ParamFields';
import { formatNumber } from './fields';
import { focusLayerRow, useLayerUi } from './layerUi';
import { orderConflict } from './order';

const MODES: { mode: LayerMode; icon: typeof Spline }[] = [
  { mode: 'line', icon: Spline },
  { mode: 'fill', icon: PaintBucket },
  { mode: 'image', icon: ImageIcon },
];
const DITHERS: ImageDitherMode[] = ['threshold', 'floyd', 'jarvis', 'stucki', 'atkinson', 'bayer4', 'grayscale'];
/** Overscan inicial al pasar de automático a manual. */
const MANUAL_OVERSCAN_MM = 2;

/**
 * Editor de ajustes de una capa. Muestra solo los campos que el modo usa (los demás se
 * conservan en el documento). Cada campo tiene su paso de deshacer; los valores inválidos se
 * marcan y no se aplican. Esc cierra el editor.
 */
export function LayerEditor({ layer }: { layer: LayerSettings }) {
  const color = layer.color;
  const guide = isGuideColor(color);
  const headRef = useRef<HTMLHeadingElement>(null);
  const showHelp = useLayerUi((s) => s.showHelp);
  const layerCount = useStore((s) => s.doc.layers.length);
  const allLayers = useStore((s) => s.doc.layers);
  const maxRate = useStore((s) => selectActiveDevice(s).max_rate[0]);
  const updateLayer = useStore((s) => s.updateLayer);
  const applyDocChange = useStore((s) => s.applyDocChange);

  useEffect(() => headRef.current?.focus(), []);

  const set = (patch: Partial<LayerSettings>) => updateLayer(color, patch);
  // Cada subcampo de imagen lleva su propia clave: un paso de deshacer por campo.
  const setImage = (key: keyof ImageSettings, value: ImageSettings[keyof ImageSettings]) =>
    applyDocChange(
      'history.editLayer',
      (doc) => ({
        ...doc,
        layers: doc.layers.map((l) => (l.color === color ? { ...l, image: { ...l.image, [key]: value } } : l)),
      }),
      `layer:${color}:image.${key}`,
    );

  const close = () => {
    useLayerUi.getState().closeEditor();
    focusLayerRow(color);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    }
  };

  const name = layer.name || colorName(color);
  const isImage = layer.mode === 'image';
  const usesRaster = layer.mode !== 'line';
  const img = layer.image;
  const pinned = layer.order !== null;

  return (
    <section className="lyr-editor panel-scroll" aria-label={t('layers.editor', { name })} onKeyDown={onKeyDown}>
      <header className="lyr-editor__head">
        <button type="button" className="icon-btn icon-btn--boxed" onClick={close} aria-label={t('layers.editorBack')} title={t('layers.editorBack')}>
          <ArrowLeft size={15} aria-hidden="true" />
        </button>
        <span className="lyr-swatch" data-guide={guide} style={{ '--sw': color, '--ink': readableOn(color) } as React.CSSProperties} aria-hidden="true" />
        <h3 ref={headRef} tabIndex={-1} className="lyr-editor__title">
          {name}
        </h3>
        <button
          type="button"
          className="btn"
          aria-pressed={showHelp}
          title={t('layers.helpToggleHint')}
          onClick={() => useLayerUi.getState().setShowHelp(!showHelp)}
        >
          {t('layers.helpToggle')}
        </button>
      </header>

      <Group title={t('layers.group.general')}>
        <NameField value={layer.name} fallback={colorName(color)} onApply={(v) => set({ name: v })} />
        {guide ? (
          <p className="lyr-info">{t('layers.guideInfo')}</p>
        ) : (
          <ModePicker mode={layer.mode} onChange={(mode) => set({ mode })} />
        )}
        <Toggle label={t('layers.visible')} checked={layer.visible} onChange={(v) => set({ visible: v })} help={t('layers.help.visible')} />
        <Toggle
          label={t('layers.output')}
          checked={layer.output && !guide}
          disabled={guide}
          reason={t('layers.outputGuide')}
          onChange={(v) => set({ output: v })}
          help={t('layers.help.output')}
        />
      </Group>

      {!guide && (
        <>
          <Group title={t('layers.group.power')}>
            <NumField
              field="speed"
              label={t('layers.speed')}
              value={layer.speed}
              onApply={(v) => set({ speed: v })}
              help={t('layers.help.speed')}
              live={layer.speed > maxRate ? t('layers.warn.speed', { max: maxRate }) : undefined}
            />
            <NumField
              field="power"
              label={isImage ? t('layers.powerMax') : t('layers.power')}
              value={layer.power}
              onApply={(v) => set({ power: v })}
              ctx={{ minPower: isImage ? layer.min_power : undefined }}
              help={isImage ? t('layers.help.powerImage') : t('layers.help.power')}
            />
            {isImage && (
              <NumField
                field="min_power"
                label={t('layers.powerMin')}
                value={layer.min_power}
                onApply={(v) => set({ min_power: v })}
                ctx={{ power: layer.power }}
                help={t('layers.help.minPower')}
              />
            )}
            <NumField field="passes" label={t('layers.passes')} value={layer.passes} onApply={(v) => set({ passes: v })} help={t('layers.help.passes')} />
            <Toggle label={t('layers.air')} checked={layer.air} onChange={(v) => set({ air: v })} help={t('layers.help.air')} />
          </Group>

          {layer.mode === 'line' && (
            <Group title={t('layers.group.line')}>
              <NumField field="kerf" label={t('layers.kerf')} value={layer.kerf} onApply={(v) => set({ kerf: v })} help={t('layers.help.kerf')} />
            </Group>
          )}

          {usesRaster && (
            <Group title={t('layers.group.scan')}>
              <NumField
                field="interval"
                label={t('layers.interval')}
                value={layer.interval}
                onApply={(v) => set({ interval: v })}
                help={t('layers.help.interval')}
                live={t('layers.intervalLive', {
                  interval: formatNumber(layer.interval, 4),
                  dpi: formatNumber(intervalToDpi(layer.interval), 1),
                  cm: formatNumber(10 / layer.interval, 1),
                })}
              />
              <NumField
                field="dpi"
                label={t('layers.dpi')}
                value={layer.interval}
                toView={intervalToDpi}
                onApply={(v) => set({ interval: dpiToInterval(v) })}
                help={t('layers.help.dpi')}
              />
              {layer.mode === 'fill' && (
                <>
                  <NumField field="angle" label={t('layers.angle')} value={layer.angle} onApply={(v) => set({ angle: v })} help={t('layers.help.angle')} />
                  <Toggle label={t('layers.crosshatch')} checked={layer.crosshatch} onChange={(v) => set({ crosshatch: v })} help={t('layers.help.crosshatch')} />
                </>
              )}
              <Toggle label={t('layers.bidirectional')} checked={layer.bidirectional} onChange={(v) => set({ bidirectional: v })} help={t('layers.help.bidirectional')} />
              <OverscanField
                value={layer.overscan}
                onAuto={(auto) => set({ overscan: auto ? 'auto' : MANUAL_OVERSCAN_MM })}
                onMm={(v) => set({ overscan: v })}
              />
            </Group>
          )}

          {isImage && (
            <Group title={t('layers.group.image')}>
              <SelectField<ImageDitherMode>
                label={t('layers.imageMode')}
                value={img.mode}
                options={DITHERS.map((m) => ({ value: m, label: t(`layers.dither.${m}`) }))}
                onChange={(v) => setImage('mode', v)}
                help={t(`layers.dither.${img.mode}.help`)}
              />
              {img.mode === 'threshold' && (
                <NumField field="threshold" label={t('layers.threshold')} value={img.threshold} onApply={(v) => setImage('threshold', v)} help={t('layers.help.threshold')} />
              )}
              <NumField field="brightness" label={t('layers.brightness')} value={img.brightness} onApply={(v) => setImage('brightness', v)} help={t('layers.help.brightness')} />
              <NumField field="contrast" label={t('layers.contrast')} value={img.contrast} onApply={(v) => setImage('contrast', v)} help={t('layers.help.contrast')} />
              <NumField field="gamma" label={t('layers.gamma')} value={img.gamma} onApply={(v) => setImage('gamma', v)} help={t('layers.help.gamma')} />
              <Toggle label={t('layers.invert')} checked={img.invert} onChange={(v) => setImage('invert', v)} help={t('layers.help.invert')} />
              <NumField field="dot_width" label={t('layers.dotWidth')} value={img.dot_width} onApply={(v) => setImage('dot_width', v)} help={t('layers.help.dotWidth')} />
            </Group>
          )}

          <Group title={t('layers.group.order')}>
            <Toggle
              label={t('layers.orderPin')}
              checked={pinned}
              onChange={(v) => set({ order: v ? firstFreePosition(allLayers, color) : null })}
              help={t('layers.help.order')}
            />
            {pinned && (
              <NumField
                field="order"
                label={t('layers.orderPosition')}
                value={layer.order ?? 1}
                ctx={{ layerCount }}
                onApply={(v) => set({ order: v })}
                extra={(v) => {
                  const other = orderConflict(allLayers, color, v);
                  return other ? { key: 'layers.err.orderTaken', params: { name: other.name || colorName(other.color) } } : null;
                }}
              />
            )}
          </Group>
        </>
      )}
    </section>
  );
}

function ModePicker({ mode, onChange }: { mode: LayerMode; onChange: (m: LayerMode) => void }) {
  const showHelp = useLayerUi((s) => s.showHelp);
  const labelId = useId();
  const name = useId();
  return (
    <div className="lyr-field">
      <span id={labelId} className="lyr-field__label">
        {t('layers.mode')}
      </span>
      <div className="lyr-seg" role="radiogroup" aria-labelledby={labelId}>
        {MODES.map(({ mode: m, icon: Icon }) => (
          <label key={m}>
            <input type="radio" name={name} checked={mode === m} onChange={() => onChange(m)} />
            <span>
              <Icon size={13} aria-hidden="true" />
              {t(`layers.mode.${m}`)}
            </span>
          </label>
        ))}
      </div>
      <p className="lyr-help lyr-help--mode" aria-live="polite">
        {showHelp ? t(`layers.mode.${mode}.help`) : null}
      </p>
    </div>
  );
}

/** Overscan: automático (lo calcula el motor) o una distancia en mm. */
function OverscanField({ value, onAuto, onMm }: { value: LayerSettings['overscan']; onAuto: (auto: boolean) => void; onMm: (mm: number) => void }) {
  const auto = value === 'auto';
  return (
    <>
      <Toggle label={t('layers.overscanAuto')} checked={auto} onChange={onAuto} help={t('layers.help.overscan')} />
      {!auto && <NumField field="overscan" label={t('layers.overscan')} value={typeof value === 'number' ? value : MANUAL_OVERSCAN_MM} onApply={onMm} />}
    </>
  );
}

/** Nombre de la capa: no puede quedar vacío. */
function NameField({ value, fallback, onApply }: { value: string; fallback: string; onApply: (v: string) => void }) {
  const id = useId();
  const [text, setText] = useState(value || fallback);
  useEffect(() => setText(value || fallback), [value, fallback]);
  const empty = text.trim() === '';
  return (
    <div className="lyr-field" data-invalid={empty}>
      <div className="lyr-field__top">
        <label htmlFor={id} className="lyr-field__label">
          {t('layers.name')}
        </label>
        <input
          id={id}
          className="input lyr-field__input lyr-name-input"
          value={text}
          maxLength={60}
          aria-invalid={empty}
          onChange={(e) => setText(e.target.value)}
          onBlur={() => !empty && text.trim() !== value && onApply(text.trim())}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !empty) onApply(text.trim());
            else if (e.key === 'Escape' && text !== (value || fallback)) {
              e.stopPropagation();
              setText(value || fallback);
            }
          }}
        />
      </div>
      {empty && (
        <p className="lyr-err" role="alert">
          {t('layers.err.nameEmpty')}
        </p>
      )}
    </div>
  );
}

/** Primera posición de ejecución que ninguna otra capa tiene fijada. */
function firstFreePosition(layers: LayerSettings[], color: string): number {
  const taken = new Set(layers.filter((l) => l.color !== color && l.order !== null).map((l) => l.order));
  for (let n = 1; n <= layers.length; n++) if (!taken.has(n)) return n;
  return layers.length;
}
