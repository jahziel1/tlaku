import { Plus, Trash2 } from 'lucide-react';
import { useId, type ReactNode } from 'react';
import type { DeviceProfile, RotaryKind, RotarySettings, ScanOffset } from '@/api/types';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import {
  applyRotaryPreset,
  matchRotaryPreset,
  ROTARY_PRESETS,
  rotaryCircumference,
  scanOffsetRowIssues,
  sortScanOffsets,
  type DeviceField,
  type DeviceIssue,
} from '@/lib/device';
import { Field, NumInput } from './DeviceFields';

type Issues = Partial<Record<DeviceField, DeviceIssue>>;
/** Problemas que `validateDevice` informa para la tabla de barrido. */
export type ScanIssue = Extract<DeviceIssue, 'required' | 'positive' | 'duplicate'>;

/** Paso con el que se propone la velocidad de una fila nueva de la tabla de barrido. */
const NEW_ROW_SPEED_STEP = 1000;
const NEW_ROW_FIRST_SPEED = 3000;

/**
 * Casilla del eje Z motorizado (habilita en las capas el desplazamiento y la bajada por pasada) y,
 * con él, la de invertir el sentido de Z en el G-code.
 */
export function ZAxisField({ draft, edit }: { draft: DeviceProfile; edit: (patch: Partial<DeviceProfile>) => void }) {
  return (
    <>
      <Field label={t('machine.dev.zAxis')} help="machine.dev.zAxis.help">
        {({ id, describedBy }) => (
          <label className="mc-check">
            <input id={id} type="checkbox" checked={draft.z_axis} aria-describedby={describedBy} onChange={(e) => edit({ z_axis: e.target.checked })} />
            <span>{t('machine.dev.zAxis.label')}</span>
          </label>
        )}
      </Field>
      {draft.z_axis && (
        <Field label={t('devices.zInvert')} help="devices.zInvert.help">
          {({ id, describedBy }) => (
            <label className="mc-check">
              <input id={id} type="checkbox" checked={draft.z_invert} aria-describedby={describedBy} onChange={(e) => edit({ z_invert: e.target.checked })} />
              <span>{t('devices.zInvert.label')}</span>
            </label>
          )}
        </Field>
      )}
    </>
  );
}

/** Rotativo en lugar del eje Y: modelo, tipo, mm por vuelta, diámetros y espejo. `children` va al final (la prueba de una vuelta). */
export function RotaryFields({
  draft,
  edit,
  issues,
  children,
}: {
  draft: DeviceProfile;
  edit: (patch: Partial<DeviceProfile>) => void;
  issues: Issues;
  children?: ReactNode;
}) {
  const r = draft.rotary;
  const preset = matchRotaryPreset(r);
  const turn = rotaryCircumference(r);
  const setRotary = (patch: Partial<RotarySettings>) => edit({ rotary: { ...r, ...patch } });
  return (
    <>
      <Field label={t('machine.dev.rotary')} help="machine.dev.rotary.help">
        {({ id, describedBy }) => (
          <label className="mc-check">
            <input id={id} type="checkbox" checked={r.enabled} aria-describedby={describedBy} onChange={(e) => setRotary({ enabled: e.target.checked })} />
            <span>{t('machine.dev.rotary.label')}</span>
          </label>
        )}
      </Field>
      {r.enabled && (
        <>
          <Field label={t('devices.rotary.preset')} help="devices.rotary.preset.help">
            {({ id, describedBy }) => (
              <select
                id={id}
                className="select"
                value={preset?.id ?? ''}
                aria-describedby={describedBy}
                onChange={(e) => {
                  const chosen = ROTARY_PRESETS.find((p) => p.id === e.target.value);
                  if (chosen) setRotary(applyRotaryPreset(r, chosen));
                }}
              >
                <option value="">{t('devices.rotary.preset.choose')}</option>
                {ROTARY_PRESETS.map((p) => (
                  <option key={p.id} value={p.id}>{t(`devices.rotary.preset.${p.id}`)}</option>
                ))}
              </select>
            )}
          </Field>
          <div className="field-grid">
            <Field label={t('machine.dev.rotary.kind')} help="machine.dev.rotary.kind.help">
              {({ id, describedBy }) => (
                <select id={id} className="select" value={r.kind} aria-describedby={describedBy} onChange={(e) => setRotary({ kind: e.target.value as RotaryKind })}>
                  <option value="roller">{t('machine.dev.rotary.roller')}</option>
                  <option value="chuck">{t('machine.dev.rotary.chuck')}</option>
                </select>
              )}
            </Field>
            <Field label={t('machine.dev.rotary.mmRot')} help="machine.dev.rotary.mmRot.help" error={issues.mm_per_rotation}>
              {({ id, describedBy, invalid }) => <NumInput id={id} unit="mm" value={r.mm_per_rotation} describedBy={describedBy} invalid={invalid} onCommit={(v) => setRotary({ mm_per_rotation: v })} />}
            </Field>
          </div>
          <div className="field-grid">
            {r.kind === 'roller' && (
              <Field label={t('machine.dev.rotary.roller.d')} help="machine.dev.rotary.roller.d.help" error={issues.roller_diameter}>
                {({ id, describedBy, invalid }) => <NumInput id={id} unit="mm" value={r.roller_diameter} describedBy={describedBy} invalid={invalid} onCommit={(v) => setRotary({ roller_diameter: v })} />}
              </Field>
            )}
            <Field label={t('machine.dev.rotary.object.d')} help="machine.dev.rotary.object.d.help" error={issues.object_diameter}>
              {({ id, describedBy, invalid }) => <NumInput id={id} unit="mm" value={r.object_diameter} describedBy={describedBy} invalid={invalid} onCommit={(v) => setRotary({ object_diameter: v })} />}
            </Field>
          </div>
          {turn !== null && <p className="hint">{t('devices.rotary.turn', { mm: fmt(turn, 1) })}</p>}
          <Field label={t('devices.rotary.mirror')} help="devices.rotary.mirror.help">
            {({ id, describedBy }) => (
              <label className="mc-check">
                <input id={id} type="checkbox" checked={r.mirror} aria-describedby={describedBy} onChange={(e) => setRotary({ mirror: e.target.checked })} />
                <span>{t('devices.rotary.mirror.label')}</span>
              </label>
            )}
          </Field>
          {children}
        </>
      )}
    </>
  );
}

