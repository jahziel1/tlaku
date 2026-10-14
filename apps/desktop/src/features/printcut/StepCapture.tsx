import {
  ArrowDown, ArrowDownLeft, ArrowDownRight, ArrowLeft, ArrowRight, ArrowUp, ArrowUpLeft, ArrowUpRight, Camera, Check, Crosshair, MapPin, MousePointerClick, Save, TriangleAlert, type LucideIcon,
} from 'lucide-react';
import { useCallback } from 'react';
import { getApiKind } from '@/api';
import { mockSheetPoint } from '@/api/mock/printcut';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { jogBlockedReason, positionLaserBlockedReason } from '@/store/machine';
import { JOG_DIRS, type JogDirName } from '@/features/machine/jogMath';
import { PositionLaserButton } from '@/features/machine/MoveExtras';
import { onPadKeyDown, useHoldJog } from '@/features/machine/useHoldJog';
import { goToCurrentMark, laserBedPosition, savePickedPoint, saveLaserPosition } from './actions';
import { useCanvasPick } from './canvasPick';
import { usePrintCut, type CaptureMethod } from './printcutStore';

const ICONS: Record<JogDirName, LucideIcon> = {
  upLeft: ArrowUpLeft, up: ArrowUp, upRight: ArrowUpRight, left: ArrowLeft, right: ArrowRight, downLeft: ArrowDownLeft, down: ArrowDown, downRight: ArrowDownRight,
};
const GRID: (JogDirName | 'center')[] = ['upLeft', 'up', 'upRight', 'left', 'center', 'right', 'downLeft', 'down', 'downRight'];
const DIR_LABEL: Record<JogDirName, I18nKey> = {
  upLeft: 'machine.jog.upLeft', up: 'machine.jog.up', upRight: 'machine.jog.upRight', left: 'machine.jog.left', right: 'machine.jog.right',
  downLeft: 'machine.jog.downLeft', down: 'machine.jog.down', downRight: 'machine.jog.downRight',
};

/** Pad de jog compacto, con los mismos pasos y velocidad que la pestaña Máquina. */
function JogPad() {
  const whyKey = useStore(jogBlockedReason);
  const why = whyKey ? t(whyKey) : undefined;
  const step = useStore((s) => s.jogStep);
  const steps = useStore((s) => s.jogSteps);
  const setStep = useStore((s) => s.setJogStep);
  const holdProps = useHoldJog();
  return (
    <div className="pc-jog">
      <div className="mc-pad" role="group" aria-label={t('printcut.capture.pad')} onKeyDown={onPadKeyDown}>
        {GRID.map((cell) => {
          if (cell === 'center') return <span key={cell} aria-hidden="true" />;
          const Icon = ICONS[cell];
          return (
            <button key={cell} type="button" className="icon-btn icon-btn--boxed mc-pad__btn" data-dir={cell} disabled={whyKey !== null} title={why ?? t(DIR_LABEL[cell])} aria-label={t(DIR_LABEL[cell])} {...holdProps(JOG_DIRS[cell])}>
              <Icon size={16} aria-hidden="true" />
            </button>
          );
        })}
      </div>
      <div className="jog__steps" role="radiogroup" aria-label={t('machine.jog.step')}>
        {steps.map((s) => (
          <button key={s} type="button" role="radio" aria-checked={step === s} className="chip num" onClick={() => setStep(s)}>
            {fmt(s, s < 1 ? (s < 0.1 ? 2 : 1) : Number.isInteger(s) ? 0 : 1)} mm
          </button>
        ))}
      </div>
    </div>
  );
}

/** Medir con el láser: mover el cabezal hasta la marca impresa y guardar dónde quedó. */
function LaserCapture() {
  const current = usePrintCut((s) => s.current);
  const mark = usePrintCut((s) => s.marks[current]);
  const connection = useStore((s) => s.connection);
  const jobState = useStore((s) => s.jobState);
  const status = useStore((s) => s.status);
  const devices = useStore((s) => s.devices);
  const activeDeviceId = useStore((s) => s.activeDeviceId);
  const position = laserBedPosition({ connection, jobState, status, devices, activeDeviceId });
  const goBlocked = useStore(positionLaserBlockedReason);
  const mock = getApiKind() === 'mock';
  const device = useStore(selectActiveDevice);
  const goTo = useStore((s) => s.goToDocPoint);

  // Solo en el modo demostración: lleva el cabezal a donde cayó la marca en la hoja simulada.
  const goToSheet = () => {
    if (!mark) return;
    const [x, y] = mockSheetPoint(mark.point);
    void goTo(x, y);
  };

  return (
    <>
      <p className="hint">{t('printcut.capture.absolute')}</p>
      <div className="pc-row">
        <button type="button" className="btn" disabled={goBlocked !== null} title={goBlocked ? t(goBlocked) : t('printcut.capture.goto.hint')} onClick={() => void goToCurrentMark()}>
          <MapPin size={14} aria-hidden="true" />{t('printcut.capture.goto')}
        </button>
        <PositionLaserButton />
      </div>
      <JogPad />
      {mock && (
        <div className="pc-mock">
          <p className="hint">{t('printcut.mock.help')}</p>
          <button type="button" className="btn" disabled={goBlocked !== null} title={goBlocked ? t(goBlocked) : undefined} onClick={goToSheet}>
            <Crosshair size={14} aria-hidden="true" />{t('printcut.mock.goto')}
          </button>
        </div>
      )}
      <p className="pc-position num" role="status">
        {'point' in position
          ? t('printcut.capture.here', { x: fmt(position.point[0], 2), y: fmt(position.point[1], 2) })
          : t(position.reason)}
      </p>
      <p className="warn-line pc-pointer">
        <TriangleAlert size={13} aria-hidden="true" />
        <span>{t('printcut.capture.pointer', { name: device.name })}</span>
      </p>
      <button
        type="button"
        className="btn btn--accent"
        aria-disabled={!('point' in position)}
        title={'reason' in position ? t(position.reason) : undefined}
        onClick={() => saveLaserPosition()}
      >
        <Save size={14} aria-hidden="true" />{t('printcut.capture.save')}
      </button>
    </>
  );
}

