import { useEffect, useState, type ReactNode } from 'react';
import type { DeviceProfile, LaserSource } from '@/api/types';
import type { CylinderAxis } from '@/bindings/CylinderAxis';
import type { SplitAxes } from '@/bindings/SplitAxes';
import type { SplitOrder } from '@/bindings/SplitOrder';
import { t } from '@/i18n';
import { NumInput } from './DeviceFields';
import { Check, GField, Sub } from './GalvoFieldParts';
import { GalvoExperimentalBadge } from './GalvoNotice';
import { GalvoInterlockEditor } from './GalvoInterlock';
import { GalvoRotaryFields } from './GalvoRotaryFields';
import { warpActive } from './galvoWarp';
import { useWarpUi } from './galvoWarpState';
import { SOURCE_WAVELENGTH_NM } from './galvoPorts';
import { COMMON_FIELDS_MM, formatPulseWidths, GALVO_SOURCES, parsePulseWidths, usesPulseWidth, type GalvoField, type GalvoIssues } from './galvoProfile';

type Edit = (patch: Partial<DeviceProfile>) => void;

/** Lista de anchos de pulso escrita a mano («4, 6, 9»); se confirma al salir del campo. */
function PulseWidthsInput({ id, describedBy, invalid, value, onCommit }: { id: string; describedBy: string; invalid: boolean; value: number[]; onCommit: (v: number[]) => void }) {
  const [text, setText] = useState(formatPulseWidths(value));
  const [bad, setBad] = useState(false);
  useEffect(() => {
    setText(formatPulseWidths(value));
    setBad(false);
  }, [value]);
  const commit = () => {
    const list = text.trim() === '' ? [] : parsePulseWidths(text);
    if (list === null) return setBad(true);
    setBad(false);
    if (formatPulseWidths(list) !== formatPulseWidths(value)) onCommit(list);
    else setText(formatPulseWidths(list));
  };
  return (
    <>
      <div className="field__box">
        <input id={id} className="field__input num" value={text} aria-describedby={describedBy} aria-invalid={invalid || bad || undefined} placeholder="4, 6, 9, 13" onChange={(e) => setText(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), commit())} />
        <span className="field__unit">ns</span>
      </div>
      {bad && <p className="hint hint--danger" role="alert">{t('galvo.err.pulseList')}</p>}
    </>
  );
}

const AXES: CylinderAxis[] = ['x', 'y'];
const ORDERS: SplitOrder[] = ['snake', 'rows'];
const SPLIT_AXES: SplitAxes[] = ['xy', 'x', 'y'];

/**
 * Campos de un perfil galvo (EXPERIMENTAL): la lente y la fuente, los retardos y saltos por defecto, el
 * puntero rojo, la corrección del campo, el cilindro y el cono, y el marcado dividido y repetido. Cada
 * grupo trae una ayuda corta. El origen y la cama se editan en sus grupos de siempre; aquí solo se avisa
 * que el origen debe ser «abajo a la izquierda».
 */
