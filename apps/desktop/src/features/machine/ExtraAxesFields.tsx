import type { DeviceProfile } from '@/api/types';
import { t } from '@/i18n';
import { Field, NumInput } from './DeviceFields';

/** Casillas del eje U y del segundo láser: solo habilitan las opciones que los usan (jog de U, láser por capa). */
export function ExtraAxesFields({ draft, edit }: { draft: DeviceProfile; edit: (patch: Partial<DeviceProfile>) => void }) {
  return (
    <>
      <Field label={t('machine.dev.uAxis')} help="machine.dev.uAxis.help">
        {({ id, describedBy }) => (
          <label className="mc-check">
            <input id={id} type="checkbox" checked={draft.u_axis} aria-describedby={describedBy} onChange={(e) => edit({ u_axis: e.target.checked })} />
            <span>{t('machine.dev.uAxis.label')}</span>
          </label>
        )}
      </Field>
      {draft.u_axis && (
        <Field label={t('machine.dev.uOffset')} help="machine.dev.uOffset.help">
          {({ id, describedBy }) => <NumInput id={id} unit="mm" value={draft.u_offset} describedBy={describedBy} onCommit={(v) => edit({ u_offset: v })} />}
        </Field>
      )}
      <Field label={t('machine.dev.laser2')} help="machine.dev.laser2.help">
        {({ id, describedBy }) => (
          <label className="mc-check">
            <input id={id} type="checkbox" checked={draft.laser2} aria-describedby={describedBy} onChange={(e) => edit({ laser2: e.target.checked })} />
            <span>{t('machine.dev.laser2.label')}</span>
          </label>
        )}
      </Field>
    </>
  );
}
