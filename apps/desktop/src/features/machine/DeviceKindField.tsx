import { TriangleAlert } from 'lucide-react';
import { t } from '@/i18n';
import {
  dialectSummary,
  DEVICE_KINDS,
  EXPORT_ONLY_KINDS,
  type DeviceKind,
  type KindChange,
} from '@/lib/device';
import type { DeviceProfile } from '@/api/types';
import { Field } from './DeviceFields';
import { Modal } from './Modal';
import './DeviceDialect.css';

/** Selector del tipo de controladora, con el aviso de los tipos que todavía no se pueden enviar. */
export function KindField({ kind, onChange }: { kind: DeviceKind; onChange: (kind: DeviceKind) => void }) {
  return (
    <div className="dv-kind">
      <Field label={t('machine.dev.kind')} help="devices.kind.help">
        {({ id, describedBy }) => (
          <select id={id} className="select" value={kind} aria-describedby={describedBy} onChange={(e) => onChange(e.target.value as DeviceKind)}>
            {DEVICE_KINDS.map((k) => (
              <option key={k} value={k}>
                {EXPORT_ONLY_KINDS.has(k) ? `${t(`devices.kind.${k}`)} (${t('devices.kind.exportOnly')})` : t(`devices.kind.${k}`)}
              </option>
            ))}
          </select>
        )}
      </Field>
      {EXPORT_ONLY_KINDS.has(kind) && (
        <p className="warn-line" role="status">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('devices.kind.exportOnly.hint')}</span>
        </p>
      )}
      {kind === 'custom_gcode' && <p className="hint">{t('devices.kind.custom.hint')}</p>}
    </div>
  );
}

/** Cómo se ve el valor de un campo del perfil en la tabla de la confirmación. */
function valueText(draft: DeviceProfile, change: KindChange, which: 'from' | 'to'): string {
  const v = change[which];
  if (change.field === 'dialect') return dialectSummary({ dialect: v as DeviceProfile['dialect'], s_max: draft.s_max }, false);
  if (change.field === 'power_mode') return t(v === 'dynamic' ? 'machine.dev.powerMode.dynamic' : 'machine.dev.powerMode.constant');
  return String(v);
}

const FIELD_LABEL = {
  dialect: 'devices.kind.dlg.dialect',
  s_max: 'machine.dev.sMax',
  power_mode: 'machine.dev.powerMode',
  baud: 'machine.baud',
} as const;

/** Qué se pisaría al aplicar los valores del tipo: campo, valor actual y valor nuevo. */
export function KindChangeBody({ draft, changes }: { draft: DeviceProfile; changes: KindChange[] }) {
  return (
    <>
      <p>{t('devices.kind.dlg.text')}</p>
      <table className="mc-table dv-kindtable">
        <thead>
          <tr>
            <th scope="col">{t('devices.kind.dlg.field')}</th>
            <th scope="col">{t('devices.kind.dlg.current')}</th>
            <th scope="col">{t('devices.kind.dlg.new')}</th>
          </tr>
        </thead>
        <tbody>
          {changes.map((c) => (
            <tr key={c.field}>
              <th scope="row">{t(FIELD_LABEL[c.field])}</th>
              <td className="num">{valueText(draft, c, 'from')}</td>
              <td className="num mc-new">{valueText(draft, c, 'to')}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

/**
 * Confirmación al cambiar el tipo: aplicar los valores de partida del tipo pisa campos del perfil
 * (dialecto, S máxima, modo de potencia y baudios), así que se muestran antes de hacerlo.
 */
export function KindChangeDialog({
  draft,
  kind,
  changes,
  onApply,
  onKindOnly,
  onCancel,
}: {
  draft: DeviceProfile;
  kind: DeviceKind;
  changes: KindChange[];
  onApply: () => void;
  onKindOnly: () => void;
  onCancel: () => void;
}) {
  return (
    <Modal
      title={t('devices.kind.dlg.title', { kind: t(`devices.kind.${kind}`) })}
      tone="warn"
      size="md"
      onClose={onCancel}
      footer={
        <>
          <button type="button" className="btn" onClick={onCancel}>{t('machine.dlg.cancel')}</button>
          <span className="topbar__spacer" />
          <button type="button" className="btn" title={t('devices.kind.dlg.only.help')} onClick={onKindOnly}>{t('devices.kind.dlg.only')}</button>
          <button type="button" className="btn btn--accent" data-autofocus onClick={onApply}>{t('devices.kind.dlg.apply')}</button>
        </>
      }
    >
      <KindChangeBody draft={draft} changes={changes} />
      <p className="hint">{t('devices.kind.dlg.only.help')}</p>
    </Modal>
  );
}
