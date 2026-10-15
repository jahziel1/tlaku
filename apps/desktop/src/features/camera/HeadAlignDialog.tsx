import { Camera, Check, Move, RotateCcw, Square, X } from 'lucide-react';
import { useId, useState } from 'react';
import type { HeadAlignIssue, HeadAlignReport } from '@/api/types';
import { Modal } from '@/features/history';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { NumberField } from '@/ui/NumberField';
import {
  headAlignBlockedReason,
  resolveHeadAlign,
  saveBlockedReason,
  saveHeadAlignment,
  startHeadAlign,
} from './headAlign';
import { HEAD_ALIGN_MAX_GRID, HEAD_ALIGN_MIN_GRID, useHeadAlign, type HeadAlignSettings, type ShotInfo } from './headAlignState';
import { startCamera } from './session';
import './camera.css';

/** Cámara y alturas con las que se enciende la corrección de paralaje la primera vez. */
const DEFAULT_CAMERA_HEIGHT_MM = 250;
const DEFAULT_FEATURE_HEIGHT_MM = 40;

/** Asistente: el cabezal recorre puntos conocidos con el láser apagado y se ajusta la cámara fija. */
export function HeadAlignDialog({ onClose }: { onClose: () => void }) {
  const titleId = useId();
  const phase = useHeadAlign((s) => s.phase);
  return (
    <Modal labelId={titleId} onClose={onClose} wide>
      <div className="cam-dialog cam-ha">
        <div className="modal__head">
          <div className="grow">
            <h2 id={titleId} className="modal__title">{t('camera.headAlign.dialog.title')}</h2>
            <p className="hint">{t('camera.headAlign.dialog.subtitle')}</p>
          </div>
          <button type="button" className="icon-btn" aria-label={t('camera.headAlign.close')} title={t('camera.headAlign.close')} onClick={onClose}>
            <X size={16} aria-hidden="true" />
          </button>
        </div>
        <div className="cam-ha__body">
          {phase === 'config' && <ConfigPane />}
          {phase === 'running' && <RunningPane />}
          {phase === 'result' && <ResultPane onClose={onClose} />}
        </div>
      </div>
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// Preparación
// ---------------------------------------------------------------------------

function ConfigPane() {
  const blocked = useStore((s) => headAlignBlockedReason(s));
  const lens = useStore((s) => s.cameraSetup.calibration);
  const camStatus = useStore((s) => s.cameraStatus);
  return (
    <ConfigView
      settings={useHeadAlign((s) => s.settings)}
      set={useHeadAlign((s) => s.setSettings)}
      confirmed={useHeadAlign((s) => s.confirmed)}
      setConfirmed={useHeadAlign((s) => s.setConfirmed)}
      error={useHeadAlign((s) => s.error)}
      blocked={blocked}
      hasLens={lens !== null}
      camStatus={camStatus}
      onStart={() => void startHeadAlign()}
      onStartCamera={() => void startCamera()}
    />
  );
}

export interface ConfigViewProps {
  settings: HeadAlignSettings;
  set: (patch: Partial<HeadAlignSettings>) => void;
  confirmed: boolean;
  setConfirmed: (v: boolean) => void;
  error: string | null;
  /** Motivo por el que todavía no se puede empezar (clave de texto), o `null`. */
  blocked: I18nKey | null;
  hasLens: boolean;
  camStatus: 'off' | 'starting' | 'live';
  onStart: () => void;
  onStartCamera: () => void;
}

/** Preparación: explicación, opciones, aviso de seguridad y confirmación. */
export function ConfigView({ settings, set, confirmed, setConfirmed, error, blocked, hasLens, camStatus, onStart, onStartCamera }: ConfigViewProps) {
  const lens = hasLens;
  const parallax = settings.cameraHeightMm !== null && settings.featureHeightMm !== null;
  const reason = blocked ? t(blocked) : !confirmed ? t('camera.headAlign.confirm.needed') : null;
  const confirmId = useId();

  return (
    <>
      <section className="cam-cal__block">
        <p>{t('camera.headAlign.explain')}</p>
        <ol className="cam-steps">
          <li>{t('camera.headAlign.step1')}</li>
          <li>{t('camera.headAlign.step2')}</li>
          <li>{t('camera.headAlign.step3')}</li>
          <li>{t('camera.headAlign.step4')}</li>
        </ol>
        <p className="hint">{t('camera.headAlign.visibleHint')}</p>
        {!lens && <p className="warn-line" role="status">{t('camera.headAlign.noLens')}</p>}
      </section>

      <details className="cam-cal__block cam-ha__options">
        <summary>{t('camera.headAlign.options')}</summary>
        <div className="field-grid">
          <div className="field">
            <label className="field__label" htmlFor="cam-ha-grid">{t('camera.headAlign.grid')}</label>
            <select id="cam-ha-grid" className="select" value={settings.grid} onChange={(e) => set({ grid: Number(e.target.value) })}>
              {Array.from({ length: HEAD_ALIGN_MAX_GRID - HEAD_ALIGN_MIN_GRID + 1 }, (_, i) => HEAD_ALIGN_MIN_GRID + i).map((n) => (
                <option key={n} value={n}>{t('camera.headAlign.gridOption', { n, total: n * n })}</option>
              ))}
            </select>
          </div>
          <div className="field">
            <label className="field__label" htmlFor="cam-ha-method">{t('camera.headAlign.method')}</label>
            <select id="cam-ha-method" className="select" value={settings.method} onChange={(e) => set({ method: e.target.value as typeof settings.method })}>
              <option value="auto">{t('camera.headAlign.method.auto')}</option>
              <option value="led">{t('camera.headAlign.method.led')}</option>
              <option value="diff">{t('camera.headAlign.method.diff')}</option>
            </select>
          </div>
          <NumberField label={t('camera.headAlign.margin')} unit="mm" value={settings.marginMm} min={0} max={150} step={5} onCommit={(v) => set({ marginMm: v })} />
        </div>
        <label className="mc-check">
          <input
            type="checkbox"
            checked={parallax}
            onChange={(e) => set(e.target.checked ? { cameraHeightMm: DEFAULT_CAMERA_HEIGHT_MM, featureHeightMm: DEFAULT_FEATURE_HEIGHT_MM } : { cameraHeightMm: null, featureHeightMm: null })}
          />
          {t('camera.headAlign.parallax')}
        </label>
        {parallax && (
          <div className="field-grid">
            <NumberField label={t('camera.headAlign.cameraHeight')} unit="mm" value={settings.cameraHeightMm ?? DEFAULT_CAMERA_HEIGHT_MM} min={20} max={2000} step={5} onCommit={(v) => set({ cameraHeightMm: v })} />
            <NumberField label={t('camera.headAlign.featureHeight')} unit="mm" value={settings.featureHeightMm ?? DEFAULT_FEATURE_HEIGHT_MM} min={0} max={500} step={1} onCommit={(v) => set({ featureHeightMm: v })} />
          </div>
        )}
        <p className="hint">{t('camera.headAlign.parallaxHint')}</p>
        <div className="field-grid">
          <NumberField label={t('camera.headAlign.offsetX')} unit="mm" value={settings.offset[0]} min={-100} max={100} step={0.5} onCommit={(v) => set({ offset: [v, settings.offset[1]] })} />
          <NumberField label={t('camera.headAlign.offsetY')} unit="mm" value={settings.offset[1]} min={-100} max={100} step={0.5} onCommit={(v) => set({ offset: [settings.offset[0], v] })} />
        </div>
        <p className="hint">{t('camera.headAlign.offsetHint')}</p>
      </details>

      <section className="cam-cal__block">
        <p className="warn-line" role="note">{t('camera.headAlign.warning')}</p>
        <label className="mc-check" htmlFor={confirmId}>
          <input id={confirmId} type="checkbox" checked={confirmed} onChange={(e) => setConfirmed(e.target.checked)} />
          {t('camera.headAlign.confirm')}
        </label>
        <div className="row cam-wrap">
          {camStatus !== 'live' && (
            <button type="button" className="btn" disabled={camStatus === 'starting'} onClick={onStartCamera}>
              <Camera size={14} aria-hidden="true" />
              {t('camera.marks.startCamera')}
            </button>
          )}
          <button type="button" className="btn btn--accent" aria-disabled={reason !== null} title={reason ?? undefined} onClick={() => reason === null && onStart()}>
            <Move size={14} aria-hidden="true" />
            {t('camera.headAlign.start')}
          </button>
        </div>
        {reason && <p className="hint" role="status">{reason}</p>}
        {error && <p className="hint hint--danger" role="alert">{error}</p>}
      </section>
    </>
  );
}

// ---------------------------------------------------------------------------
// Recorrido
// ---------------------------------------------------------------------------

function RunningPane() {
  const shots = useHeadAlign((s) => s.shots);
  const round = useHeadAlign((s) => s.round);
  const cancelRequested = useHeadAlign((s) => s.cancelRequested);
  const requestCancel = useHeadAlign((s) => s.requestCancel);
  return <RunningView shots={shots} round={round} cancelRequested={cancelRequested} onStop={() => requestCancel(true)} />;
}

/** Avance del recorrido: una línea por punto con su miniatura y estado. */
export function RunningView({ shots, round, cancelRequested, onStop }: { shots: ShotInfo[]; round: number; cancelRequested: boolean; onStop: () => void }) {
  const done = shots.filter((s) => s.state === 'shot').length;
  const current = shots.find((s) => s.state === 'moving') ?? null;
  const label = t('camera.headAlign.progress', { done, total: shots.length });

  return (
    <section className="cam-cal__block" aria-label={t('camera.headAlign.running')}>
      <h3 className="panel-title">{t('camera.headAlign.running')}</h3>
      <p className="hint num" role="status" aria-live="polite">
        {label}
        {current && ` · ${t('camera.headAlign.moving', { x: fmt(current.mm[0], 1), y: fmt(current.mm[1], 1) })}`}
        {round > 0 && ` · ${t('camera.headAlign.round', { n: round })}`}
      </p>
      <div className="cam-meter" role="meter" aria-valuemin={0} aria-valuemax={Math.max(1, shots.length)} aria-valuenow={done} aria-label={label}>
        <div className="cam-meter__bar" style={{ width: `${shots.length ? Math.round((done / shots.length) * 100) : 0}%` }} />
      </div>
      <p className="hint">{t('camera.headAlign.laserOff')}</p>
      <ol className="cam-ha__shots" aria-label={t('camera.headAlign.shots')}>
        {shots.map((s, i) => <ShotItem key={s.key} shot={s} index={i + 1} />)}
      </ol>
      <button type="button" className="btn" disabled={cancelRequested} onClick={onStop}>
        <Square size={14} aria-hidden="true" />
        {cancelRequested ? t('camera.headAlign.stopping') : t('camera.headAlign.stop')}
      </button>
    </section>
  );
}

function ShotItem({ shot, index }: { shot: ShotInfo; index: number }) {
  return (
    <li className="cam-ha__shot" data-state={shot.state}>
      <span className="cam-ha__thumb" aria-hidden="true">{shot.thumb ? <img src={shot.thumb} alt="" /> : <span className="num">{index}</span>}</span>
      <span className="cam-ha__shot-text">
        <span className="num">{t('camera.headAlign.point', { n: index, x: fmt(shot.mm[0], 1), y: fmt(shot.mm[1], 1) })}</span>
        <span className="hint">{t(`camera.headAlign.state.${shot.state}` as I18nKey)}</span>
      </span>
    </li>
  );
}

// ---------------------------------------------------------------------------
// Resultado
// ---------------------------------------------------------------------------

/** Texto de un aviso del motor. */
export function issueText(i: HeadAlignIssue): string {
  switch (i.kind) {
    case 'not_detected':
      return t('camera.headAlign.issue.notDetected', { count: i.count });
    case 'dropped':
      return t('camera.headAlign.issue.dropped', { count: i.count });
    case 'few_points':
      return t('camera.headAlign.issue.fewPoints', { used: i.used });
    case 'partial_coverage': {
      const [x0, y0, x1, y1] = i.covered_mm;
      return t('camera.headAlign.issue.partialCoverage', { x0: fmt(x0, 0), x1: fmt(x1, 0), y0: fmt(y0, 0), y1: fmt(y1, 0) });
    }
    case 'high_error':
      return t('camera.headAlign.issue.highError', { mm: fmt(i.loo_rms_mm, 2) });
    case 'lens_estimated':
      return t('camera.headAlign.issue.lensEstimated', { model: i.model === 'fisheye' ? t('camera.headAlign.model.fisheye') : t('camera.headAlign.model.brown'), before: fmt(i.rms_before_mm, 2), after: fmt(i.rms_after_mm, 2) });
  }
}

function ResultPane({ onClose }: { onClose: () => void }) {
  const report = useHeadAlign((s) => s.report);
  const shots = useHeadAlign((s) => s.shots);
  const saved = useHeadAlign((s) => s.saved);
  const error = useHeadAlign((s) => s.error);
  const settings = useHeadAlign((s) => s.settings);
  const back = useHeadAlign((s) => s.backToConfig);
  const set = useHeadAlign((s) => s.setSettings);
  const lens = useStore((s) => s.cameraSetup.calibration);
  const [busy, setBusy] = useState(false);
  if (!report) return null;
  return (
    <ResultView
      report={report}
      shots={shots}
      saved={saved}
      error={error}
      settings={settings}
      set={set}
      hasLens={lens !== null}
      busy={busy}
      blocked={saveBlockedReason(report)}
      onRecalc={() => {
        setBusy(true);
        void resolveHeadAlign(useHeadAlign.getState().settings).finally(() => setBusy(false));
      }}
      onSave={() => saveHeadAlignment(report) && onClose()}
      onRepeat={back}
      onDiscard={onClose}
    />
  );
}

export interface ResultViewProps {
  report: HeadAlignReport;
  shots: ShotInfo[];
  saved: boolean;
  error: string | null;
  settings: HeadAlignSettings;
  set: (patch: Partial<HeadAlignSettings>) => void;
  hasLens: boolean;
  busy: boolean;
  /** Por qué no se puede guardar, o `null`. */
  blocked: 'noFit' | 'size' | null;
  onRecalc: () => void;
  onSave: () => void;
  onRepeat: () => void;
  onDiscard: () => void;
}

/** Resultado: calidad y error, avisos, tabla por punto, ajuste del desfase y guardar. */
export function ResultView({ report, shots, saved, error, settings, set, hasLens, busy, blocked, onRecalc, onSave, onRepeat, onDiscard }: ResultViewProps) {
  const fit = report.fit;
  const total = report.frames.length;
  const lens = hasLens;

  return (
    <>
      <section className="cam-cal__block cam-result" data-grade={fit?.quality ?? 'poor'} aria-live="polite">
        <h3 className="panel-title">{t('camera.headAlign.result')}</h3>
        {fit ? (
          <>
            <p className="cam-result__rms">
              <span className="num cam-result__value">{fmt(fit.loo_rms_mm, 2)} mm</span>
              <span className="cam-result__grade">{t(`camera.headAlign.quality.${fit.quality}` as I18nKey)}</span>
            </p>
            <p className="hint">{t('camera.headAlign.found', { found: report.detected, total })}</p>
            <p className="hint">{report.method === 'led' ? t('camera.headAlign.usedLed') : t('camera.headAlign.usedDiff')}</p>
            <p className="hint num">{t('camera.headAlign.fit', { rms: fmt(fit.rms_mm, 2), loo: fmt(fit.loo_rms_mm, 2), max: fmt(fit.loo_max_mm, 2), used: fit.used })}</p>
            <p className="hint num">{t('camera.headAlign.covered', { x0: fmt(fit.covered_mm[0], 0), x1: fmt(fit.covered_mm[2], 0), y0: fmt(fit.covered_mm[1], 0), y1: fmt(fit.covered_mm[3], 0) })}</p>
          </>
        ) : (
          <>
            <p className="warn-line" role="alert">{t('camera.headAlign.noFit')}</p>
            <p className="hint">{report.fit_error}</p>
            <p className="hint">{t('camera.headAlign.noFitHelp')}</p>
          </>
        )}
        {report.issues.length > 0 && (
          <ul className="cam-ha__issues" aria-label={t('camera.headAlign.issues')}>
            {report.issues.map((i, k) => <li key={k} className="warn-line">{issueText(i)}</li>)}
          </ul>
        )}
        {report.lens && <p className="hint">{t('camera.headAlign.lensWillSave')}</p>}
        {!lens && !report.lens && fit && <p className="hint">{t('camera.headAlign.noLens')}</p>}
      </section>

      <FramesTable report={report} shots={shots} />

      {fit && (
        <details className="cam-cal__block cam-ha__options">
          <summary>{t('camera.headAlign.adjust')}</summary>
          <div className="field-grid">
            <NumberField label={t('camera.headAlign.offsetX')} unit="mm" value={settings.offset[0]} min={-100} max={100} step={0.5} onCommit={(v) => set({ offset: [v, settings.offset[1]] })} />
            <NumberField label={t('camera.headAlign.offsetY')} unit="mm" value={settings.offset[1]} min={-100} max={100} step={0.5} onCommit={(v) => set({ offset: [settings.offset[0], v] })} />
          </div>
          <p className="hint">{t('camera.headAlign.adjustHint')}</p>
          <button type="button" className="btn" disabled={busy} onClick={onRecalc}>
            <RotateCcw size={14} aria-hidden="true" />
            {t('camera.headAlign.recalc')}
          </button>
        </details>
      )}

      <div className="row cam-wrap">
        <button
          type="button"
          className="btn btn--accent"
          aria-disabled={blocked !== null || saved}
          title={blocked ? t(`camera.headAlign.saveBlocked.${blocked}` as I18nKey) : undefined}
          onClick={() => blocked === null && !saved && onSave()}
        >
          <Check size={14} aria-hidden="true" />
          {t('camera.headAlign.save')}
        </button>
        <button type="button" className="btn" onClick={onRepeat}>{t('camera.headAlign.repeat')}</button>
        <button type="button" className="btn" onClick={onDiscard}>{t('camera.headAlign.discard')}</button>
      </div>
      {blocked && <p className="hint" role="status">{t(`camera.headAlign.saveBlocked.${blocked}` as I18nKey)}</p>}
      {error && <p className="hint hint--danger" role="alert">{error}</p>}
    </>
  );
}

/** Una fila por foto: miniatura, posición, qué se vio y el error en mm. */
function FramesTable({ report, shots }: { report: HeadAlignReport; shots: ShotInfo[] }) {
  return (
    <table className="cam-marks__table cam-ha__table" aria-label={t('a11y.table.headAlign')}>
      <thead>
        <tr>
          <th scope="col">{t('camera.headAlign.col.point')}</th>
          <th scope="col">{t('camera.headAlign.col.position')}</th>
          <th scope="col">{t('camera.headAlign.col.seen')}</th>
          <th scope="col">{t('camera.headAlign.col.error')}</th>
        </tr>
      </thead>
      <tbody>
        {report.frames.map((f, i) => {
          const thumb = shots[i]?.thumb ?? null;
          const tone = f.px === null ? 'missing' : !f.used ? 'warn' : 'ok';
          return (
            <tr key={i} data-tone={tone}>
              <td className="num">
                <span className="cam-ha__thumb cam-ha__thumb--row" aria-hidden="true">{thumb ? <img src={thumb} alt="" /> : null}</span>
                {i + 1}
              </td>
              <td className="num">{fmt(f.machine_mm[0], 1)}, {fmt(f.machine_mm[1], 1)}</td>
              <td>{f.px === null ? t(`camera.headAlign.failure.${f.failure ?? 'no_change'}` as I18nKey) : !f.used ? t('camera.headAlign.discarded') : t('camera.headAlign.seen')}</td>
              <td className="num">{f.error_mm === null ? '—' : `${fmt(f.error_mm, 2)} mm`}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
