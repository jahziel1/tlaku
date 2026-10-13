import { Image as ImageIcon, PaintBucket, Spline, TriangleAlert } from 'lucide-react';
import { useId } from 'react';
import type { DeviceProfile, FillStyle, ImageDitherMode, LayerMode, LayerSettings } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { dpiToInterval, intervalToDpi, usesZ } from '@/lib/layers';
import { selectActiveDevice, useStore } from '@/store';
import { FeatureBlock, Fold, Group, NumField, SelectField, Toggle } from './ParamFields';
import { formatNumber } from './fields';
import { useLayerUi } from './layerUi';
import { activeConflicts, advancedInUse, layerRules, minPowerApplies, minPowerLineNote, type AdvancedFeature, type LayerRules } from './rules';
import { useLayerSetters, type LayerTarget, type SetIn, type SetPatch } from './targets';

const MODES: { mode: LayerMode; icon: typeof Spline }[] = [
  { mode: 'line', icon: Spline },
  { mode: 'fill', icon: PaintBucket },
  { mode: 'image', icon: ImageIcon },
];
const DITHERS: ImageDitherMode[] = ['threshold', 'floyd', 'jarvis', 'stucki', 'atkinson', 'bayer4', 'grayscale', 'halftone', 'newsprint', 'sketch'];
const FILL_STYLES: FillStyle[] = ['scan', 'offset', 'flood'];
const FILL_GROUPINGS: LayerSettings['fill_grouping'][] = ['all', 'by_shape', 'by_group'];
const LEAD_KINDS: LayerSettings['lead_in']['kind'][] = ['straight', 'arc'];
/** Tramados que usan una rejilla de celdas (tamaño y ángulo). */
const GRID_DITHERS: ImageDitherMode[] = ['halftone', 'newsprint'];
/** Overscan inicial al pasar de automático a manual. */
const MANUAL_OVERSCAN_MM = 2;

const CUT_FEATURES: AdvancedFeature[] = ['perforation', 'lead_in', 'lead_out', 'overcut', 'delays', 'cut_through', 'dot_mode', 'ramp'];
const FILL_FEATURES: AdvancedFeature[] = ['fill_style', 'fill_grouping'];
const IMAGE_FEATURES: AdvancedFeature[] = ['pass_through', 'enhance', 'denoise'];

interface FieldsProps {
  layer: LayerSettings;
  target: LayerTarget;
  /** Modo principiante: sin los ajustes avanzados. */
  beginner: boolean;
}

/**
 * Los ajustes de corte de una capa o de una de sus operaciones adicionales: potencia, contorno,
 * barrido, imagen, secciones avanzadas y Z. No incluye lo que solo tiene la capa principal
 * (nombre, visibilidad, salida, posición y operaciones adicionales).
 */
export function LayerFields(props: FieldsProps) {
  const maxRate = useStore((s) => selectActiveDevice(s).max_rate[0]);
  const powerMode = useStore((s) => selectActiveDevice(s).power_mode);
  const hasZ = useStore((s) => selectActiveDevice(s).z_axis === true);
  const hasLaser2 = useStore((s) => selectActiveDevice(s).laser2 === true);
  return <LayerFieldsView {...props} device={{ maxRate, powerMode, hasZ, hasLaser2 }} />;
}

/** Lo que los campos necesitan saber de la máquina activa. */
export interface FieldsDevice {
  /** Velocidad máxima del eje X, mm/min. */
  maxRate: number;
  powerMode: DeviceProfile['power_mode'];
  hasZ: boolean;
  /** La máquina tiene segundo láser: se ofrece elegirlo por capa. */
  hasLaser2?: boolean;
}