/** Medir con la cámara: un clic sobre la marca en la imagen alineada (dibujada en el lienzo). */
function CameraCapture() {
  const picking = usePrintCut((s) => s.picking);
  const patch = usePrintCut((s) => s.patch);
  const stop = useCallback(() => usePrintCut.setState({ picking: false }), []);
  useCanvasPick(picking, savePickedPoint, stop);
  const mock = getApiKind() === 'mock';
  const current = usePrintCut((s) => s.current);
  const mark = usePrintCut((s) => s.marks[current]);

  return (
    <>
      <p className="hint">{t('printcut.capture.camera.help')}</p>
      <button type="button" className={picking ? 'btn btn--accent' : 'btn'} aria-pressed={picking} onClick={() => patch({ picking: !picking })}>
        <MousePointerClick size={14} aria-hidden="true" />{picking ? t('printcut.capture.camera.picking') : t('printcut.capture.camera.pick')}
      </button>
      {mock && mark && (
        <div className="pc-mock">
          <p className="hint">{t('printcut.mock.cameraHelp')}</p>
          <button type="button" className="btn" onClick={() => savePickedPoint(mockSheetPoint(mark.point))}>
            <Crosshair size={14} aria-hidden="true" />{t('printcut.mock.pick')}
          </button>
        </div>
      )}
    </>
  );
}

/** Paso 2: medir en la cama dónde quedó cada marca impresa. */
export function StepCapture() {
  const marks = usePrintCut((s) => s.marks);
  const physical = usePrintCut((s) => s.physical);
  const current = usePrintCut((s) => s.current);
  const method = usePrintCut((s) => s.method);
  const patch = usePrintCut((s) => s.patch);
  const cameraReady = useStore((s) => s.cameraBackdrop !== null);
  const mark = marks[current];

  const setMethod = (m: CaptureMethod) => patch({ method: m, picking: false });
  const done = physical.filter((p) => p !== null).length;

  return (
    <>
      <p className="hint">{t('printcut.capture.intro')}</p>
      <div className="pc-row" role="radiogroup" aria-label={t('printcut.capture.method')}>
        <button type="button" role="radio" aria-checked={method === 'laser'} className="chip" onClick={() => setMethod('laser')}>
          <Crosshair size={13} aria-hidden="true" />{t('printcut.capture.laser')}
        </button>
        <button
          type="button"
          role="radio"
          aria-checked={method === 'camera'}
          aria-disabled={!cameraReady}
          className="chip"
          title={cameraReady ? undefined : t('printcut.capture.camera.reason')}
          onClick={() => cameraReady && setMethod('camera')}
        >
          <Camera size={13} aria-hidden="true" />{t('printcut.capture.camera')}
        </button>
      </div>

      <ol className="pc-marks" aria-label={t('printcut.marks.list')}>
        {marks.map((m, i) => {
          const p = physical[i];
          return (
            <li key={m.ids.join(',')}>
              <button type="button" className="pc-mark" aria-current={i === current} onClick={() => patch({ current: i, picking: false })}>
                {p ? <Check size={13} aria-hidden="true" /> : <span className="pc-mark__dot" aria-hidden="true" />}
                <span>{t('printcut.capture.markN', { n: i + 1 })}</span>
                <span className="num pc-mark__pos">{p ? t('printcut.capture.saved', { x: fmt(p[0], 2), y: fmt(p[1], 2) }) : t('printcut.capture.pending')}</span>
              </button>
            </li>
          );
        })}
      </ol>

      {mark && (
        <p className="pc-current" role="status">
          {t('printcut.capture.progress', { n: current + 1, total: marks.length, done })}
          <span className="num"> · {t('printcut.capture.designAt', { x: fmt(mark.point[0], 2), y: fmt(mark.point[1], 2) })}</span>
        </p>
      )}

      {method === 'camera' && cameraReady ? <CameraCapture /> : <LaserCapture />}
    </>
  );
}
