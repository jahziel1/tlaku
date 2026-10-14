import { Crosshair, MoveRight, Square, Trash2 } from 'lucide-react';
import { useMemo } from 'react';
import { useCommand } from '@/commands/useCommand';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { formatSize } from '@/lib/units';
import { selectActiveDevice, usePref, useStore } from '@/store';
import { NumberField } from '@/ui/NumberField';
import { clearHeadCalibration, setHeadEnabled } from './actions';
import { runAction } from './bedImage';
import { HeadMarksBlock } from './HeadMarksBlock';
import {
  currentFit,
  defaultMark,
  moveToNextObservation,
  openHeadMarkAsDocument,
  saveHeadCalibration,
  startMeasuring,
} from './headCal';
import { estimateScanSeconds, headPxPerMm, planScan } from './head';
import { clearMosaic, scanRegion } from './headScan';
import { HEAD_CAL_MIN_OBS, HEAD_CAL_STEP_MM, useHeadCal, useHeadUi, useViewTool, type HeadAction } from './headState';
import { zoneSize } from './zone';
import './camera.css';

/** Texto del aviso "por validar con hardware real" que llevan las funciones sin probar en máquina. */
function Unvalidated({ k }: { k: I18nKey }) {
  return <p className="hint hint--warn">{t(k)}</p>;
}

/** Confirmación (la cabeza se va a mover) y avance del recorrido. Se pinta una sola vez en el panel. */
export function HeadConfirm() {
  const confirm = useHeadUi((s) => s.confirm);
  const progress = useHeadUi((s) => s.progress);
  const error = useHeadUi((s) => s.error);
  const useSelection = useHeadUi((s) => s.useSelection);
  const zone = useHeadUi((s) => s.zone);
  const useZone = useHeadUi((s) => s.useZone);
  const ask = useHeadUi((s) => s.ask);
  const hasSelection = useStore((s) => s.selectedIds.length > 0);
  const cal = useStore((s) => s.cameraSetup.head?.calibration ?? null);
  const device = useStore(selectActiveDevice);
  const jogSpeed = useStore((s) => s.jogSpeed);

  const plan = useMemo(() => {
    if (!cal || !confirm) return null;
    return planScan(device.bed, cal, scanRegion(useSelection && hasSelection, useZone ? zone : null));
  }, [cal, confirm, device.bed, useSelection, hasSelection, useZone, zone]);

  if (progress) {
    return (
      <section className="section cam-confirm" aria-label={t('camera.head.scan.title')} aria-live="polite">
        <h3 className="panel-title">{t('camera.head.scan.title')}</h3>
        <p className="hint num" role="status">{t('camera.head.scan.progress', { step: progress.step, total: progress.total })}</p>
        <div className="cam-meter" role="meter" aria-valuemin={0} aria-valuemax={progress.total} aria-valuenow={progress.step} aria-label={t('camera.head.scan.progress', { step: progress.step, total: progress.total })}>
          <div className="cam-meter__bar" style={{ width: `${Math.round((progress.step / progress.total) * 100)}%` }} />
        </div>
        <button type="button" className="btn" onClick={() => useHeadUi.getState().requestCancel(true)}>{t('camera.head.scan.cancel')}</button>
      </section>
    );
  }
  if (!confirm) {
    return error ? <p className="warn-line" role="alert">{error}</p> : null;
  }
  const secs = plan && !('error' in plan) ? estimateScanSeconds(plan, jogSpeed, 500, null) : 0;
  return (
    <section className="section cam-confirm" aria-label={t('camera.head.confirm.title')}>
      <h3 className="panel-title">{t('camera.head.confirm.title')}</h3>
      <p className="warn-line" role="alert">{t('camera.head.confirm.warning')}</p>
      {plan && 'error' in plan && <p className="hint hint--danger" role="alert">{t(`camera.head.plan.${plan.error}` as I18nKey)}</p>}
      {plan && !('error' in plan) && (
        <>
          <p className="hint num">{t('camera.head.confirm.summary', { n: plan.stops.length, cols: plan.cols, rows: plan.rows, secs: Math.max(1, Math.round(secs)) })}</p>
          {plan.partial && <p className="hint hint--warn">{t('camera.head.scan.partial')}</p>}
        </>
      )}
      <p className="hint">{t(`camera.head.confirm.area.${useZone && zone ? 'zone' : useSelection && hasSelection ? 'selection' : 'bed'}` as I18nKey)}</p>
      <div className="row cam-wrap">
        <button type="button" className="btn btn--accent grow" disabled={!plan || 'error' in plan} onClick={() => void runAction(confirm as HeadAction)}>
          {t('camera.head.confirm.go')}
        </button>
        <button type="button" className="btn" onClick={() => ask(null)}>{t('camera.head.confirm.cancel')}</button>
      </div>
    </section>
  );
}