/** `LayerFields` con la máquina como dato (así se prueba sin tocar el estado global). */
export function LayerFieldsView({ layer, target, beginner, device }: FieldsProps & { device: FieldsDevice }) {
  const { set, setIn } = useLayerSetters(target);
  const { maxRate, powerMode, hasZ } = device;

  const isImage = layer.mode === 'image';
  const isLine = layer.mode === 'line';
  const usesRaster = !isLine;
  const img = layer.image;
  const tabs = layer.tabs;
  const offsetFill = layer.mode === 'fill' && layer.fill_style === 'offset';
  const rules = layerRules(layer);
  const used = advancedInUse(layer);
  const usedIn = (list: AdvancedFeature[]) => list.some((f) => used.includes(f));
  const showMinPower = minPowerApplies(layer);
  const showMinPowerNote = minPowerLineNote(layer, powerMode) && !beginner;
  const hiddenAdvanced = beginner && used.length > 0;
  const setImage = <K extends keyof LayerSettings['image']>(key: K, value: LayerSettings['image'][K]) => setIn('image', key, value);
  const activeBadge = t('layers.adv.active');

  return (
    <>
      {hiddenAdvanced && (
        <p className="lyr-info" role="status">
          {t('layers.beginner.hiddenInUse')}
        </p>
      )}

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
          ctx={{ minPower: showMinPower ? layer.min_power : undefined }}
          help={isImage ? t('layers.help.powerImage') : t('layers.help.power')}
        />
        {showMinPower && (
          <NumField
            field="min_power"
            label={t('layers.powerMin')}
            value={layer.min_power}
            onApply={(v) => set({ min_power: v })}
            ctx={{ power: layer.power }}
            help={t('layers.help.minPower')}
          />
        )}
        {showMinPowerNote && <p className="lyr-info">{t('layers.note.minPowerLine')}</p>}
        <NumField field="passes" label={t('layers.passes')} value={layer.passes} onApply={(v) => set({ passes: v })} help={t('layers.help.passes')} />
        <Toggle label={t('layers.air')} checked={layer.air} onChange={(v) => set({ air: v })} help={t('layers.help.air')} />
        {(device.hasLaser2 || layer.laser2) && (
          <Toggle label={t('layers.laser2')} checked={layer.laser2} onChange={(v) => set({ laser2: v })} help={t(device.hasLaser2 ? 'layers.help.laser2' : 'layers.help.laser2None')} />
        )}
      </Group>

      {isLine && (
        <Group title={t('layers.group.line')}>
          <NumField field="kerf" label={t('layers.kerf')} value={layer.kerf} onApply={(v) => set({ kerf: v })} help={t('layers.help.kerf')} />
        </Group>
      )}

      {isLine && !beginner && (
        <Group title={t('layers.group.tabs')}>
          <Toggle label={t('layers.tabs.enabled')} checked={tabs.enabled} onChange={(v) => setIn('tabs', 'enabled', v)} help={t('layers.help.tabsEnabled')} />
          {tabs.enabled && (
            <>
              <NumField field="tab_width" label={t('layers.tabs.width')} value={tabs.width} onApply={(v) => setIn('tabs', 'width', v)} help={t('layers.help.tabsWidth')} />
              <NumField
                field="tab_count"
                label={tabs.spacing > 0 ? t('layers.tabs.countMin') : t('layers.tabs.count')}
                value={tabs.count}
                onApply={(v) => setIn('tabs', 'count', v)}
                help={tabs.spacing > 0 ? t('layers.help.tabsCountMin') : t('layers.help.tabsCount')}
                live={
                  tabs.spacing > 0
                    ? t('layers.tabs.live.spacing', { spacing: formatNumber(tabs.spacing, 2), n: tabs.count })
                    : t('layers.tabs.live.count', { n: tabs.count })
                }
              />
              <NumField
                field="tab_spacing"
                label={t('layers.tabs.spacing')}
                value={tabs.spacing}
                ctx={{ tabWidth: tabs.width }}
                onApply={(v) => setIn('tabs', 'spacing', v)}
                help={t('layers.help.tabsSpacing')}
              />
              <NumField field="tab_power" label={t('layers.tabs.power')} value={tabs.power} onApply={(v) => setIn('tabs', 'power', v)} help={t('layers.help.tabsPower')} />
              <Toggle label={t('layers.tabs.skipInner')} checked={tabs.skip_inner} onChange={(v) => setIn('tabs', 'skip_inner', v)} help={t('layers.help.tabsSkipInner')} />
            </>
          )}
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
          {offsetFill ? (
            <p className="lyr-info">{t('layers.fillStyle.offsetNote')}</p>
          ) : (
            <>
              <NumField
                field="angle"
                label={isImage ? t('layers.angleImage') : t('layers.angle')}
                value={layer.angle}
                onApply={(v) => set({ angle: v })}
                help={isImage ? t('layers.help.angleImage') : t('layers.help.angle')}
              />
              {layer.mode === 'fill' && (
                <Toggle label={t('layers.crosshatch')} checked={layer.crosshatch} onChange={(v) => set({ crosshatch: v })} help={t('layers.help.crosshatch')} />
              )}
              <Toggle label={t('layers.bidirectional')} checked={layer.bidirectional} onChange={(v) => set({ bidirectional: v })} help={t('layers.help.bidirectional')} />
              <OverscanField value={layer.overscan} onAuto={(auto) => set({ overscan: auto ? 'auto' : MANUAL_OVERSCAN_MM })} onMm={(v) => set({ overscan: v })} />
            </>
          )}
          {!beginner && (
            <NumField
              field="angle_increment"
              label={t('layers.angleIncrement')}
              value={layer.angle_increment}
              onApply={(v) => set({ angle_increment: v })}
              disabled={rules.angleIncrement !== null}
              reason={rules.angleIncrement ? t(rules.angleIncrement) : undefined}
              live={layer.passes < 2 && layer.angle_increment !== 0 ? t('layers.angleIncrement.live') : undefined}
              help={t('layers.help.angleIncrement')}
            />
          )}
        </Group>
      )}

      {isImage && (
        <Group title={t('layers.group.image')}>
          {rules.imageProcessing && <p className="lyr-info">{t(rules.imageProcessing)}</p>}
          <SelectField<ImageDitherMode>
            label={t('layers.imageMode')}
            value={img.mode}
            options={DITHERS.map((m) => ({ value: m, label: t(`layers.dither.${m}`) }))}
            onChange={(v) => setImage('mode', v)}
            disabled={rules.imageProcessing !== null}
            help={t(`layers.dither.${img.mode}.help`)}
          />
          {img.mode === 'threshold' && (
            <NumField
              field="threshold"
              label={t('layers.threshold')}
              value={img.threshold}
              onApply={(v) => setImage('threshold', v)}
              disabled={rules.imageProcessing !== null}
              help={t('layers.help.threshold')}
            />
          )}
          {GRID_DITHERS.includes(img.mode) && (
            <>
              <NumField
                field="halftone_cell"
                label={t('layers.halftoneCell')}
                value={img.halftone_cell}
                onApply={(v) => setImage('halftone_cell', v)}
                disabled={rules.imageProcessing !== null}
                help={t('layers.help.halftoneCell')}
              />
              <NumField
                field="halftone_angle"
                label={t('layers.halftoneAngle')}
                value={img.halftone_angle}
                onApply={(v) => setImage('halftone_angle', v)}
                disabled={rules.imageProcessing !== null}
                help={t('layers.help.halftoneAngle')}
              />
            </>
          )}
          <NumField field="brightness" label={t('layers.brightness')} value={img.brightness} onApply={(v) => setImage('brightness', v)} disabled={rules.imageProcessing !== null} help={t('layers.help.brightness')} />
          <NumField field="contrast" label={t('layers.contrast')} value={img.contrast} onApply={(v) => setImage('contrast', v)} disabled={rules.imageProcessing !== null} help={t('layers.help.contrast')} />
          <NumField field="gamma" label={t('layers.gamma')} value={img.gamma} onApply={(v) => setImage('gamma', v)} disabled={rules.imageProcessing !== null} help={t('layers.help.gamma')} />
          <Toggle label={t('layers.invert')} checked={img.invert} onChange={(v) => setImage('invert', v)} disabled={rules.imageProcessing !== null} help={t('layers.help.invert')} />
          <NumField field="dot_width" label={t('layers.dotWidth')} value={img.dot_width} onApply={(v) => setImage('dot_width', v)} help={t('layers.help.dotWidth')} />
        </Group>
      )}

      {!beginner && isLine && (
        <Fold title={t('layers.adv.cut')} badge={usedIn(CUT_FEATURES) ? activeBadge : undefined} defaultOpen={usedIn(CUT_FEATURES)}>
          <CutAdvanced layer={layer} rules={rules} set={set} setIn={setIn} />
        </Fold>
      )}

      {!beginner && layer.mode === 'fill' && (
        <Fold title={t('layers.adv.fill')} badge={usedIn(FILL_FEATURES) ? activeBadge : undefined} defaultOpen={usedIn(FILL_FEATURES)}>
          <SelectField<FillStyle>
            label={t('layers.fillStyle')}
            value={layer.fill_style}
            options={FILL_STYLES.map((f) => ({ value: f, label: t(`layers.fillStyle.${f}`) }))}
            onChange={(v) => set({ fill_style: v })}
            help={t('layers.help.fillStyle')}
          />
          <SelectField<LayerSettings['fill_grouping']>
            label={t('layers.fillGrouping')}
            value={layer.fill_grouping}
            options={FILL_GROUPINGS.map((g) => ({ value: g, label: t(`layers.fillGrouping.${g}`) }))}
            onChange={(v) => set({ fill_grouping: v })}
            help={t('layers.help.fillGrouping')}
          />
        </Fold>
      )}

      {!beginner && isImage && (
        <Fold title={t('layers.adv.image')} badge={usedIn(IMAGE_FEATURES) ? activeBadge : undefined} defaultOpen={usedIn(IMAGE_FEATURES)}>
          <Toggle label={t('layers.passThrough')} checked={img.pass_through} onChange={(v) => setImage('pass_through', v)} help={t('layers.help.passThrough')} />
          <NumField
            field="enhance_radius"
            label={t('layers.enhanceRadius')}
            value={img.enhance_radius}
            onApply={(v) => setImage('enhance_radius', v)}
            disabled={rules.imageProcessing !== null}
            reason={rules.imageProcessing ? t(rules.imageProcessing) : undefined}
            help={t('layers.help.enhanceRadius')}
          />
          <NumField
            field="enhance_amount"
            label={t('layers.enhanceAmount')}
            value={img.enhance_amount}
            onApply={(v) => setImage('enhance_amount', v)}
            disabled={rules.imageProcessing !== null}
            live={!rules.imageProcessing && (img.enhance_radius > 0) !== (img.enhance_amount > 0) ? t('layers.enhance.live') : undefined}
            help={t('layers.help.enhanceAmount')}
          />
          <NumField
            field="denoise"
            label={t('layers.denoise')}
            value={img.denoise}
            onApply={(v) => setImage('denoise', v)}
            disabled={rules.imageProcessing !== null}
            help={t('layers.help.denoise')}
          />
        </Fold>
      )}

      {!beginner && (hasZ || usesZ(layer)) && (
        <Group title={t('layers.group.z')}>
          {hasZ ? (
            <>
              <NumField field="z_offset" label={t('layers.zOffset')} value={layer.z_offset} onApply={(v) => set({ z_offset: v })} help={t('layers.help.zOffset')} />
              <NumField field="z_step" label={t('layers.zStep')} value={layer.z_step} onApply={(v) => set({ z_step: v })} help={t('layers.help.zStep')} />
            </>
          ) : (
            <>
              <p className="warn-line" role="status">
                <TriangleAlert size={14} aria-hidden="true" />
                <span>{t('layers.warn.zNoAxis')}</span>
              </p>
              <button type="button" className="btn" onClick={() => set({ z_offset: 0, z_step: 0 })}>
                {t('layers.zReset')}
              </button>
            </>
          )}
        </Group>
      )}
    </>
  );
}

