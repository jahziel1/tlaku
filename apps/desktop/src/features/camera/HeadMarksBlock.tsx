import { ScanSearch } from 'lucide-react';
import { useId } from 'react';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { NumberField } from '@/ui/NumberField';
import { headPxPerMm } from './head';
import {
  clampMarksSize,
  currentMarksFit,
  marksOrigin,
  openHeadMarksAsDocument,
  runDetectHeadMarks,
  saveHeadMarksCalibration,
} from './headMarks';
import { HEAD_MARKS_MAX_SIZE, HEAD_MARKS_MIN_SIZE, HEAD_MARKS_MIN_USED, useHeadMarks } from './headMarksState';

/** Alineación automática del cabezal: grabar 9 marcas, verlas con la cámara y calcular el desfase. */
export function HeadMarksBlock() {
  const headId = useId();
  const size = useHeadMarks((s) => s.size);
  const origin = useHeadMarks((s) => s.origin);
  const obs = useHeadMarks((s) => s.obs);
  const detections = useHeadMarks((s) => s.detections);
  const busy = useHeadMarks((s) => s.busy);
  const error = useHeadMarks((s) => s.error);
  const set = useHeadMarks.getState;
  const bed = useStore((s) => selectActiveDevice(s).bed);
  const live = useStore((s) => s.cameraStatus === 'live');
  const connected = useStore((s) => s.connection.connected);
  const shown = marksOrigin(size, origin, bed);
  const fit = obs.length > 0 ? currentMarksFit() : null;
  const result = fit && 'calibration' in fit ? fit.calibration : null;
  const blocked = !live ? t('camera.reason.notLive') : !connected ? t('reason.needMachine') : null;

  return (
    <div className="cam-subsection" role="group" aria-labelledby={headId}>
      <h4 id={headId} className="cam-subtitle">{t('camera.head.marks.title')}</h4>
      <p className="hint">{t('camera.head.marks.help')}</p>
      <div className="field-grid">
        <NumberField label={t('camera.head.marks.size')} unit="mm" value={size} min={HEAD_MARKS_MIN_SIZE} max={HEAD_MARKS_MAX_SIZE} step={5} onCommit={(v) => set().setSize(clampMarksSize(v))} />
        <div />
        <NumberField label={t('camera.head.marks.x')} unit="mm" value={shown[0]} min={0} max={bed[0]} step={1} onCommit={(v) => set().setOrigin([v, shown[1]])} />
        <NumberField label={t('camera.head.marks.y')} unit="mm" value={shown[1]} min={0} max={bed[1]} step={1} onCommit={(v) => set().setOrigin([shown[0], v])} />
      </div>
      <button type="button" className="btn" onClick={() => void openHeadMarksAsDocument()}>{t('camera.head.marks.generate')}</button>
      <ol className="cam-steps">
        <li>{t('camera.head.marks.step1')}</li>
        <li>{t('camera.head.marks.step2')}</li>
        <li>{t('camera.head.marks.step3', { min: HEAD_MARKS_MIN_USED })}</li>
      </ol>
      <button type="button" className="btn btn--accent" aria-disabled={!!blocked || busy} title={blocked ?? undefined} onClick={() => !blocked && !busy && void runDetectHeadMarks()}>
        <ScanSearch size={14} aria-hidden="true" />
        {busy ? t('camera.head.marks.detecting') : obs.length > 0 ? t('camera.head.marks.detectMore') : t('camera.head.marks.detect')}
      </button>
      {error && <p className="warn-line" role="alert">{error}</p>}
      {detections.length > 0 && (
        <div role="status" aria-live="polite">
          <p className="hint num">{t('camera.head.marks.positions', { n: detections.length, marks: obs.length })}</p>
          {fit && 'error' in fit && <p className="warn-line" role="alert">{t(`camera.head.cal.err.${fit.error}` as I18nKey)}</p>}
          {result && (
            <>
              <p className="hint num cam-ok">{t('camera.head.cal.preview', { x: fmt(result.offset_mm[0], 2), y: fmt(result.offset_mm[1], 2), scale: fmt(headPxPerMm(result), 2), rms: fmt(result.rms_mm, 2) })}</p>
              {result.rms_mm > 0.5 && <p className="warn-line" role="alert">{t('camera.head.cal.rmsHigh')}</p>}
              <div className="row cam-wrap">
                <button type="button" className="btn btn--accent" onClick={() => saveHeadMarksCalibration(result)}>{t('camera.head.cal.save')}</button>
                <button type="button" className="btn" onClick={() => set().reset()}>{t('camera.head.marks.reset')}</button>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}