export function GalvoFields({ draft, edit, issues, calibrate }: { draft: DeviceProfile; edit: Edit; issues: GalvoIssues; calibrate?: ReactNode }) {
  const g = draft.galvo;
  const set = (patch: Partial<DeviceProfile['galvo']>) => edit({ galvo: { ...g, ...patch } });
  const setTiming = (patch: Partial<typeof g.timing>) => set({ timing: { ...g.timing, ...patch } });
  const setJump = (patch: Partial<typeof g.jump>) => set({ jump: { ...g.jump, ...patch } });
  const cor = g.correction;
  const setCor = (patch: Partial<typeof cor>) => set({ correction: { ...cor, ...patch } });
  const pair = (v: [number, number], i: 0 | 1, n: number): [number, number] => (i === 0 ? [n, v[1]] : [v[0], n]);
  const pairBool = (v: [boolean, boolean], i: 0 | 1, n: boolean): [boolean, boolean] => (i === 0 ? [n, v[1]] : [v[0], n]);
  const warpShow = useWarpUi((w) => w.show);
  const setWarpShow = useWarpUi((w) => w.setShow);
  const warpOn = warpActive(g);
  const source = g.source;
  const nm = SOURCE_WAVELENGTH_NM[source];

  return (
    <>
      <p className="hint">
        <GalvoExperimentalBadge /> {t('galvo.experimental.note')}
      </p>
      {issues.origin && <p className="hint hint--danger" role="alert">{t(issues.origin)}</p>}
      <p className="hint">{t('galvo.dev.hidden')}</p>

      <Sub title={t('galvo.dev.lens')} help="galvo.dev.lens.help">
        <div className="field-grid">
          <GField label={t('galvo.dev.field')} help="galvo.dev.field.help" error={issues.field_mm}>
            {({ id, describedBy, invalid }) => (
              <>
                <NumInput id={id} unit="mm" value={g.field_mm} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ field_mm: v })} />
                <div className="mc-chips" role="group" aria-label={t('galvo.dev.field.common')}>
                  {COMMON_FIELDS_MM.map((mm) => (
                    <button key={mm} type="button" className="chip num" aria-pressed={g.field_mm === mm} onClick={() => set({ field_mm: mm })}>{mm}</button>
                  ))}
                </div>
              </>
            )}
          </GField>
          <GField label={t('galvo.dev.source')} help="galvo.dev.source.help">
            {({ id, describedBy }) => (
              <select id={id} className="select" value={source} aria-describedby={describedBy} onChange={(e) => set({ source: e.target.value as LaserSource })}>
                {GALVO_SOURCES.map((s) => (
                  <option key={s} value={s}>{t(`galvo.source.${s}`)}</option>
                ))}
              </select>
            )}
          </GField>
        </div>
        <p className="hint num">{t('galvo.dev.wavelength', { nm })}</p>
        <div className="field-grid">
          <GField label={t('galvo.dev.watts')} help="galvo.dev.watts.help" error={issues.watts}>
            {({ id, describedBy, invalid }) => <NumInput id={id} unit="W" value={g.watts} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ watts: v })} />}
          </GField>
          <GField label={t('galvo.dev.maxPower')} help="galvo.dev.maxPower.help" error={issues.max_power_pct}>
            {({ id, describedBy, invalid }) => <NumInput id={id} unit="%" value={g.max_power_pct} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ max_power_pct: v })} />}
          </GField>
        </div>
        <GField label={t('galvo.dev.freq')} help="galvo.dev.freq.help" error={issues.freq}>
          {({ id, describedBy, invalid }) => (
            <div className="field-grid">
              <NumInput id={id} label={t('galvo.dev.freq.min')} unit="kHz" value={g.freq_min_khz} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ freq_min_khz: v })} />
              <NumInput label={t('galvo.dev.freq.max')} unit="kHz" value={g.freq_max_khz} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ freq_max_khz: v })} />
            </div>
          )}
        </GField>
        {usesPulseWidth(source) ? (
          <GField label={t('galvo.dev.pulses')} help={source === 'fiber_mopa' ? 'galvo.dev.pulses.help.mopa' : 'galvo.dev.pulses.help.uv'} error={issues.pulse_widths}>
            {({ id, describedBy, invalid }) => <PulseWidthsInput id={id} describedBy={describedBy} invalid={invalid} value={g.pulse_widths_ns} onCommit={(v) => set({ pulse_widths_ns: v })} />}
          </GField>
        ) : (
          <p className="hint">{t('galvo.dev.pulses.none')}</p>
        )}
      </Sub>

      <Sub title={t('galvo.dev.timing')} help="galvo.dev.timing.help">
        <GField label={t('galvo.dev.delays')} help="galvo.dev.delays.help" error={issues.timing}>
          {({ id, describedBy, invalid }) => (
            <div className="field-grid">
              <NumInput id={id} label={t('galvo.dev.laserOn')} unit="µs" value={g.timing.laser_on_us} describedBy={describedBy} invalid={invalid} onCommit={(v) => setTiming({ laser_on_us: v })} />
              <NumInput label={t('galvo.dev.laserOff')} unit="µs" value={g.timing.laser_off_us} describedBy={describedBy} invalid={invalid} onCommit={(v) => setTiming({ laser_off_us: v })} />
              <NumInput label={t('galvo.dev.end')} unit="µs" value={g.timing.end_us} describedBy={describedBy} invalid={invalid} onCommit={(v) => setTiming({ end_us: v })} />
              <NumInput label={t('galvo.dev.polygon')} unit="µs" value={g.timing.polygon_us} describedBy={describedBy} invalid={invalid} onCommit={(v) => setTiming({ polygon_us: v })} />
            </div>
          )}
        </GField>
        <GField label={t('galvo.dev.jump')} help="galvo.dev.jump.help" error={issues.jump}>
          {({ id, describedBy, invalid }) => (
            <div className="field-grid">
              <NumInput id={id} label={t('galvo.dev.jump.speed')} unit="mm/s" value={g.jump.speed} describedBy={describedBy} invalid={invalid} onCommit={(v) => setJump({ speed: v })} />
              <NumInput label={t('galvo.dev.jump.min')} unit="µs" value={g.jump.delay_min_us} describedBy={describedBy} invalid={invalid} onCommit={(v) => setJump({ delay_min_us: v })} />
              <NumInput label={t('galvo.dev.jump.max')} unit="µs" value={g.jump.delay_max_us} describedBy={describedBy} invalid={invalid} onCommit={(v) => setJump({ delay_max_us: v })} />
              <NumInput label={t('galvo.dev.jump.limit')} unit="mm" value={g.jump.distance_limit} describedBy={describedBy} invalid={invalid} onCommit={(v) => setJump({ distance_limit: v })} />
            </div>
          )}
        </GField>
        <GField label={t('galvo.dev.overscan')} help="galvo.dev.overscan.help" error={issues.overscan}>
          {({ id, describedBy, invalid }) => <NumInput id={id} unit="mm" value={g.overscan_mm} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ overscan_mm: v })} />}
        </GField>
      </Sub>

      <Sub title={t('galvo.dev.pointer')} help="galvo.dev.pointer.help">
        <GField label={t('galvo.dev.lightSpeed')} help="galvo.dev.lightSpeed.help" error={issues.light_speed}>
          {({ id, describedBy, invalid }) => <NumInput id={id} unit="mm/s" value={g.light_speed} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ light_speed: v })} />}
        </GField>
        <GField label={t('galvo.dev.pointerOffset')} help="galvo.dev.pointerOffset.help" error={issues.pointer_offset}>
          {({ id, describedBy, invalid }) => (
            <div className="field-grid">
              <NumInput id={id} label="X" unit="mm" value={draft.pointer_offset[0]} describedBy={describedBy} invalid={invalid} onCommit={(v) => edit({ pointer_offset: pair(draft.pointer_offset, 0, v) })} />
              <NumInput label="Y" unit="mm" value={draft.pointer_offset[1]} describedBy={describedBy} invalid={invalid} onCommit={(v) => edit({ pointer_offset: pair(draft.pointer_offset, 1, v) })} />
            </div>
          )}
        </GField>
      </Sub>

      <Sub title={t('galvo.dev.interlock')} help="galvo.dev.interlock.help">
        <GalvoInterlockEditor value={g.require_input} onChange={(v) => set({ require_input: v })} />
        {issues.interlock && <p className="hint hint--danger" role="alert">{t(issues.interlock)}</p>}
      </Sub>

      <Sub title={t('galvo.dev.correction')} help="galvo.dev.correction.help">
        <GField label={t('galvo.dev.scale')} help="galvo.dev.scale.help" error={issues.scale}>
          {({ id, describedBy, invalid }) => (
            <div className="field-grid">
              <NumInput id={id} label="X" value={cor.scale[0]} describedBy={describedBy} invalid={invalid} onCommit={(v) => setCor({ scale: pair(cor.scale, 0, v) })} />
              <NumInput label="Y" value={cor.scale[1]} describedBy={describedBy} invalid={invalid} onCommit={(v) => setCor({ scale: pair(cor.scale, 1, v) })} />
            </div>
          )}
        </GField>
        <GField label={t('galvo.dev.offset')} help="galvo.dev.offset.help" error={issues.rotation}>
          {({ id, describedBy, invalid }) => (
            <div className="field-grid">
              <NumInput id={id} label="X" unit="mm" value={cor.offset[0]} describedBy={describedBy} invalid={invalid} onCommit={(v) => setCor({ offset: pair(cor.offset, 0, v) })} />
              <NumInput label="Y" unit="mm" value={cor.offset[1]} describedBy={describedBy} invalid={invalid} onCommit={(v) => setCor({ offset: pair(cor.offset, 1, v) })} />
              <NumInput label={t('galvo.dev.rotation')} unit="°" value={cor.rotation_deg} describedBy={describedBy} invalid={invalid} onCommit={(v) => setCor({ rotation_deg: v })} />
            </div>
          )}
        </GField>
        <GField label={t('galvo.dev.mount')} help="galvo.dev.mount.help">
          {() => (
            <div className="mc-checks">
              <Check label={t('galvo.dev.invertX')} checked={cor.invert[0]} onChange={(v) => setCor({ invert: pairBool(cor.invert, 0, v) })} />
              <Check label={t('galvo.dev.invertY')} checked={cor.invert[1]} onChange={(v) => setCor({ invert: pairBool(cor.invert, 1, v) })} />
              <Check label={t('galvo.dev.swap')} checked={cor.swap_xy} onChange={(v) => setCor({ swap_xy: v })} />
            </div>
          )}
        </GField>
        <GField label={t('galvo.dev.nonlinear')} help="galvo.dev.nonlinear.help" error={issues.nonlinear}>
          {({ id, describedBy, invalid }) => (
            <div className="mc-galvo-grid">
              <NumInput id={id} label={t('galvo.dev.bulgeX')} value={cor.bulge[0]} describedBy={describedBy} invalid={invalid} onCommit={(v) => setCor({ bulge: pair(cor.bulge, 0, v) })} />
              <NumInput label={t('galvo.dev.bulgeY')} value={cor.bulge[1]} describedBy={describedBy} invalid={invalid} onCommit={(v) => setCor({ bulge: pair(cor.bulge, 1, v) })} />
              <NumInput label={t('galvo.dev.trapX')} value={cor.trapezoid[0]} describedBy={describedBy} invalid={invalid} onCommit={(v) => setCor({ trapezoid: pair(cor.trapezoid, 0, v) })} />
              <NumInput label={t('galvo.dev.trapY')} value={cor.trapezoid[1]} describedBy={describedBy} invalid={invalid} onCommit={(v) => setCor({ trapezoid: pair(cor.trapezoid, 1, v) })} />
              <NumInput label={t('galvo.dev.skewX')} value={cor.skew[0]} describedBy={describedBy} invalid={invalid} onCommit={(v) => setCor({ skew: pair(cor.skew, 0, v) })} />
              <NumInput label={t('galvo.dev.skewY')} value={cor.skew[1]} describedBy={describedBy} invalid={invalid} onCommit={(v) => setCor({ skew: pair(cor.skew, 1, v) })} />
            </div>
          )}
        </GField>
        <GField label={t('galvo.dev.corFile')} help="galvo.dev.corFile.help">
          {({ id, describedBy }) => (
            <input id={id} className="input num" value={cor.cor_file ?? ''} aria-describedby={describedBy} placeholder="lens.cor" onChange={(e) => setCor({ cor_file: e.target.value.trim() === '' ? null : e.target.value })} />
          )}
        </GField>
        {cor.cor_file && (
          <>
            <p className="warn-line" role="note">{t('galvo.dev.corFile.applied')}</p>
            <p className="hint">{t('galvo.dev.corFile.warn')}</p>
            <GField label={t('galvo.dev.corTranspose')} help="galvo.dev.corTranspose.help">
              {() => <Check label={t('galvo.dev.corTranspose')} checked={cor.cor_transpose} onChange={(v) => setCor({ cor_transpose: v })} />}
            </GField>
            <GField label={t('galvo.dev.corInvert')} help="galvo.dev.corInvert.help">
              {() => <Check label={t('galvo.dev.corInvert')} checked={cor.cor_invert} onChange={(v) => setCor({ cor_invert: v })} />}
            </GField>
          </>
        )}
        {calibrate}
      </Sub>

      <Sub title={t('galvo.dev.cylinder')} help="galvo.dev.cylinder.help">
        <Check label={t('galvo.dev.cylinder.enable')} checked={g.cylinder.enabled} onChange={(v) => set({ cylinder: { ...g.cylinder, enabled: v } })} />
        {g.cylinder.enabled && (
          <>
            <div className="field-grid">
              <GField label={t('galvo.dev.cylinder.diameter')} help="galvo.dev.cylinder.diameter.help" error={issues.cylinder_diameter}>
                {({ id, describedBy, invalid }) => <NumInput id={id} unit="mm" value={g.cylinder.diameter_mm} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ cylinder: { ...g.cylinder, diameter_mm: v } })} />}
              </GField>
              <GField label={t('galvo.dev.cylinder.distance')} help="galvo.dev.cylinder.distance.help" error={issues.cylinder_distance}>
                {({ id, describedBy, invalid }) => <NumInput id={id} unit="mm" value={g.cylinder.mirror_distance_mm} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ cylinder: { ...g.cylinder, mirror_distance_mm: v } })} />}
              </GField>
            </div>
            <div className="field-grid">
              <GField label={t('galvo.dev.axis')} help="galvo.dev.cylinder.axis.help">
                {({ id, describedBy }) => (
                  <select id={id} className="select" value={g.cylinder.axis} aria-describedby={describedBy} onChange={(e) => set({ cylinder: { ...g.cylinder, axis: e.target.value as CylinderAxis } })}>
                    {AXES.map((a) => (
                      <option key={a} value={a}>{t(`galvo.axis.${a}`)}</option>
                    ))}
                  </select>
                )}
              </GField>
              <Check label={t('galvo.dev.cylinder.concave')} checked={g.cylinder.concave} onChange={(v) => set({ cylinder: { ...g.cylinder, concave: v } })} />
            </div>
          </>
        )}
        <Check label={t('galvo.dev.taper.enable')} checked={g.taper.enabled} onChange={(v) => set({ taper: { ...g.taper, enabled: v } })} />
        {g.taper.enabled && (
          <GField label={t('galvo.dev.taper')} help="galvo.dev.taper.help" error={issues.taper}>
            {({ id, describedBy, invalid }) => (
              <>
                <div className="field-grid">
                  <NumInput id={id} label={t('galvo.dev.taper.top')} unit="mm" value={g.taper.top_diameter_mm} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ taper: { ...g.taper, top_diameter_mm: v } })} />
                  <NumInput label={t('galvo.dev.taper.bottom')} unit="mm" value={g.taper.bottom_diameter_mm} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ taper: { ...g.taper, bottom_diameter_mm: v } })} />
                  <NumInput label={t('galvo.dev.taper.length')} unit="mm" value={g.taper.length_mm} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ taper: { ...g.taper, length_mm: v } })} />
                </div>
                <div className="field-grid">
                  <select className="select" aria-label={t('galvo.dev.axis')} value={g.taper.axis} onChange={(e) => set({ taper: { ...g.taper, axis: e.target.value as CylinderAxis } })}>
                    {AXES.map((a) => (
                      <option key={a} value={a}>{t(`galvo.axis.${a}`)}</option>
                    ))}
                  </select>
                  <Check label={t('galvo.dev.taper.topPositive')} checked={g.taper.top_at_positive} onChange={(v) => set({ taper: { ...g.taper, top_at_positive: v } })} />
                </div>
              </>
            )}
          </GField>
        )}
        <GField label={t('galvo.dev.warp.show')} help={warpOn ? 'galvo.dev.warp.help' : 'galvo.dev.warp.inactive'}>
          {() => <Check label={t('galvo.dev.warp.show')} checked={warpShow} disabled={!warpOn} onChange={setWarpShow} />}
        </GField>
        <p className="hint">{t('galvo.dev.cylinder.unverified')}</p>
      </Sub>

      <Sub title={t('galvo.dev.split')} help="galvo.dev.split.help">
        <Check label={t('galvo.dev.split.enable')} checked={g.split.enabled} onChange={(v) => set({ split: { ...g.split, enabled: v } })} />
        {g.split.enabled && (
          <>
            <div className="field-grid">
              <GField label={t('galvo.dev.split.piece')} help="galvo.dev.split.piece.help" error={issues.piece_mm}>
                {({ id, describedBy, invalid }) => <NumInput id={id} unit="mm" value={g.split.piece_mm} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ split: { ...g.split, piece_mm: v } })} />}
              </GField>
              <GField label={t('galvo.dev.split.overlap')} help="galvo.dev.split.overlap.help" error={issues.overlap_mm}>
                {({ id, describedBy, invalid }) => <NumInput id={id} unit="mm" value={g.split.overlap_mm} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ split: { ...g.split, overlap_mm: v } })} />}
              </GField>
            </div>
            <div className="field-grid">
              <GField label={t('galvo.dev.split.order')} help="galvo.dev.split.order.help">
                {({ id, describedBy }) => (
                  <select id={id} className="select" value={g.split.order} aria-describedby={describedBy} onChange={(e) => set({ split: { ...g.split, order: e.target.value as SplitOrder } })}>
                    {ORDERS.map((o) => (
                      <option key={o} value={o}>{t(`galvo.split.order.${o}`)}</option>
                    ))}
                  </select>
                )}
              </GField>
              <GField label={t('galvo.dev.split.axes')} help="galvo.dev.split.axes.help">
                {({ id, describedBy }) => (
                  <select id={id} className="select" value={g.split.axes} aria-describedby={describedBy} onChange={(e) => set({ split: { ...g.split, axes: e.target.value as SplitAxes } })}>
                    {SPLIT_AXES.map((a) => (
                      <option key={a} value={a}>{t(`galvo.split.axes.${a}`)}</option>
                    ))}
                  </select>
                )}
              </GField>
            </div>
            <GField label={t('galvo.dev.split.whole')} help="galvo.dev.split.whole.help">
              {() => <Check label={t('galvo.dev.split.whole')} checked={g.split.whole_shapes} onChange={(v) => set({ split: { ...g.split, whole_shapes: v } })} />}
            </GField>
            <GField label={t('galvo.dev.split.pause')} help="galvo.dev.split.pause.help" error={issues.pause_ms}>
              {({ id, describedBy, invalid }) => <NumInput id={id} unit="ms" value={g.split.pause_after_move_ms} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ split: { ...g.split, pause_after_move_ms: v } })} />}
            </GField>
          </>
        )}
        <GField label={t('galvo.dev.repeat')} help="galvo.dev.repeat.help" error={issues.repeat_count}>
          {({ id, describedBy, invalid }) => <NumInput id={id} value={g.repeat.count} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ repeat: { ...g.repeat, count: v } })} />}
        </GField>
        {g.repeat.count > 1 && (
          <>
            <GField label={t('galvo.dev.repeat.increment')} help="galvo.dev.repeat.increment.help" error={issues.increment}>
              {({ id, describedBy, invalid }) => (
                <div className="field-grid">
                  <NumInput id={id} label="X" unit="mm" value={g.repeat.increment_mm[0]} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ repeat: { ...g.repeat, increment_mm: pair(g.repeat.increment_mm, 0, v) } })} />
                  <NumInput label="Y" unit="mm" value={g.repeat.increment_mm[1]} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ repeat: { ...g.repeat, increment_mm: pair(g.repeat.increment_mm, 1, v) } })} />
                </div>
              )}
            </GField>
            <GField label={t('galvo.dev.repeat.pause')} help="galvo.dev.repeat.pause.help" error={issues.pause_ms}>
              {({ id, describedBy, invalid }) => <NumInput id={id} unit="ms" value={g.repeat.pause_ms} describedBy={describedBy} invalid={invalid} onCommit={(v) => set({ repeat: { ...g.repeat, pause_ms: v } })} />}
            </GField>
          </>
        )}
        <p className="hint">{t('galvo.dev.noAxes')}</p>
        <p className="hint">{t('galvo.dev.split.noFly')}</p>
      </Sub>

      <GalvoRotaryFields draft={draft} edit={edit} issues={issues} />
    </>
  );
}

export type { GalvoField };
