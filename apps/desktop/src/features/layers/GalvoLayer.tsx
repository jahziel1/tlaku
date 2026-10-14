import { TriangleAlert } from 'lucide-react';
import type { LaserSource, LayerSettings } from '@/api/types';
import { t } from '@/i18n';
import { Fold, Group, NumField, SelectField, Toggle } from './ParamFields';
import type { SetIn } from './targets';

/** Lo que la sección Galvo necesita saber de la máquina activa (su perfil galvo). */
export interface GalvoUiProfile {
  source: LaserSource;
  freqMinKhz: number;
  freqMaxKhz: number;
  /** Anchos de pulso del perfil (ns). */
  pulseWidthsNs: number[];
  /** Overscan de rellenos e imágenes del perfil (mm); 0 o ausente = ninguno. */
  overscanMm?: number;
}

/** mm/min de la capa a mm/s que se muestran, y al revés. */
export const mmMinToMmS = (v: number): number => v / 60;
export const mmSToMmMin = (v: number): number => Math.round(v * 60 * 1000) / 1000;

/** ¿Esta fuente tiene ancho de pulso por comando propio? (fibra MOPA y UV). */
export const sourceHasPulse = (s: LaserSource): boolean => s === 'fiber_mopa' || s === 'uv';

/** `null` si la frecuencia cabe en el rango de la fuente; si no, el rango, para avisar. */
export function freqOutOfRange(khz: number, p: GalvoUiProfile): { min: number; max: number } | null {
  return p.freqMaxKhz > 0 && (khz < p.freqMinKhz || khz > p.freqMaxKhz) ? { min: p.freqMinKhz, max: p.freqMaxKhz } : null;
}

interface Props {
  layer: LayerSettings;
  setIn: SetIn;
  profile: GalvoUiProfile;
}

const NO_PULSE = '';

/**
 * Sección «Galvo» del editor de capa (solo con una máquina galvo): frecuencia y ancho de pulso, retardos
 * propios de la capa, bamboleo, repeticiones y pausas, PPI y grabado 3D por cortes. La velocidad de la
 * capa se edita en mm/s arriba (el galvo se piensa en mm/s) y se guarda en mm/min.
 */
