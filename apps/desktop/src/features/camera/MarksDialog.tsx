import { X } from 'lucide-react';
import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { getApiKind } from '@/api';
import type { MarksReport } from '@/api/types';
import { Modal } from '@/features/history';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { NumberField } from '@/ui/NumberField';
import { MIN_HEIGHT_GAP_MM } from './height';
import { acceptMarks, captureAndDetect } from './marks';
import { useMarks } from './marksState';
import { markTone, missingMarkPositions, reportGrade, type MarkTone } from './marksView';
import { getCorrectedCanvas, startCamera } from './session';
import './camera.css';

interface Captured {
  report: MarksReport;
  png: string;
  size: [number, number];
}

/** Asistente de captura: foto de la cama con las marcas, detección con el motor, revisión y aceptar. */
export function MarksDialog({ onClose }: { onClose: () => void }) {
  const titleId = useId();
  const status = useStore((s) => s.cameraStatus);
  const frozen = useStore((s) => s.cameraFrozen);
  const lens = useStore((s) => s.cameraSetup.calibration);
  const savedHeight = useStore((s) => s.cameraSetup.prefs.material_height_mm ?? 0);
  const notify = useStore((s) => s.notify);
  const count = useMarks((s) => s.count);
  const [height, setHeight] = useState(savedHeight);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [shot, setShot] = useState<Captured | null>(null);
  const hasImage = status === 'live' || frozen;

  const detect = async () => {
    setBusy(true);
    setError(null);
    try {
      setShot(await captureAndDetect(count));
    } catch (e) {
      setShot(null);
      setError(t('camera.marks.fail', { error: errorMessage(e) }));
    } finally {
      setBusy(false);
    }
  };

  const accept = () => {
    if (!shot) return;
    const reason = acceptMarks(shot.report, shot.size, height);
    if (reason === 'tooClose') return setError(t('camera.marks.tooClose', { gap: MIN_HEIGHT_GAP_MM }));
    if (reason === 'tooFew') return setError(t('camera.marks.tooFew'));
    if (reason === 'noSpace') return setError(t('camera.marks.sizeChanged'));
    notify('success', t('camera.marks.accepted', { h: fmt(height, 1) }));
    onClose();
  };

  return (
    <Modal labelId={titleId} onClose={onClose} wide>
      <div className="cam-dialog">
        <div className="modal__head">
          <div className="grow">
            <h2 id={titleId} className="modal__title">{t('camera.marks.dialog.title')}</h2>
            <p className="hint">{t('camera.marks.dialog.subtitle')}</p>
          </div>
          <button type="button" className="icon-btn" aria-label={t('camera.marks.close')} title={t('camera.marks.close')} onClick={onClose}>
            <X size={16} aria-hidden="true" />
          </button>
        </div>
        <div className="cam-dialog__body">
          <section className="cam-cal__stage" aria-label={t('camera.marks.photo')}>
            {shot ? <Photo shot={shot} /> : <LivePreview active={hasImage} onStart={() => void startCamera()} starting={status === 'starting'} />}
            {shot && <p className="hint">{t('camera.marks.legend')}</p>}
            {getApiKind() === 'mock' && <p className="hint">{t('camera.marks.mockNote')}</p>}
          </section>

          <div className="cam-cal__side">
            <section className="cam-cal__block">
              <h3 className="panel-title">{t('camera.marks.steps.title')}</h3>
              <ol className="cam-steps">
                <li>{t('camera.marks.step1')}</li>
                <li>{t('camera.marks.step2')}</li>
                <li>{t('camera.marks.step3')}</li>
              </ol>
            </section>

            <section className="cam-cal__block">
              <NumberField label={t('camera.marks.height')} unit="mm" value={height} min={0} max={300} step={0.5} onCommit={setHeight} />
              <p className="hint">{t('camera.marks.heightHint')}</p>
              {!lens && <p className="warn-line" role="status">{t('camera.marks.noLens')}</p>}
              <button type="button" className="btn btn--accent" disabled={!hasImage || busy} onClick={() => void detect()}>
                {busy ? t('camera.marks.detecting') : shot ? t('camera.marks.retry') : t('camera.marks.detect')}
              </button>
              {error && <p className="hint hint--danger" role="alert">{error}</p>}
            </section>

            {shot && <Result report={shot.report} onAccept={accept} />}
          </div>
        </div>
      </div>
    </Modal>
  );
}

/** Copia a ~10 fps el cuadro corregido de la sesión en un canvas propio (el original lo usa el panel). */
function LivePreview({ active, onStart, starting }: { active: boolean; onStart: () => void; starting: boolean }) {
  const cv = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    if (!active) return;
    const id = setInterval(() => {
      const el = cv.current;
      const src = getCorrectedCanvas();
      if (!el || src.width === 0) return;
      if (el.width !== src.width || el.height !== src.height) {
        el.width = src.width;
        el.height = src.height;
      }
      el.getContext('2d')?.drawImage(src, 0, 0);
    }, 100);
    return () => clearInterval(id);
  }, [active]);
  return (
    <div className="cam-cal__frame cam-marks__frame">
      <canvas ref={cv} className="cam-marks__live" aria-hidden="true" />
      {!active && (
        <div className="cam-cal__empty">
          <p>{t('camera.marks.noCamera')}</p>
          <button type="button" className="btn btn--accent" onClick={onStart} disabled={starting}>
            {t('camera.marks.startCamera')}
          </button>
        </div>
      )}
    </div>
  );
}

