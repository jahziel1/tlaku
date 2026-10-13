import { Check, MousePointerClick, RotateCcw } from 'lucide-react';
import { useEffect, useId, useState } from 'react';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { NumberField } from '@/ui/NumberField';
import { JOG_STEPS } from './jogMath';
import { FINISH_MODES, MAX_JOG_STEPS, parseJogSteps, type FinishMode } from './moveSettings';

/** Texto de la lista de pasos: «0,1 · 1 · 10 · 50» como se escribe, sin ceros sobrantes. */
const stepsText = (steps: readonly number[]) => steps.map((s) => String(s)).join('; ');

/** Edita los pasos del jog (hasta seis, de 0,01 a 500 mm). Confirma con Enter o al salir. */
export function JogStepsEditor() {
  const id = useId();
  const steps = useStore((s) => s.jogSteps);
  const setSteps = useStore((s) => s.setJogSteps);
  const [text, setText] = useState(stepsText(steps));
  const [bad, setBad] = useState(false);
  useEffect(() => {
    setText(stepsText(steps));
    setBad(false);
  }, [steps]);
  const isDefault = steps.length === JOG_STEPS.length && steps.every((s, i) => s === JOG_STEPS[i]);

  const commit = () => {
    const parsed = parseJogSteps(text);
    if (!parsed) return setBad(true);
    setBad(false);
    setSteps(parsed);
  };

  return (
    <div className="field mc-steps">
      <label htmlFor={id} className="field__label">{t('machine.move.steps')}</label>
      <div className="row">
        <div className="field__box grow">
          <input
            id={id}
            className="field__input num"
            value={text}
            aria-invalid={bad}
            aria-describedby={`${id}-help`}
            onChange={(e) => setText(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === 'Enter') commit();
              else if (e.key === 'Escape') {
                setText(stepsText(steps));
                setBad(false);
              }
            }}
          />
          <span className="field__unit">mm</span>
        </div>
        <button type="button" className="icon-btn icon-btn--boxed" disabled={isDefault} title={t('machine.move.steps.reset')} aria-label={t('machine.move.steps.reset')} onClick={() => setSteps([...JOG_STEPS])}>
          <RotateCcw size={13} aria-hidden="true" />
        </button>
      </div>
      <p id={`${id}-help`} className={bad ? 'hint hint--danger' : 'hint'}>{bad ? t('machine.move.steps.bad', { max: MAX_JOG_STEPS }) : t('machine.move.steps.help', { max: MAX_JOG_STEPS })}</p>
    </div>
  );
}

const FINISH_LABEL = {
  stay: 'machine.finish.stay',
  origin: 'machine.finish.origin',
  point: 'machine.finish.point',
} as const satisfies Record<FinishMode, string>;

/** "Posición final": qué hace el cabezal al terminar un trabajo (quedarse, volver al origen o ir a un punto). */
export function FinishPositionControls() {
  const finish = useStore((s) => s.finish);
  const set = useStore((s) => s.setFinishPosition);
  const userOrigin = useStore((s) => s.jobOptions.user_origin);
  const device = useStore(selectActiveDevice);
  const goNow = useStore((s) => s.goToFinishPosition);
  const connected = useStore((s) => s.connection.connected);

  return (
    <div className="mc-finish">
      <h4 className="mc-sub">{t('machine.finish')}</h4>
      <div className="mc-modes mc-modes--row" role="radiogroup" aria-label={t('machine.finish')}>
        {FINISH_MODES.map((m) => (
          <button key={m} type="button" role="radio" aria-checked={finish.mode === m} className="chip" onClick={() => set({ mode: m })}>
            {t(FINISH_LABEL[m])}
          </button>
        ))}
      </div>
      {finish.mode === 'point' && (
        <div className="field-grid">
          <NumberField label="X" unit="mm" value={finish.x} min={0} max={device.bed[0]} step={1} onCommit={(x) => set({ x })} />
          <NumberField label="Y" unit="mm" value={finish.y} min={0} max={device.bed[1]} step={1} onCommit={(y) => set({ y })} />
        </div>
      )}
      <p className="hint num">
        {finish.mode === 'stay' && t('machine.finish.stay.help')}
        {finish.mode === 'origin' && (userOrigin ? t('machine.finish.origin.user', { x: fmt(userOrigin[0], 2), y: fmt(userOrigin[1], 2) }) : t('machine.finish.origin.zero'))}
        {finish.mode === 'point' && t('machine.finish.point.help', { x: fmt(finish.x, 1), y: fmt(finish.y, 1) })}
      </p>
      {finish.mode === 'stay' && device.return_to_origin && <p className="hint">{t('machine.finish.profileReturns')}</p>}
      {finish.mode !== 'stay' && (
        <button type="button" className="btn" disabled={!connected} title={connected ? t('machine.finish.goNow.help') : t('reason.needMachine')} onClick={() => void goNow()}>
          <Check size={14} aria-hidden="true" />{t('machine.finish.goNow')}
        </button>
      )}
    </div>
  );
}

/** Botón del modo "Posicionar el láser": el siguiente clic en la cama manda allí la cabeza, con el láser apagado. */
export function PositionLaserButton() {
  const cmd = useCommand('machine.positionLaser');
  const on = useStore((s) => s.positioningLaser);
  return (
    <>
      <button type="button" className={on ? 'btn btn--accent' : 'btn'} aria-pressed={on} disabled={!cmd.enabled && !on} title={cmd.tooltip} onClick={cmd.run}>
        <MousePointerClick size={14} aria-hidden="true" />{cmd.label}
      </button>
      {on && <p className="hint" role="status">{t('machine.position.active')}</p>}
    </>
  );
}