/** Contenido de la sección "Corte avanzado" (solo capas de línea). */
function CutAdvanced({ layer, rules, set, setIn }: { layer: LayerSettings; rules: LayerRules; set: SetPatch; setIn: SetIn }) {
  const perf = layer.perforation;
  const ct = layer.cut_through;
  const dot = layer.dot_mode;
  const ramp = layer.ramp;
  const reason = (r: I18nKey | null) => (r ? t(r) : undefined);
  return (
    <>
      {activeConflicts(layer).map((k) => (
        <p key={k} className="warn-line" role="status">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t(k)}</span>
        </p>
      ))}

      <FeatureBlock
        label={t('layers.perf.enabled')}
        checked={perf.enabled}
        onChange={(v) => setIn('perforation', 'enabled', v)}
        disabled={rules.perforation !== null}
        reason={reason(rules.perforation)}
        help={t('layers.help.perf')}
      >
        <NumField field="perf_cut" label={t('layers.perf.cut')} value={perf.cut_mm} onApply={(v) => setIn('perforation', 'cut_mm', v)} help={t('layers.help.perfCut')} />
        <NumField
          field="perf_skip"
          label={t('layers.perf.skip')}
          value={perf.skip_mm}
          onApply={(v) => setIn('perforation', 'skip_mm', v)}
          help={t('layers.help.perfSkip')}
          live={t('layers.perf.live', { period: formatNumber(perf.cut_mm + perf.skip_mm, 3), cut: formatNumber(perf.cut_mm, 3), skip: formatNumber(perf.skip_mm, 3) })}
        />
      </FeatureBlock>

      <LeadBlock which="lead_in" lead={layer.lead_in} setIn={setIn} />
      <LeadBlock which="lead_out" lead={layer.lead_out} setIn={setIn} />

      <NumField field="overcut" label={t('layers.overcut')} value={layer.overcut} onApply={(v) => set({ overcut: v })} help={t('layers.help.overcut')} />
      <NumField field="delay" label={t('layers.startDelay')} value={layer.start_delay_ms} onApply={(v) => set({ start_delay_ms: v })} help={t('layers.help.startDelay')} />
      <NumField field="delay" label={t('layers.endDelay')} value={layer.end_delay_ms} onApply={(v) => set({ end_delay_ms: v })} help={t('layers.help.endDelay')} />

      <FeatureBlock label={t('layers.cutThrough.enabled')} checked={ct.enabled} onChange={(v) => setIn('cut_through', 'enabled', v)} help={t('layers.help.cutThrough')}>
        <NumField field="cut_through_power" label={t('layers.cutThrough.power')} value={ct.power} onApply={(v) => setIn('cut_through', 'power', v)} help={t('layers.help.cutThroughPower')} />
        <NumField field="cut_through_ms" label={t('layers.cutThrough.ms')} value={ct.ms} onApply={(v) => setIn('cut_through', 'ms', v)} help={t('layers.help.cutThroughMs')} />
      </FeatureBlock>

      <FeatureBlock
        label={t('layers.dot.enabled')}
        checked={dot.enabled}
        onChange={(v) => setIn('dot_mode', 'enabled', v)}
        disabled={rules.dotMode !== null}
        reason={reason(rules.dotMode)}
        help={t('layers.help.dot')}
      >
        <NumField field="dot_dwell" label={t('layers.dot.dwell')} value={dot.dwell_ms} onApply={(v) => setIn('dot_mode', 'dwell_ms', v)} help={t('layers.help.dotDwell')} />
        <NumField field="dot_spacing" label={t('layers.dot.spacing')} value={dot.spacing_mm} onApply={(v) => setIn('dot_mode', 'spacing_mm', v)} help={t('layers.help.dotSpacing')} />
      </FeatureBlock>

      <FeatureBlock
        label={t('layers.ramp.enabled')}
        checked={ramp.enabled}
        onChange={(v) => setIn('ramp', 'enabled', v)}
        disabled={rules.ramp !== null}
        reason={reason(rules.ramp)}
        help={t('layers.help.ramp')}
      >
        <NumField field="ramp_length" label={t('layers.ramp.length')} value={ramp.length_mm} onApply={(v) => setIn('ramp', 'length_mm', v)} help={t('layers.help.rampLength')} />
        <NumField field="ramp_start" label={t('layers.ramp.start')} value={ramp.start_pct} onApply={(v) => setIn('ramp', 'start_pct', v)} help={t('layers.help.rampStart')} />
        <Toggle label={t('layers.ramp.outer')} checked={ramp.outer_only} onChange={(v) => setIn('ramp', 'outer_only', v)} help={t('layers.help.rampOuter')} />
      </FeatureBlock>
    </>
  );
}