/** Tabla de corrección de barrido: desfase de ida y vuelta medido a cada velocidad. */
export function ScanOffsetsTable({ rows, onChange, error }: { rows: ScanOffset[]; onChange: (rows: ScanOffset[]) => void; error?: ScanIssue }) {
  const helpId = useId();
  const errId = useId();
  const rowIssues = scanOffsetRowIssues(rows);
  const update = (i: number, patch: Partial<ScanOffset>) => onChange(sortScanOffsets(rows.map((r, j) => (j === i ? { ...r, ...patch } : r))));
  const add = () => {
    const top = rows.reduce((m, r) => Math.max(m, r.speed), 0);
    onChange(sortScanOffsets([...rows, { speed: top > 0 ? top + NEW_ROW_SPEED_STEP : NEW_ROW_FIRST_SPEED, offset: 0 }]));
  };
  const describedBy = [error ? errId : '', helpId].filter(Boolean).join(' ');
  return (
    <div className="mc-field" data-invalid={error || undefined}>
      <p id={helpId} className="hint mc-help">{t('machine.dev.scan.help')}</p>
      <p className="hint mc-help">{t('devices.scan.measured')}</p>
      {rows.length === 0 ? (
        <p className="hint">{t('machine.dev.scan.empty')}</p>
      ) : (
        <table className="mc-table" aria-describedby={describedBy}>
          <thead>
            <tr>
              <th scope="col">{t('machine.dev.scan.speed')}</th>
              <th scope="col">{t('machine.dev.scan.offset')}</th>
              <th scope="col"><span className="sr-only">{t('machine.dev.scan.remove')}</span></th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={i}>
                <td>
                  <NumInput label={t('machine.dev.scan.speedRow', { n: i + 1 })} unit="mm/min" value={r.speed} invalid={rowIssues[i] === 'positive' || rowIssues[i] === 'duplicate'} onCommit={(v) => update(i, { speed: v })} />
                </td>
                <td>
                  <NumInput label={t('machine.dev.scan.offsetRow', { n: i + 1 })} unit="mm" value={r.offset} invalid={rowIssues[i] === 'required'} onCommit={(v) => update(i, { offset: v })} />
                </td>
                <td>
                  <button type="button" className="icon-btn" aria-label={t('machine.dev.scan.removeRow', { n: i + 1 })} title={t('machine.dev.scan.remove')} onClick={() => onChange(rows.filter((_, j) => j !== i))}>
                    <Trash2 size={14} aria-hidden="true" />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {error && <p id={errId} className="hint hint--danger" role="alert">{t(`machine.dev.scan.err.${error}`)}</p>}
      <div className="row">
        <button type="button" className="btn" onClick={add}><Plus size={14} aria-hidden="true" />{t('machine.dev.scan.add')}</button>
      </div>
    </div>
  );
}

/** Desfase inicial del barrido (`scan_initial_offset`): corre todas las líneas, también las unidireccionales. */
export function ScanInitialOffsetField({ draft, edit, issues }: { draft: DeviceProfile; edit: (patch: Partial<DeviceProfile>) => void; issues: Issues }) {
  return (
    <Field label={t('devices.scan.initial')} help="devices.scan.initial.help" error={issues.scan_initial_offset}>
      {({ id, describedBy, invalid }) => <NumInput id={id} unit="mm" value={draft.scan_initial_offset} describedBy={describedBy} invalid={invalid} onCommit={(v) => edit({ scan_initial_offset: v })} />}
    </Field>
  );
}
