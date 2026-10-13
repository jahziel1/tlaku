import { House, LockOpen, OctagonAlert } from 'lucide-react';
import { useMemo } from 'react';
import { t, type I18nKey } from '@/i18n';
import { useStore } from '@/store';
import { homingEnabled, selectFirmwareFamily } from '@/store/machine';
import { alarmAdvice, alarmKey, type AlarmAction, type AlarmTip } from './grblCodes';

const TIP_KEY: Record<AlarmTip, I18nKey> = {
  moveAway: 'machine.alarm.tip.moveAway',
  moveAwayThenUnlock: 'machine.alarm.tip.moveAwayThenUnlock',
  homeAfter: 'machine.alarm.tip.homeAfter',
  checkLimits: 'machine.alarm.tip.checkLimits',
  homingOff: 'machine.alarm.tip.homingOff',
  homingOn: 'machine.alarm.tip.homingOn',
  ifLimitSwitches: 'machine.alarm.tip.ifLimitSwitches',
  ifNoLimitSwitches: 'machine.alarm.tip.ifNoLimitSwitches',
  positionLost: 'machine.alarm.tip.positionLost',
};

const ACTION_LABEL: Record<AlarmAction, I18nKey> = { unlock: 'machine.alarm.unlock', home: 'machine.alarm.home' };

/**
 * Banner de alarma: explica la causa en español y sugiere $X o $H según el homing de la máquina.
 * Nunca envía nada por su cuenta: cada botón abre un diálogo que explica el riesgo.
 */
export function AlarmBanner({ compact }: { compact?: boolean }) {
  // Solo valores primitivos: un selector que devuelve un objeto nuevo en cada llamada entra en bucle.
  const inAlarm = useStore((s) => s.status?.state === 'alarm');
  const code = useStore((s) => s.alarmCode);
  const homing = useStore((s) => homingEnabled(s.settings));
  const family = useStore(selectFirmwareFamily);
  const openDialog = useStore((s) => s.openDialog);
  const advice = useMemo(() => alarmAdvice(code, homing), [code, homing]);
  if (!inAlarm) return null;
  const alarm = { code };

  return (
    <div className="mc-alarm" role="alert">
      <div className="mc-alarm__head">
        <OctagonAlert size={18} aria-hidden="true" />
        <div>
          <p className="mc-alarm__title">{t('machine.alarm.title')}</p>
          <p>
            {alarm.code !== null && <span className="num mc-alarm__code">ALARM:{alarm.code}</span>}{' '}
            {t(alarmKey(alarm.code, family))}
          </p>
        </div>
      </div>
      {!compact && (
        <>
          <p className="mc-alarm__lead">{t('machine.alarm.whatToDo')}</p>
          <ul className="mc-alarm__tips">
            {advice.tips.map((tip) => (
              <li key={tip}>{t(TIP_KEY[tip])}</li>
            ))}
            {homing === null && <li>{t('machine.alarm.homingUnknown')}</li>}
          </ul>
        </>
      )}
      <div className="row mc-alarm__actions">
        {advice.actions.map((a) => (
          <button
            key={a}
            type="button"
            className={`btn grow ${a === advice.recommended ? 'btn--accent' : ''}`}
            onClick={() => openDialog({ kind: 'alarm', action: a })}
          >
            {a === 'unlock' ? <LockOpen size={14} aria-hidden="true" /> : <House size={14} aria-hidden="true" />}
            {t(ACTION_LABEL[a])}
          </button>
        ))}
      </div>
      {!compact && <p className="hint">{t('machine.alarm.confirmNote')}</p>}
    </div>
  );
}
