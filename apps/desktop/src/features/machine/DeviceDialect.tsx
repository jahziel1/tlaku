import { TriangleAlert } from 'lucide-react';
import { useMemo, type ReactNode } from 'react';
import type { DeviceProfile } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { dialectSummary, dialectWarnings, sampleGcode, type DialectField, type DialectIssue } from '@/lib/device';
import { Field, NumInput } from './DeviceFields';
import './DeviceDialect.css';

type Dialect = DeviceProfile['dialect'];
type Issues = Partial<Record<DialectField, DialectIssue>>;

interface Props {
  draft: DeviceProfile;
  edit: (patch: Partial<DeviceProfile>) => void;
  issues: Issues;
}

/** Texto de una orden (una línea) del dialecto. */
function Cmd({
  label,
  help,
  value,
  onChange,
  editable,
  error,
}: {
  label: I18nKey;
  help: I18nKey;
  value: string;
  onChange: (v: string) => void;
  editable: boolean;
  error?: DialectIssue;
}) {
  return (
    <Field label={t(label)} help={help} error={error}>
      {({ id, describedBy, invalid }) => (
        <input
          id={id}
          className="input num"
          value={value}
          spellCheck={false}
          readOnly={!editable}
          aria-describedby={describedBy}
          aria-invalid={invalid || undefined}
          onChange={(e) => onChange(e.target.value)}
        />
      )}
    </Field>
  );
}

function Check({ label, text, help, checked, onChange, editable }: { label: I18nKey; text: I18nKey; help: I18nKey; checked: boolean; onChange: (v: boolean) => void; editable: boolean }) {
  return (
    <Field label={t(label)} help={help}>
      {({ id, describedBy }) => (
        <label className="mc-check">
          <input id={id} type="checkbox" checked={checked} disabled={!editable} aria-describedby={describedBy} onChange={(e) => onChange(e.target.checked)} />
          <span>{t(text)}</span>
        </label>
      )}
    </Field>
  );
}

function Group({ title, children }: { title: I18nKey; children: ReactNode }) {
  return (
    <fieldset className="dv-sub">
      <legend>{t(title)}</legend>
      {children}
    </fieldset>
  );
}

/**
 * Dialecto de G-code del perfil. Siempre se ve el resumen, las órdenes y una vista previa; solo
 * se pueden editar con el tipo «G-code personalizado» (en los demás tipos lo fija el firmware).
 */
