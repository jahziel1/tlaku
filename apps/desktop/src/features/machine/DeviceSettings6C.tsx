import { ChevronDown, ChevronUp } from 'lucide-react';
import type { DeviceProfile } from '@/api/types';
import { t } from '@/i18n';
import { speedFromDisplay, speedToDisplay, speedUnitLabel, SPEED_UNITS, isGrblFamily } from '@/lib/device';
import { DEVICE_ICONS, defaultIconId } from './deviceIcons';
import { Field, NumInput } from './DeviceFields';
import './DeviceSettings6C.css';

type Edit = (patch: Partial<DeviceProfile>) => void;

/** Velocidad que se muestra y se escribe en la unidad del perfil (mm/min o mm/s); el perfil guarda mm/min. */
export function SpeedInput({
  id,
  value,
  unit,
  label,
  describedBy,
  invalid,
  onCommit,
}: {
  id?: string;
  value: number;
  unit: DeviceProfile['speed_unit'];
  label?: string;
  describedBy?: string;
  invalid?: boolean;
  onCommit: (mmPerMin: number) => void;
}) {
  const shown = Math.round(speedToDisplay(value, unit) * 1000) / 1000;
  return (
    <NumInput
      id={id}
      label={label}
      unit={speedUnitLabel(unit)}
      value={shown}
      describedBy={describedBy}
      invalid={invalid}
      onCommit={(v) => onCommit(speedFromDisplay(v, unit))}
    />
  );
}

/** Ícono del perfil y marca de «perfil predeterminado» (el que se activa al abrir la app). */
export function DeviceIdentityFields({ draft, edit }: { draft: DeviceProfile; edit: Edit }) {
  const current = draft.icon || defaultIconId(draft.kind);
  return (
    <>
      <Field label={t('devices.icon.label')} help="devices.icon.help">
        {({ describedBy }) => (
          <div className="mc-iconpick" role="radiogroup" aria-label={t('devices.icon.label')} aria-describedby={describedBy}>
            {DEVICE_ICONS.map(({ id, Icon, label }) => (
              <button
                key={id}
                type="button"
                role="radio"
                aria-checked={current === id}
                aria-label={t(label)}
                title={t(label)}
                className="mc-iconpick__btn"
                onClick={() => edit({ icon: id })}
              >
                <Icon size={16} aria-hidden="true" />
              </button>
            ))}
          </div>
        )}
      </Field>
      <Field label={t('devices.default.title')} help="devices.default.help">
        {({ id, describedBy }) => (
          <label className="mc-check">
            <input id={id} type="checkbox" checked={draft.is_default} aria-describedby={describedBy} onChange={(e) => edit({ is_default: e.target.checked })} />
            <span>{t('devices.default.label')}</span>
          </label>
        )}
      </Field>
    </>
  );
}

/** Subir y bajar el perfil seleccionado en la lista (el orden se guarda en `devices.json`). */
export function DeviceOrderButtons({ index, count, disabled, onMove }: { index: number; count: number; disabled: boolean; onMove: (to: number) => void }) {
  return (
    <div className="mc-devlist__order">
      <button type="button" className="btn grow" disabled={disabled || index <= 0} title={t('devices.order.up')} aria-label={t('devices.order.up')} onClick={() => onMove(index - 1)}>
        <ChevronUp size={14} aria-hidden="true" />
      </button>
      <button type="button" className="btn grow" disabled={disabled || index < 0 || index >= count - 1} title={t('devices.order.down')} aria-label={t('devices.order.down')} onClick={() => onMove(index + 1)}>
        <ChevronDown size={14} aria-hidden="true" />
      </button>
    </div>
  );
}

/** Área: ejes cruzados (intercambio X/Y), desfase del puntero rojo y unidad con la que se muestran las velocidades. */
export function DeviceFrameFields({ draft, edit }: { draft: DeviceProfile; edit: Edit }) {
  const [px, py] = draft.pointer_offset;
  return (
    <>
      <Field label={t('devices.swapXy.title')} help="devices.swapXy.help">
        {({ id, describedBy }) => (
          <label className="mc-check">
            <input id={id} type="checkbox" checked={draft.swap_xy} aria-describedby={describedBy} onChange={(e) => edit({ swap_xy: e.target.checked })} />
            <span>{t('devices.swapXy.label')}</span>
          </label>
        )}
      </Field>
      <Field label={t('devices.pointer.title')} help="devices.pointer.help">
        {({ id, describedBy }) => (
          <div className="field-grid">
            <NumInput id={id} label={t('devices.pointer.x')} unit="mm" value={px} describedBy={describedBy} onCommit={(v) => edit({ pointer_offset: [v, py] })} />
            <NumInput label={t('devices.pointer.y')} unit="mm" value={py} describedBy={describedBy} onCommit={(v) => edit({ pointer_offset: [px, v] })} />
          </div>
        )}
      </Field>
      <Field label={t('devices.speedUnit.title')} help="devices.speedUnit.help">
        {({ id, describedBy }) => (
          <select id={id} className="select" value={draft.speed_unit} aria-describedby={describedBy} onChange={(e) => edit({ speed_unit: e.target.value as DeviceProfile['speed_unit'] })}>
            {SPEED_UNITS.map((u) => (
              <option key={u} value={u}>{speedUnitLabel(u)}</option>
            ))}
          </select>
        )}
      </Field>
    </>
  );
}

/** Enlace con GRBL: homing al conectar, DTR y envío síncrono. Solo aplican a la familia GRBL. */
export function DeviceLinkFields({ draft, edit }: { draft: DeviceProfile; edit: Edit }) {
  if (!isGrblFamily(draft.kind)) return null;
  const serial = (draft.connection?.type ?? 'serial') === 'serial';
  return (
    <>
      <Field label={t('devices.homeOnConnect.title')} help="devices.homeOnConnect.help">
        {({ id, describedBy }) => (
          <label className="mc-check">
            <input id={id} type="checkbox" checked={draft.home_on_connect} aria-describedby={describedBy} onChange={(e) => edit({ home_on_connect: e.target.checked })} />
            <span>{t('devices.homeOnConnect.label')}</span>
          </label>
        )}
      </Field>
      {serial && (
        <Field label={t('devices.noDtr.title')} help="devices.noDtr.help">
          {({ id, describedBy }) => (
            <label className="mc-check">
              <input id={id} type="checkbox" checked={draft.no_dtr_reset} aria-describedby={describedBy} onChange={(e) => edit({ no_dtr_reset: e.target.checked })} />
              <span>{t('devices.noDtr.label')}</span>
            </label>
          )}
        </Field>
      )}
      <Field label={t('devices.syncStream.title')} help="devices.syncStream.help">
        {({ id, describedBy }) => (
          <label className="mc-check">
            <input id={id} type="checkbox" checked={draft.sync_streaming} aria-describedby={describedBy} onChange={(e) => edit({ sync_streaming: e.target.checked })} />
            <span>{t('devices.syncStream.label')}</span>
          </label>
        )}
      </Field>
    </>
  );
}
