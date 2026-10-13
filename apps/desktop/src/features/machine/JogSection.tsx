import {
  ArrowDown, ArrowDownLeft, ArrowDownRight, ArrowLeft, ArrowRight, ArrowUp, ArrowUpLeft, ArrowUpRight,
  Crosshair, House, MapPin, X, type LucideIcon,
} from 'lucide-react';
import { useCommand } from '@/commands/useCommand';
import { NumberField } from '@/ui/NumberField';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { jogBlockedReason } from '@/store/machine';
import { JOG_DIRS, JOG_STEPS, type JogDirName } from './jogMath';
import { onPadKeyDown, useHoldJog } from './useHoldJog';

const ICONS: Record<JogDirName, LucideIcon> = {
  upLeft: ArrowUpLeft,
  up: ArrowUp,
  upRight: ArrowUpRight,
  left: ArrowLeft,
  right: ArrowRight,
  downLeft: ArrowDownLeft,
  down: ArrowDown,
  downRight: ArrowDownRight,
};

/** Las nueve celdas del pad, fila por fila. */
const GRID: (JogDirName | 'center')[] = ['upLeft', 'up', 'upRight', 'left', 'center', 'right', 'downLeft', 'down', 'downRight'];

const DIR_LABEL: Record<JogDirName, I18nKey> = {
  upLeft: 'machine.jog.upLeft',
  up: 'machine.jog.up',
  upRight: 'machine.jog.upRight',
  left: 'machine.jog.left',
  right: 'machine.jog.right',
  downLeft: 'machine.jog.downLeft',
  down: 'machine.jog.down',
  downRight: 'machine.jog.downRight',
};

/** Mover el cabezal: pad de 8 direcciones y centro, pasos, velocidad, home y origen de usuario. */
export function JogSection() {
  const whyKey = useStore(jogBlockedReason);
  const why = whyKey ? t(whyKey) : undefined;
  const connected = useStore((s) => s.connection.connected);
  const step = useStore((s) => s.jogStep);
  const speed = useStore((s) => s.jogSpeed);
  const maxSpeed = useStore((s) => Math.min(...selectActiveDevice(s).max_rate));
  const userOrigin = useStore((s) => s.jobOptions.user_origin);
  const setStep = useStore((s) => s.setJogStep);
  const setSpeed = useStore((s) => s.setJogSpeed);
  const goToOrigin = useStore((s) => s.goToOrigin);
  const setHere = useStore((s) => s.setUserOriginHere);
  const clearOrigin = useStore((s) => s.clearUserOrigin);
  const home = useCommand('machine.home');
  const holdProps = useHoldJog();
  const disabled = whyKey !== null;

  return (
    <section className="section" aria-labelledby="sec-jog">
      <h3 id="sec-jog" className="panel-title">{t('machine.jog')}</h3>
      <div className="jog">
        <div className="mc-pad" role="group" aria-label={t('machine.jog')} aria-describedby="jog-help" onKeyDown={onPadKeyDown}>
          {GRID.map((cell) => {
            if (cell === 'center') {
              const label = t(userOrigin ? 'machine.jog.toUserOrigin' : 'machine.jog.toZero');
              return (
                <button key={cell} type="button" className="icon-btn icon-btn--boxed mc-pad__btn mc-pad__center" disabled={disabled} title={why ?? label} aria-label={label} onClick={() => void goToOrigin()}>
                  <Crosshair size={15} aria-hidden="true" />
                </button>
              );
            }
            const Icon = ICONS[cell];
            return (
              <button key={cell} type="button" className="icon-btn icon-btn--boxed mc-pad__btn" data-dir={cell} disabled={disabled} title={why ?? t(DIR_LABEL[cell])} aria-label={t(DIR_LABEL[cell])} {...holdProps(JOG_DIRS[cell])}>
                <Icon size={16} aria-hidden="true" />
              </button>
            );
          })}
        </div>
        <div className="mc-jog-side">
          <div className="jog__steps" role="radiogroup" aria-label={t('machine.jog.step')}>
            {JOG_STEPS.map((s) => (
              <button key={s} type="button" role="radio" aria-checked={step === s} className="chip num" onClick={() => setStep(s)}>
                {fmt(s, s < 1 ? 1 : 0)} mm
              </button>
            ))}
          </div>
          <NumberField label={t('machine.jog.speed')} unit="mm/min" value={speed} min={1} max={maxSpeed} step={100} onCommit={setSpeed} />
        </div>
      </div>
      <p id="jog-help" className="hint">{t('machine.jog.help')}</p>
      <div className="row">
        <button type="button" className="btn grow" disabled={!home.enabled} title={home.tooltip} onClick={home.run}>
          <House size={14} aria-hidden="true" />{home.label}
        </button>
        <button type="button" className="btn grow" disabled={!connected} title={connected ? t('machine.origin.setHint') : t('reason.needMachine')} onClick={setHere}>
          <MapPin size={14} aria-hidden="true" />{t('machine.origin.set')}
        </button>
      </div>
      {userOrigin && (
        <p className="hint num mc-origin-line">
          <span>{t('machine.origin.current', { x: fmt(userOrigin[0], 2), y: fmt(userOrigin[1], 2) })}</span>
          <button type="button" className="icon-btn" title={t('machine.origin.clear')} aria-label={t('machine.origin.clear')} onClick={clearOrigin}>
            <X size={13} aria-hidden="true" />
          </button>
        </p>
      )}
    </section>
  );
}
