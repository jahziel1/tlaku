import { TriangleAlert } from 'lucide-react';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { homingEnabled } from '@/store/machine';
import type { AlarmAction } from './grblCodes';
import { Modal } from './Modal';

/** Confirmación de `$X` o `$H`: explica el riesgo y muestra exactamente lo que se enviará. */
export function AlarmActionDialog({ action }: { action: AlarmAction }) {
  const close = useStore((s) => s.closeDialog);
  const confirm = useStore((s) => s.confirmAlarmAction);
  const homing = useStore((s) => homingEnabled(s.settings));
  const line = action === 'unlock' ? '$X' : '$H';

  return (
    <Modal
      title={t(action === 'unlock' ? 'machine.alarm.unlock.title' : 'machine.alarm.home.title')}
      tone="danger"
      onClose={close}
      footer={
        <>
          <button type="button" className="btn" data-autofocus onClick={close}>{t('machine.dlg.cancel')}</button>
          <button type="button" className="btn btn--danger" onClick={() => void confirm(action)}>
            {t('machine.alarm.send', { line })}
          </button>
        </>
      }
    >
      <p>{t(action === 'unlock' ? 'machine.risk.unlock' : 'machine.risk.home')}</p>
      <ul className="mc-list">
        {(action === 'unlock'
          ? (['machine.alarm.unlock.check1', 'machine.alarm.unlock.check2'] as const)
          : (['machine.alarm.home.check1', 'machine.alarm.home.check2'] as const)
        ).map((k) => (
          <li key={k}>{t(k)}</li>
        ))}
      </ul>
      {action === 'home' && homing === false && (
        <p className="warn-line">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('machine.alarm.home.noHoming')}</span>
        </p>
      )}
      <p>{t('machine.confirm.exact')}</p>
      <pre className="mc-code num">{line}</pre>
    </Modal>
  );
}
