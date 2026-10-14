import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, LocateFixed, TriangleAlert } from 'lucide-react';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { positionLaserBlockedReason } from '@/store/machine';
import { selectActiveDevice, useStore } from '@/store';
import { useCenter } from '@/features/camera/centerFinder';
import { NumberField } from '@/ui/NumberField';
import { Modal } from './Modal';
import {
  PROBE_LIMITS,
  cancelTouch,
  confirmTouch,
  moveHeadToCenter,
  probeBlockedReason,
  probeParamsProblem,
  recordPointerPoint,
  removeLastPoint,
  requestTouch,
  restart,
  setOriginAtCenter,
  touchDelta,
  useProbeCenter,
  type ProbeDirName,
} from './probeCenter';

const ARROWS = { up: ArrowUp, left: ArrowLeft, right: ArrowRight, down: ArrowDown } as const;
const DIR_LABEL = {
  up: 'machine.probe.dir.up',
  left: 'machine.probe.dir.left',
  right: 'machine.probe.dir.right',
  down: 'machine.probe.dir.down',
} as const satisfies Record<ProbeDirName, I18nKey>;

/** La línea de G-code que se enviará, para mostrarla antes de mover (misma forma que `worker/probe.rs`). */
export function probeLine(dx: number, dy: number, feed: number): string {
  const n = (v: number) => String(Math.round(v * 1000) / 1000 + 0);
  return `G38.3 X${n(dx)} Y${n(dy)} F${n(feed)}`;
}

/**
 * Buscador de centro con el cabezal (M48): con la sonda toca tres puntos del borde de un objeto
 * redondo; sin ella, la persona lleva el puntero a tres puntos y confirma cada uno.
 */
export function ProbeCenterDialog() {
  const open = useProbeCenter((s) => s.open);
  return open ? <Body /> : null;
}