const TONE_VAR: Record<MarkTone, string> = { ok: 'var(--ok)', warn: 'var(--warn)', bad: 'var(--danger)', missing: 'var(--danger)' };

/** Foto capturada con las marcas dibujadas encima (SVG en píxeles de la imagen). */
export function Photo({ shot }: { shot: Captured }) {
  const { report, png, size } = shot;
  const [w, h] = size;
  const r = Math.max(6, w / 110);
  const stroke = Math.max(1.5, w / 520);
  const font = Math.max(10, w / 70);
  const missing = useMemo(() => missingMarkPositions(report), [report]);
  // Candidatos que no son ninguna marca encontrada (puntas de cruz, esquinas...).
  const extras = report.candidates.filter((c) => !report.marks.some((m) => m.image && Math.hypot(m.image[0] - c[0], m.image[1] - c[1]) < 4));
  return (
    <div className="cam-cal__frame cam-marks__frame" style={{ aspectRatio: `${w} / ${h}` }}>
      <img className="cam-marks__live" src={`data:image/png;base64,${png}`} alt={t('camera.marks.photo')} />
      <svg className="cam-marks__svg" viewBox={`0 0 ${w} ${h}`} aria-hidden="true" data-testid="marks-overlay">
        {extras.map(([x, y], i) => (
          <path key={`c${i}`} d={`M${x - r / 2} ${y - r / 2}L${x + r / 2} ${y + r / 2}M${x + r / 2} ${y - r / 2}L${x - r / 2} ${y + r / 2}`} stroke="var(--text-muted)" strokeWidth={stroke} opacity={0.7} />
        ))}
        {missing.map(({ index, at }) => (
          <g key={`m${index}`} data-tone="missing">
            <circle cx={at[0]} cy={at[1]} r={r * 1.4} fill="none" stroke={TONE_VAR.missing} strokeWidth={stroke} strokeDasharray={`${stroke * 3} ${stroke * 2}`} />
            <text x={at[0] + r * 1.7} y={at[1] - r} fill={TONE_VAR.missing} fontSize={font}>{index + 1}</text>
          </g>
        ))}
        {report.marks.map((m) => {
          if (!m.image) return null;
          const tone = markTone(m);
          const [x, y] = m.image;
          return (
            <g key={m.index} data-tone={tone}>
              <circle cx={x} cy={y} r={r} fill="none" stroke={TONE_VAR[tone]} strokeWidth={stroke * 1.4} />
              <path d={`M${x - r * 1.5} ${y}L${x + r * 1.5} ${y}M${x} ${y - r * 1.5}L${x} ${y + r * 1.5}`} stroke={TONE_VAR[tone]} strokeWidth={stroke} />
              <text x={x + r * 1.2} y={y - r * 1.1} fill={TONE_VAR[tone]} fontSize={font} fontWeight={650}>
                {m.index + 1}
                {m.residual_mm !== null ? ` · ${fmt(m.residual_mm, 2)}` : ''}
              </text>
            </g>
          );
        })}
      </svg>
    </div>
  );
}

export function Result({ report, onAccept }: { report: MarksReport; onAccept: () => void }) {
  const grade = reportGrade(report);
  const enough = report.used >= 4;
  return (
    <section className="cam-cal__block cam-result" data-grade={grade} aria-live="polite">
      <h3 className="panel-title">{t('camera.marks.result')}</h3>
      <p className="cam-result__rms">
        <span className="num cam-result__value">{fmt(report.loo_max_mm ?? report.max_residual_mm, 2)} mm</span>
        <span className="cam-result__grade">{t(`camera.marks.grade.${grade}` as I18nKey)}</span>
      </p>
      <p className="hint">{t('camera.marks.found', { used: report.used, expected: report.expected, found: report.found })}</p>
      <p className="hint num">{t('camera.marks.fit', { rms: fmt(report.rms_mm, 3), max: fmt(report.max_residual_mm, 2) })}</p>
      {report.loo_rms_mm !== null && report.loo_max_mm !== null ? (
        <p className="hint num">{t('camera.marks.loo', { rms: fmt(report.loo_rms_mm, 2), max: fmt(report.loo_max_mm, 2) })}</p>
      ) : (
        <p className="warn-line">{t('camera.marks.looNone')}</p>
      )}
      {report.warnings.map((w, i) => <p key={i} className="warn-line">{w}</p>)}
      <table className="cam-marks__table" aria-label={t('a11y.table.marks')}>
        <thead>
          <tr>
            <th scope="col">{t('camera.marks.col.mark')}</th>
            <th scope="col">{t('camera.marks.col.error')}</th>
            <th scope="col">{t('camera.marks.col.loo')}</th>
          </tr>
        </thead>
        <tbody>
          {report.marks.map((m) => (
            <tr key={m.index} data-tone={markTone(m)}>
              <td className="num">{m.index + 1}</td>
              <td className="num">{!m.image ? t('camera.marks.missing') : !m.used ? t('camera.marks.discarded') : `${fmt(m.residual_mm ?? 0, 2)} mm`}</td>
              <td className="num">{m.loo_mm !== null ? `${fmt(m.loo_mm, 2)} mm` : '—'}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <button type="button" className="btn btn--accent" disabled={!enough} title={!enough ? t('camera.marks.tooFew') : undefined} onClick={onAccept}>
        {t('camera.marks.accept')}
      </button>
    </section>
  );
}