/** Cámara montada en el cabezal (M36): modo, calibración del desfase y recorrido de la cama. */
export function HeadSection() {
  const head = useStore((s) => s.cameraSetup.head);
  const enabled = head?.enabled === true;
  const cal = head?.calibration ?? null;
  const lens = useStore((s) => s.cameraSetup.calibration);
  const overlay = useCommand('camera.updateOverlay');
  const mosaic = useHeadUi((s) => s.mosaic);
  const running = useHeadUi((s) => s.progress !== null);

  return (
    <section className="section" aria-label={t('camera.head.title')}>
      <h3 className="panel-title">{t('camera.head.title')}</h3>
      <p className="hint">{t('camera.head.help')}</p>
      <label className="mc-check">
        <input type="checkbox" checked={enabled} onChange={(e) => setHeadEnabled(e.target.checked)} />
        {t('camera.head.enable')}
      </label>
      {enabled && !cal && <p className="warn-line" role="alert">{t('camera.head.needCalibration')}</p>}
      {enabled && !lens && <p className="hint">{t('camera.head.lensHint')}</p>}
      {enabled && (
        <>
          <HeadCalibrationBlock />
          <h4 className="cam-subtitle">{t('camera.head.scan.title')}</h4>
          <p className="hint">{t('camera.head.scan.help')}</p>
          <ScanAreaControls />
          <button type="button" className="btn btn--accent" aria-disabled={!overlay.enabled || running} title={overlay.enabled ? overlay.tooltip : (overlay.reason ?? undefined)} onClick={overlay.enabled && !running ? overlay.run : undefined}>
            <MoveRight size={14} aria-hidden="true" />
            {t('camera.head.scan.update')}
          </button>
          {!overlay.enabled && overlay.reason && <p className="hint">{overlay.reason}</p>}
          {mosaic && (
            <>
              <p className="hint cam-ok num" role="status">{t('camera.head.scan.mosaic', { n: mosaic.tiles, pct: Math.round(mosaic.coverage * 100) })}</p>
              <button type="button" className="btn" onClick={clearMosaic}>{t('camera.head.scan.clear')}</button>
            </>
          )}
          {!mosaic && cal && <p className="hint">{t('camera.head.scan.live')}</p>}
        </>
      )}
      <Unvalidated k="camera.head.unvalidated" />
    </section>
  );
}

/** Qué parte de la cama recorrer: toda, la selección o un rectángulo dibujado en el lienzo. */
export function ScanAreaControls() {
  const useSelection = useHeadUi((s) => s.useSelection);
  const useZone = useHeadUi((s) => s.useZone);
  const zone = useHeadUi((s) => s.zone);
  const drawing = useHeadUi((s) => s.drawingZone);
  const setUseSelection = useHeadUi((s) => s.setUseSelection);
  const setUseZone = useHeadUi((s) => s.setUseZone);
  const setZone = useHeadUi((s) => s.setZone);
  const setDrawing = useHeadUi((s) => s.setDrawingZone);
  const hasSelection = useStore((s) => s.selectedIds.length > 0);
  const units = usePref('units');
  const value = useZone && zone ? 'zone' : useSelection && hasSelection ? 'selection' : 'bed';
  const choose = (v: 'bed' | 'selection' | 'zone') => {
    setUseSelection(v === 'selection');
    setUseZone(v === 'zone');
  };
  const size = zone ? zoneSize(zone) : null;

  return (
    <fieldset className="cam-area" aria-label={t('camera.head.area.title')}>
      <legend className="field__label">{t('camera.head.area.title')}</legend>
      <label className="mc-check">
        <input type="radio" name="cam-scan-area" checked={value === 'bed'} onChange={() => choose('bed')} />
        {t('camera.head.area.bed')}
      </label>
      <label className="mc-check">
        <input type="radio" name="cam-scan-area" checked={value === 'selection'} disabled={!hasSelection} onChange={() => choose('selection')} />
        {t('camera.head.area.selection')}
      </label>
      <label className="mc-check">
        <input type="radio" name="cam-scan-area" checked={value === 'zone'} disabled={!zone} onChange={() => choose('zone')} />
        {size ? t('camera.head.area.zoneSized', { size: formatSize(size[0], size[1], units, 1) }) : t('camera.head.area.zone')}
      </label>
      <div className="row cam-wrap">
        <button type="button" className="btn" aria-pressed={drawing} onClick={() => setDrawing(!drawing)}>
          <Square size={14} aria-hidden="true" />
          {drawing ? t('camera.head.area.drawing') : zone ? t('camera.head.area.redraw') : t('camera.head.area.draw')}
        </button>
        {zone && (
          <button type="button" className="btn" onClick={() => setZone(null)}>
            <Trash2 size={14} aria-hidden="true" />
            {t('camera.head.area.clear')}
          </button>
        )}
      </div>
      <p className="hint">{t('camera.head.area.help')}</p>
    </fieldset>
  );
}