/** Entrada o salida del contorno: interruptor y, encendido, forma, longitud y ángulo. */
function LeadBlock({ which, lead, setIn }: { which: 'lead_in' | 'lead_out'; lead: LayerSettings['lead_in']; setIn: SetIn }) {
  const inn = which === 'lead_in';
  return (
    <FeatureBlock
      label={inn ? t('layers.leadIn.enabled') : t('layers.leadOut.enabled')}
      checked={lead.enabled}
      onChange={(v) => setIn(which, 'enabled', v)}
      help={inn ? t('layers.help.leadIn') : t('layers.help.leadOut')}
    >
      <SelectField<LayerSettings['lead_in']['kind']>
        label={t('layers.lead.kind')}
        value={lead.kind}
        options={LEAD_KINDS.map((k) => ({ value: k, label: t(`layers.lead.kind.${k}`) }))}
        onChange={(v) => setIn(which, 'kind', v)}
        help={t('layers.help.leadKind')}
      />
      <NumField field="lead_length" label={t('layers.lead.length')} value={lead.length} onApply={(v) => setIn(which, 'length', v)} help={t('layers.help.leadLength')} />
      <NumField
        field="lead_angle"
        ctx={{ leadKind: lead.kind }}
        label={lead.kind === 'arc' ? t('layers.lead.sweep') : t('layers.lead.angle')}
        value={lead.angle}
        onApply={(v) => setIn(which, 'angle', v)}
        help={lead.kind === 'arc' ? t('layers.help.leadSweep') : t('layers.help.leadAngle')}
      />
    </FeatureBlock>
  );
}

/** Selector del modo de una capa o de una operación adicional. */
export function ModePicker({ mode, onChange, label }: { mode: LayerMode; onChange: (m: LayerMode) => void; label?: string }) {
  const showHelp = useLayerUi((s) => s.showHelp);
  const labelId = useId();
  const name = useId();
  return (
    <div className="lyr-field">
      <span id={labelId} className="lyr-field__label">
        {label ?? t('layers.mode')}
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