function Body() {
  const { mode, feed, travel, retract, points, confirm, busy, error, missed, setMode, setParam, close } = useProbeCenter();
  const blocked = useStore((s) => probeBlockedReason(s));
  const moveBlocked = useStore((s) => positionLaserBlockedReason(s));
  const device = useStore((s) => selectActiveDevice(s));
  const result = useCenter((s) => s.result);
  const problem = probeParamsProblem({ feed, travel, retract });
  const probe = mode === 'probe';

  const pending = confirm ? touchDelta(device, confirm, travel) : null;

  return (
    <Modal
      title={t('machine.probe.title')}
      subtitle={t('machine.probe.subtitle')}
      tone="warn"
      onClose={close}
      footer={
        <button type="button" className="btn" data-autofocus onClick={close}>
          {t('machine.dlg.close')}
        </button>
      }
    >
      <div className="field">
        <label className="field__label" htmlFor="probe-mode">{t('machine.probe.mode')}</label>
        <select id="probe-mode" className="select" value={mode} disabled={busy} onChange={(e) => setMode(e.target.value as 'probe' | 'pointer')}>
          <option value="probe">{t('machine.probe.mode.probe')}</option>
          <option value="pointer">{t('machine.probe.mode.pointer')}</option>
        </select>
      </div>

      {probe ? (
        <>
          <p className="hint">{t('machine.probe.help.probe')}</p>
          <div className="mc-probe__params">
            <NumberField label={t('machine.probe.feed')} unit="mm/min" value={feed} min={1} max={PROBE_LIMITS.feedMax} step={10} disabled={busy} onCommit={(v) => setParam({ feed: v })} />
            <NumberField label={t('machine.probe.travel')} unit="mm" value={travel} min={0.5} max={PROBE_LIMITS.travelMax} step={5} disabled={busy} onCommit={(v) => setParam({ travel: v })} />
            <NumberField label={t('machine.probe.retract')} unit="mm" value={retract} min={0} max={PROBE_LIMITS.retractMax} step={1} disabled={busy} onCommit={(v) => setParam({ retract: v })} />
          </div>
          {problem && <p className="warn-line" role="alert">{t(problem)}</p>}
          {blocked && <p className="warn-line" role="status"><TriangleAlert size={14} aria-hidden="true" /><span>{t(blocked)}</span></p>}
          <p className="hint">{t('machine.probe.pointsLeft', { n: Math.max(0, 3 - points.length) })}</p>
          <div className="mc-probe__dirs" role="group" aria-label={t('machine.probe.dirs')}>
            {(['up', 'left', 'right', 'down'] as const).map((d) => {
              const Icon = ARROWS[d];
              const why = blocked ?? (problem ?? null);
              return (
                <button key={d} type="button" className="btn" data-dir={d} aria-disabled={!!why || busy} title={why ? t(why) : t('machine.probe.dirHint', { dir: t(DIR_LABEL[d]) })} onClick={why || busy ? undefined : () => requestTouch(d)}>
                  <Icon size={14} aria-hidden="true" />
                  {t(DIR_LABEL[d])}
                </button>
              );
            })}
          </div>
          {confirm && pending && (
            <div className="mc-probe__confirm" role="alertdialog" aria-label={t('machine.probe.confirm.title')}>
              <p><strong>{t('machine.probe.confirm.title')}</strong></p>
              <p>{t('machine.probe.confirm.text', { travel: fmt(travel, 1), feed: fmt(feed, 0), dir: t(DIR_LABEL[confirm]) })}</p>
              <ul className="mc-list">
                {(['machine.probe.confirm.check1', 'machine.probe.confirm.check2', 'machine.probe.confirm.check3'] as const).map((k) => (
                  <li key={k}>{t(k)}</li>
                ))}
              </ul>
              <p>{t('machine.confirm.exact')}</p>
              <pre className="mc-code num">{`M5\nG91\n${probeLine(pending[0], pending[1], feed)}\nG90`}</pre>
              <div className="row">
                <button type="button" className="btn" onClick={cancelTouch}>{t('machine.dlg.cancel')}</button>
                <button type="button" className="btn btn--danger" data-autofocus onClick={() => void confirmTouch()}>{t('machine.probe.confirm.go')}</button>
              </div>
            </div>
          )}
          {busy && <p className="hint" role="status">{t('machine.probe.touching')}</p>}
          {missed && <p className="warn-line" role="status"><TriangleAlert size={14} aria-hidden="true" /><span>{t('machine.probe.missed', { travel: fmt(travel, 1) })}</span></p>}
        </>
      ) : (
        <>
          <p className="hint">{t('machine.probe.help.pointer')}</p>
          <p className="hint">{t('machine.probe.pointsLeft', { n: Math.max(0, 3 - points.length) })}</p>
          <button type="button" className="btn btn--accent" onClick={() => recordPointerPoint()}>
            <LocateFixed size={14} aria-hidden="true" />
            {t('machine.probe.record')}
          </button>
        </>
      )}

      {points.length > 0 && (
        <ol className="mc-probe__points num" aria-label={t('machine.probe.points')}>
          {points.map((p, i) => (
            <li key={i}>{t('machine.probe.point', { n: i + 1, x: fmt(p.doc[0], 3), y: fmt(p.doc[1], 3) })}</li>
          ))}
        </ol>
      )}
      {error && <p className="warn-line" role="alert">{error}</p>}

      {result && (result.source === 'probe' || result.source === 'pointer') && (
        <div role="status" aria-live="polite">
          <p className="hint cam-ok num">
            {t('camera.center.result', { x: fmt(result.circle.center[0], 2), y: fmt(result.circle.center[1], 2), r: fmt(result.circle.radius, 2), d: fmt(result.circle.radius * 2, 2) })}
          </p>
          <div className="row">
            <button type="button" className="btn btn--accent" aria-disabled={!!moveBlocked} title={moveBlocked ? t(moveBlocked) : t('camera.center.moveHint')} onClick={moveBlocked ? undefined : () => void moveHeadToCenter()}>
              <LocateFixed size={14} aria-hidden="true" />
              {t('camera.center.move')}
            </button>
            <button type="button" className="btn" onClick={() => setOriginAtCenter()}>{t('camera.center.setOrigin')}</button>
          </div>
        </div>
      )}
      <div className="row">
        <button type="button" className="btn" disabled={points.length === 0 || busy} onClick={removeLastPoint}>{t('machine.probe.undo')}</button>
        <button type="button" className="btn" disabled={points.length === 0 || busy} onClick={restart}>{t('machine.probe.restart')}</button>
      </div>
      <p className="hint hint--warn">{t('machine.probe.unvalidated')}</p>
    </Modal>
  );
}