function HeadCalibrationBlock() {
  const cal = useStore((s) => s.cameraSetup.head?.calibration ?? null);
  const mark = useHeadCal((s) => s.mark);
  const measuring = useHeadCal((s) => s.measuring);
  const obs = useHeadCal((s) => s.obs);
  const setMark = useHeadCal((s) => s.setMark);
  const setTool = useViewTool((s) => s.setTool);
  const connected = useStore((s) => s.connection.connected);
  const live = useStore((s) => s.cameraStatus === 'live');
  const bed = useStore((s) => selectActiveDevice(s).bed);
  const shown = mark ?? defaultMark();

  // Depende de las observaciones y de la marca (ambas leídas arriba): se recalcula en cada cambio.
  const fit = measuring ? currentFit() : null;
  const result = fit && 'calibration' in fit ? fit.calibration : null;

  const measure = () => {
    startMeasuring();
    setTool('headMark');
  };

  return (
    <div className="cam-subsection" role="group" aria-label={t('camera.head.cal.title')}>
      <h4 className="cam-subtitle">{t('camera.head.cal.title')}</h4>
      <p className="hint">{t('camera.head.cal.help')}</p>
      {cal ? (
        <p className="hint cam-ok num" role="status">
          {t('camera.head.cal.done', { x: fmt(cal.offset_mm[0], 2), y: fmt(cal.offset_mm[1], 2), scale: fmt(headPxPerMm(cal), 2), rms: fmt(cal.rms_mm, 2) })}
        </p>
      ) : (
        <p className="hint">{t('camera.head.cal.none')}</p>
      )}
      <ol className="cam-steps">
        <li>{t('camera.head.cal.step1')}</li>
        <li>{t('camera.head.cal.step2', { d: HEAD_CAL_STEP_MM })}</li>
        <li>{t('camera.head.cal.step3', { min: HEAD_CAL_MIN_OBS })}</li>
      </ol>
      <div className="field-grid">
        <NumberField label={t('camera.head.cal.markX')} unit="mm" value={shown[0]} min={0} max={bed[0]} step={1} onCommit={(v) => setMark([v, shown[1]])} />
        <NumberField label={t('camera.head.cal.markY')} unit="mm" value={shown[1]} min={0} max={bed[1]} step={1} onCommit={(v) => setMark([shown[0], v])} />
      </div>
      <div className="row cam-wrap">
        <button type="button" className="btn" onClick={() => void openHeadMarkAsDocument()}>{t('camera.head.cal.generate')}</button>
        <button type="button" className="btn btn--accent" disabled={!live || !connected} title={!live ? t('camera.reason.notLive') : !connected ? t('reason.needMachine') : undefined} onClick={measure}>
          <Crosshair size={14} aria-hidden="true" />
          {measuring ? t('camera.head.cal.restart') : t('camera.head.cal.measure')}
        </button>
      </div>
      {measuring && (
        <div role="status" aria-live="polite">
          <p className="hint num">{t('camera.head.cal.points', { n: obs.length, min: HEAD_CAL_MIN_OBS })}</p>
          <div className="row cam-wrap">
            <button type="button" className="btn" disabled={obs.length === 0} onClick={() => void moveToNextObservation()}>
              {t('camera.head.cal.next', { d: HEAD_CAL_STEP_MM })}
            </button>
            <button type="button" className="btn" onClick={() => useHeadCal.getState().reset()}>{t('camera.head.cal.cancel')}</button>
          </div>
          {fit && 'error' in fit && obs.length >= HEAD_CAL_MIN_OBS && <p className="warn-line" role="alert">{t(`camera.head.cal.err.${fit.error}` as I18nKey)}</p>}
          {result && (
            <>
              <p className="hint num cam-ok">{t('camera.head.cal.preview', { x: fmt(result.offset_mm[0], 2), y: fmt(result.offset_mm[1], 2), scale: fmt(headPxPerMm(result), 2), rms: fmt(result.rms_mm, 2) })}</p>
              {result.rms_mm > 0.5 && <p className="warn-line" role="alert">{t('camera.head.cal.rmsHigh')}</p>}
              <button type="button" className="btn btn--accent" onClick={() => saveHeadCalibration(result)}>{t('camera.head.cal.save')}</button>
            </>
          )}
        </div>
      )}
      {cal && !measuring && (
        <button type="button" className="btn" onClick={clearHeadCalibration}>{t('camera.head.cal.clear')}</button>
      )}
      <HeadMarksBlock />
    </div>
  );
}