export function GalvoLayerSection({ layer, setIn, profile }: Props) {
  const g = layer.galvo;
  const range = freqOutOfRange(g.frequency_khz, profile);
  const pulses = profile.pulseWidthsNs;
  const hasPulse = sourceHasPulse(profile.source);
  const needPulse = profile.source === 'fiber_mopa';
  const pulseValue = g.pulse_width_ns === null ? NO_PULSE : String(g.pulse_width_ns);
  // Un valor guardado que ya no está en la lista del perfil se sigue mostrando (y se avisa).
  const pulseOptions = [...new Set([...pulses, ...(g.pulse_width_ns !== null ? [g.pulse_width_ns] : [])])].sort((a, b) => a - b);
  const pulseOff = g.pulse_width_ns !== null && pulses.length > 0 && !pulses.includes(g.pulse_width_ns);
  const isLine = layer.mode === 'line';
  const isImage = layer.mode === 'image';

  return (
    <Group title={t('galvo.layer.group')}>
      <p className="lyr-help">{t('galvo.layer.intro')}</p>
      <NumField
        field="galvo_freq"
        label={t('galvo.layer.freq')}
        value={g.frequency_khz}
        onApply={(v) => setIn('galvo', 'frequency_khz', v)}
        help={t('galvo.layer.freq.help')}
        live={range ? t('galvo.layer.freq.range', { min: range.min, max: range.max }) : undefined}
      />
      {hasPulse ? (
        <SelectField<string>
          label={t('galvo.layer.pulse')}
          value={pulseValue}
          options={[{ value: NO_PULSE, label: t(needPulse ? 'galvo.layer.pulse.choose' : 'galvo.layer.pulse.auto') }, ...pulseOptions.map((p) => ({ value: String(p), label: `${p} ns` }))]}
          onChange={(v) => setIn('galvo', 'pulse_width_ns', v === NO_PULSE ? null : Number(v))}
          help={t(needPulse ? 'galvo.layer.pulse.help.mopa' : 'galvo.layer.pulse.help.uv')}
        />
      ) : (
        <p className="lyr-live">{t('galvo.layer.pulse.none')}</p>
      )}
      {hasPulse && needPulse && g.pulse_width_ns === null && (
        <p className="lyr-err" role="alert">
          <TriangleAlert size={12} aria-hidden="true" /> {t('galvo.layer.pulse.required')}
        </p>
      )}
      {pulseOff && <p className="lyr-live">{t('galvo.layer.pulse.notInList', { ns: g.pulse_width_ns ?? 0 })}</p>}
      {!isLine && (profile.overscanMm ?? 0) > 0 && <p className="lyr-live">{t('galvo.layer.overscan.profile', { mm: profile.overscanMm ?? 0 })}</p>}
      <NumField field="galvo_repeat" label={t('galvo.layer.repeat')} value={g.repeat} onApply={(v) => setIn('galvo', 'repeat', v)} help={t('galvo.layer.repeat.help')} />

      <Fold title={t('galvo.layer.timing')} defaultOpen={g.override_timings}>
        <Toggle label={t('galvo.layer.override')} checked={g.override_timings} onChange={(v) => setIn('galvo', 'override_timings', v)} help={t('galvo.layer.override.help')} />
        {g.override_timings && (
          <>
            <NumField field="galvo_delay" label={t('galvo.layer.laserOn')} value={g.timing.laser_on_us} onApply={(v) => setIn('galvo', 'timing', { ...g.timing, laser_on_us: v })} help={t('galvo.layer.laserOn.help')} />
            <NumField field="galvo_delay" label={t('galvo.layer.laserOff')} value={g.timing.laser_off_us} onApply={(v) => setIn('galvo', 'timing', { ...g.timing, laser_off_us: v })} help={t('galvo.layer.laserOff.help')} />
            <NumField field="galvo_delay" label={t('galvo.layer.end')} value={g.timing.end_us} onApply={(v) => setIn('galvo', 'timing', { ...g.timing, end_us: v })} help={t('galvo.layer.end.help')} />
            <NumField field="galvo_delay" label={t('galvo.layer.polygon')} value={g.timing.polygon_us} onApply={(v) => setIn('galvo', 'timing', { ...g.timing, polygon_us: v })} help={t('galvo.layer.polygon.help')} />
            <NumField field="galvo_jump_speed" label={t('galvo.layer.jump.speed')} value={g.jump.speed} onApply={(v) => setIn('galvo', 'jump', { ...g.jump, speed: v })} help={t('galvo.layer.jump.help')} />
            <NumField field="galvo_jump_delay" label={t('galvo.layer.jump.min')} value={g.jump.delay_min_us} onApply={(v) => setIn('galvo', 'jump', { ...g.jump, delay_min_us: v })} />
            <NumField field="galvo_jump_delay" label={t('galvo.layer.jump.max')} value={g.jump.delay_max_us} onApply={(v) => setIn('galvo', 'jump', { ...g.jump, delay_max_us: v })} />
            <NumField field="overscan" label={t('galvo.layer.jump.limit')} value={g.jump.distance_limit} onApply={(v) => setIn('galvo', 'jump', { ...g.jump, distance_limit: v })} help={t('galvo.layer.jump.limit.help')} />
          </>
        )}
      </Fold>

      {isLine && (
        <Fold title={t('galvo.layer.wobble')} defaultOpen={g.wobble.enabled}>
          <Toggle label={t('galvo.layer.wobble.enable')} checked={g.wobble.enabled} onChange={(v) => setIn('galvo', 'wobble', { ...g.wobble, enabled: v })} help={t('galvo.layer.wobble.help')} />
          {g.wobble.enabled && (
            <>
              <NumField field="galvo_wobble_size" label={t('galvo.layer.wobble.size')} value={g.wobble.size} onApply={(v) => setIn('galvo', 'wobble', { ...g.wobble, size: v })} help={t('galvo.layer.wobble.size.help')} />
              <NumField field="galvo_wobble_step" label={t('galvo.layer.wobble.step')} value={g.wobble.step} onApply={(v) => setIn('galvo', 'wobble', { ...g.wobble, step: v })} help={t('galvo.layer.wobble.step.help')} />
            </>
          )}
        </Fold>
      )}

      <Fold title={t('galvo.layer.pauses')} defaultOpen={g.pause_before_ms > 0 || g.pause_after_ms > 0 || g.ppi > 0}>
        <NumField field="galvo_pause" label={t('galvo.layer.pauseBefore')} value={g.pause_before_ms} onApply={(v) => setIn('galvo', 'pause_before_ms', v)} help={t('galvo.layer.pauseBefore.help')} />
        <NumField field="galvo_pause" label={t('galvo.layer.pauseAfter')} value={g.pause_after_ms} onApply={(v) => setIn('galvo', 'pause_after_ms', v)} help={t('galvo.layer.pauseAfter.help')} />
        <NumField field="galvo_ppi" label={t('galvo.layer.ppi')} value={g.ppi} onApply={(v) => setIn('galvo', 'ppi', v)} help={t('galvo.layer.ppi.help')} live={g.ppi > 0 ? t('galvo.layer.ppi.ignored') : undefined} />
      </Fold>

      {isImage && (
        <Fold title={t('galvo.layer.slice')} defaultOpen={g.slice3d.enabled}>
          <Toggle label={t('galvo.layer.slice.enable')} checked={g.slice3d.enabled} onChange={(v) => setIn('galvo', 'slice3d', { ...g.slice3d, enabled: v })} help={t('galvo.layer.slice.help')} />
          {g.slice3d.enabled && (
            <>
              <NumField field="galvo_slices" label={t('galvo.layer.slice.count')} value={g.slice3d.slices} onApply={(v) => setIn('galvo', 'slice3d', { ...g.slice3d, slices: v })} help={t('galvo.layer.slice.count.help')} />
              <NumField field="galvo_angle_step" label={t('galvo.layer.slice.angle')} value={g.slice3d.angle_step_deg} onApply={(v) => setIn('galvo', 'slice3d', { ...g.slice3d, angle_step_deg: v })} help={t('galvo.layer.slice.angle.help')} />
            </>
          )}
        </Fold>
      )}
    </Group>
  );
}