export function DialectEditor({ draft, edit, issues }: Props) {
  const d = draft.dialect;
  const editable = draft.kind === 'custom_gcode';
  const set = (patch: Partial<Dialect>) => edit({ dialect: { ...d, ...patch } });
  /** Orden opcional: vacía se guarda como nula (sin aire, sin cierre). */
  const optional = (field: 'air_on' | 'air_off' | 'end_sync') => (v: string) => set({ [field]: v === '' ? null : v });
  const warnings = editable ? dialectWarnings(draft) : [];
  const { s_max, power_mode, rapid_rate, start_gcode, end_gcode, return_to_origin } = draft;
  const preview = useMemo(
    () => sampleGcode({ dialect: d, s_max, power_mode, rapid_rate, start_gcode, end_gcode, return_to_origin }, { label: t('devices.preview.label') }).join('\n'),
    [d, s_max, power_mode, rapid_rate, start_gcode, end_gcode, return_to_origin],
  );

  return (
    <>
      <p className="dv-summary" aria-label={t('devices.summary.label')}>{dialectSummary(draft)}</p>
      <p className="hint">{t(editable ? 'devices.dialect.editing' : 'devices.dialect.readonly')}</p>
      {warnings.map((w) => (
        <p key={w} className="warn-line" role="status">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t(`devices.dialect.warn.${w}`)}</span>
        </p>
      ))}

      <details className="dv-details" open={editable || undefined}>
        <summary>{t('devices.dialect.details')}</summary>
        <div className="dv-groups">
          <Group title="devices.dialect.group.power">
            <Check label="devices.dialect.sInline" text="devices.dialect.sInline.label" help="devices.dialect.sInline.help" checked={d.s_inline} editable={editable} onChange={(v) => set({ s_inline: v })} />
            {!d.s_inline && (
              <Cmd label="devices.dialect.powerCmd" help="devices.dialect.powerCmd.help" value={d.power_cmd} editable={editable} error={issues.power_cmd} onChange={(v) => set({ power_cmd: v })} />
            )}
            <Field label={t('devices.dialect.sDecimals')} help="devices.dialect.sDecimals.help" error={issues.s_decimals}>
              {({ id, describedBy, invalid }) => <NumInput id={id} value={d.s_decimals} describedBy={describedBy} invalid={invalid} readOnly={!editable} onCommit={(v) => set({ s_decimals: v })} />}
            </Field>
          </Group>

          <Group title="devices.dialect.group.on">
            <Cmd label="devices.dialect.constantOn" help="devices.dialect.constantOn.help" value={d.constant_on} editable={editable} error={issues.constant_on} onChange={(v) => set({ constant_on: v })} />
            <Check
              label="devices.dialect.dynamicHas"
              text="devices.dialect.dynamicHas.label"
              help="devices.dialect.dynamicHas.help"
              checked={d.dynamic_on !== null}
              editable={editable}
              onChange={(v) => set({ dynamic_on: v ? 'M4 S0' : null })}
            />
            {d.dynamic_on !== null && (
              <Cmd label="devices.dialect.dynamicOn" help="devices.dialect.dynamicOn.help" value={d.dynamic_on} editable={editable} error={issues.dynamic_on} onChange={(v) => set({ dynamic_on: v })} />
            )}
            <Cmd label="devices.dialect.laserOff" help="devices.dialect.laserOff.help" value={d.laser_off} editable={editable} error={issues.laser_off} onChange={(v) => set({ laser_off: v })} />
          </Group>

          <Group title="devices.dialect.group.moves">
            <Check label="devices.dialect.g0Off" text="devices.dialect.g0Off.label" help="devices.dialect.g0Off.help" checked={d.g0_laser_off} editable={editable} onChange={(v) => set({ g0_laser_off: v })} />
            <Check label="devices.dialect.travelS0" text="devices.dialect.travelS0.label" help="devices.dialect.travelS0.help" checked={d.travel_s0} editable={editable} onChange={(v) => set({ travel_s0: v })} />
            <Check label="devices.dialect.feedOnG0" text="devices.dialect.feedOnG0.label" help="devices.dialect.feedOnG0.help" checked={d.feed_on_g0} editable={editable} onChange={(v) => set({ feed_on_g0: v })} />
          </Group>

          <Group title="devices.dialect.group.air">
            <div className="field-grid">
              <Cmd label="devices.dialect.airOn" help="devices.dialect.airOn.help" value={d.air_on ?? ''} editable={editable} error={issues.air_on} onChange={optional('air_on')} />
              <Cmd label="devices.dialect.airOff" help="devices.dialect.airOff.help" value={d.air_off ?? ''} editable={editable} error={issues.air_off} onChange={optional('air_off')} />
            </div>
          </Group>

          <Group title="devices.dialect.group.end">
            <Cmd label="devices.dialect.endSync" help="devices.dialect.endSync.help" value={d.end_sync ?? ''} editable={editable} error={issues.end_sync} onChange={optional('end_sync')} />
          </Group>

          <Group title="devices.dialect.group.format">
            <div className="field-grid">
              <Field label={t('devices.dialect.decimals')} help="devices.dialect.decimals.help" error={issues.decimals}>
                {({ id, describedBy, invalid }) => <NumInput id={id} value={d.decimals} describedBy={describedBy} invalid={invalid} readOnly={!editable} onCommit={(v) => set({ decimals: v })} />}
              </Field>
              <Field label={t('devices.dialect.maxLineLen')} help="devices.dialect.maxLineLen.help" error={issues.max_line_len}>
                {({ id, describedBy, invalid }) => <NumInput id={id} unit="B" value={d.max_line_len} describedBy={describedBy} invalid={invalid} readOnly={!editable} onCommit={(v) => set({ max_line_len: v })} />}
              </Field>
            </div>
            <Field label={t('devices.dialect.comment')} help="devices.dialect.comment.help">
              {({ id, describedBy }) => (
                <select id={id} className="select" value={d.comment} disabled={!editable} aria-describedby={describedBy} onChange={(e) => set({ comment: e.target.value as Dialect['comment'] })}>
                  <option value="semicolon">{t('devices.dialect.comment.semicolon')}</option>
                  <option value="parentheses">{t('devices.dialect.comment.parentheses')}</option>
                </select>
              )}
            </Field>
          </Group>
        </div>
      </details>

      <h3 className="mc-sub">{t('devices.preview.title')}</h3>
      <pre className="dv-preview num" aria-label={t('devices.preview.title')} tabIndex={0}>{preview}</pre>
      <p className="hint">{t('devices.preview.hint')}</p>
    </>
  );
}
