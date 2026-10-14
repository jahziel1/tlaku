import { X } from 'lucide-react';
import { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { getApi, getApiKind } from '@/api';
import type { CalibrationReport, LensModel, LensModelChoice } from '@/api/types';
import { Modal } from '@/features/history';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { applyLens } from './actions';
import { BOARD_CORNERS, PAGES, boardLayout, boardPdf, boardSvg, maxSquare, type PageKind } from './board';
import { saveBoard } from './boardFiles';
import {
  COVERAGE_COLS,
  COVERAGE_ROWS,
  MIN_VIEWS,
  RECOMMENDED_VIEWS,
  coverageGrid,
  coverageRatio,
  discardedViews,
  emptyRegions,
  rateRms,
} from './coverage';
import type { Vec2 } from './geometry';
import { getRawView, grabRawPng, parkRawView, startCamera } from './session';
import './camera.css';

/** Ancho máximo del cuadro que se manda a detectar en vivo (el costo crece con los píxeles). */
const LIVE_WIDTH = 800;
/** ~2 detecciones por segundo. */
const LIVE_INTERVAL_MS = 500;

interface View {
  png: string;
  corners: Vec2[];
  size: [number, number];
}

const scaleCorners = (pts: [number, number][], k: number): Vec2[] => pts.map(([x, y]) => [x / k, y / k]);

/** Asistente de calibración de lente: captura vistas del tablero, calcula y guarda. */
export function CalibrationDialog({ onClose }: { onClose: () => void }) {
  const titleId = useId();
  const status = useStore((s) => s.cameraStatus);
  const streamSize = useStore((s) => s.cameraStreamSize);
  const notify = useStore((s) => s.notify);
  const [views, setViews] = useState<View[]>([]);
  const [live, setLive] = useState<Vec2[] | null>(null);
  const [capturing, setCapturing] = useState(false);
  const [captureError, setCaptureError] = useState(false);
  const [computing, setComputing] = useState(false);
  const [report, setReport] = useState<CalibrationReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [squareMm, setSquareMm] = useState(25);
  const [model, setModel] = useState<LensModelChoice>('auto');

  const host = useRef<HTMLDivElement>(null);
  const overlay = useRef<HTMLCanvasElement>(null);

  // El video crudo vive en la sesión; aquí solo se muestra mientras el asistente está abierto.
  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const video = getRawView();
    video.className = 'cam-cal__video';
    el.prepend(video);
    return () => {
      video.className = '';
      parkRawView();
    };
  }, []);

  // Detección en vivo a ~2 fps. Una sola petición a la vez: si el motor tarda, se salta el turno.
  useEffect(() => {
    if (status !== 'live') {
      setLive(null);
      return;
    }
    let busy = false;
    let alive = true;
    const tick = async () => {
      if (busy) return;
      busy = true;
      try {
        const frame = await grabRawPng(LIVE_WIDTH);
        if (!frame || !alive) return;
        const found = await getApi().detectBoard({ image_png_base64: frame.png, board: BOARD_CORNERS });
        if (alive) setLive(found ? scaleCorners(found, frame.scale) : null);
      } catch {
        if (alive) setLive(null);
      } finally {
        busy = false;
      }
    };
    const id = setInterval(() => void tick(), LIVE_INTERVAL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [status]);

  const size: [number, number] | null = views[0]?.size ?? streamSize;
  const grid = useMemo(
    () => (size ? coverageGrid(views.map((v) => v.corners), BOARD_CORNERS, size) : []),
    [views, size],
  );
  const ratio = coverageRatio(grid);
  const missing = emptyRegions(grid);

  // Dibujo sobre el video: cuadrícula de cobertura y esquinas detectadas.
  useEffect(() => {
    const cv = overlay.current;
    const dims = streamSize;
    if (!cv || !dims) return;
    cv.width = dims[0];
    cv.height = dims[1];
    const ctx = cv.getContext('2d');
    if (!ctx) return;
    const css = getComputedStyle(cv);
    const ok = css.getPropertyValue('--ok').trim() || '#45d6a4';
    const accent = css.getPropertyValue('--accent').trim() || '#8793ff';
    ctx.clearRect(0, 0, cv.width, cv.height);
    const cw = cv.width / COVERAGE_COLS;
    const ch = cv.height / COVERAGE_ROWS;
    const sameSize = !views[0] || (views[0].size[0] === dims[0] && views[0].size[1] === dims[1]);
    if (sameSize) {
      ctx.globalAlpha = 0.22;
      ctx.fillStyle = accent;
      grid.forEach((on, i) => {
        if (on) ctx.fillRect((i % COVERAGE_COLS) * cw, Math.floor(i / COVERAGE_COLS) * ch, cw, ch);
      });
      ctx.globalAlpha = 0.5;
      ctx.strokeStyle = accent;
      ctx.lineWidth = Math.max(1, cv.width / 640);
      for (let c = 1; c < COVERAGE_COLS; c++) {
        ctx.beginPath();
        ctx.moveTo(c * cw, 0);
        ctx.lineTo(c * cw, cv.height);
        ctx.stroke();
      }
      for (let r = 1; r < COVERAGE_ROWS; r++) {
        ctx.beginPath();
        ctx.moveTo(0, r * ch);
        ctx.lineTo(cv.width, r * ch);
        ctx.stroke();
      }
    }
    ctx.globalAlpha = 1;
    if (live) {
      const r = Math.max(3, cv.width / 220);
      ctx.strokeStyle = ok;
      ctx.fillStyle = ok;
      ctx.lineWidth = Math.max(1.5, cv.width / 500);
      const [bc, br] = BOARD_CORNERS;
      ctx.beginPath();
      for (let j = 0; j < br; j++) {
        const row = live.slice(j * bc, (j + 1) * bc);
        row.forEach(([x, y], i) => (i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y)));
      }
      ctx.globalAlpha = 0.55;
      ctx.stroke();
      ctx.globalAlpha = 1;
      for (const [x, y] of live) {
        ctx.beginPath();
        ctx.arc(x, y, r, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }, [live, grid, streamSize, views]);

  const capture = useCallback(async () => {
    setCapturing(true);
    setCaptureError(false);
    try {
      const frame = await grabRawPng(Infinity);
      if (!frame) return;
      const found = await getApi().detectBoard({ image_png_base64: frame.png, board: BOARD_CORNERS });
      if (!found) return setCaptureError(true);
      const view: View = { png: frame.png, corners: scaleCorners(found, frame.scale), size: [frame.width, frame.height] };
      setReport(null);
      // Todas las vistas deben tener el mismo tamaño: si cambió la resolución se empieza de nuevo.
      setViews((prev) => (prev[0] && (prev[0].size[0] !== view.size[0] || prev[0].size[1] !== view.size[1]) ? [view] : [...prev, view]));
    } catch {
      setCaptureError(true);
    } finally {
      setCapturing(false);
    }
  }, []);

  const compute = async () => {
    setComputing(true);
    setError(null);
    try {
      const r = await getApi().calibrateLens({
        images_png_base64: views.map((v) => v.png),
        board: BOARD_CORNERS,
        square_mm: squareMm,
        model,
      });
      setReport(r);
    } catch (e) {
      setError(t('camera.cal.computeFail', { error: errorMessage(e) }));
    } finally {
      setComputing(false);
    }
  };

  const mock = getApiKind() === 'mock';
  const enough = views.length >= MIN_VIEWS;

  const use = () => {
    if (!report) return;
    applyLens(report.calibration);
    notify('success', t('camera.cal.applied'));
    onClose();
  };

  return (
    <Modal labelId={titleId} onClose={onClose} wide>
      <div className="cam-dialog">
        <div className="modal__head">
          <div className="grow">
            <h2 id={titleId} className="modal__title">{t('camera.cal.title')}</h2>
            <p className="hint">{t('camera.cal.subtitle')}</p>
          </div>
          <button type="button" className="icon-btn" aria-label={t('camera.cal.close')} title={t('camera.cal.close')} onClick={onClose}>
            <X size={16} aria-hidden="true" />
          </button>
        </div>
        <div className="cam-dialog__body">
          <section className="cam-cal__stage" aria-label={t('camera.cal.title')}>
            <div className="cam-cal__frame" ref={host} style={{ aspectRatio: streamSize ? `${streamSize[0]} / ${streamSize[1]}` : '16 / 9' }}>
              <canvas ref={overlay} className="cam-cal__overlay" aria-hidden="true" />
              {status !== 'live' && (
                <div className="cam-cal__empty">
                  <p>{t('camera.cal.noCamera')}</p>
                  <button type="button" className="btn btn--accent" onClick={() => void startCamera()} disabled={status === 'starting'}>
                    {t('camera.cal.startCamera')}
                  </button>
                </div>
              )}
            </div>
            <div className="cam-cal__bar">
              <span className="badge" data-tone={live ? 'ok' : 'warn'} role="status">
                <span className="badge__dot" />
                {live ? t('camera.cal.found') : t('camera.cal.notFound')}
              </span>
              <button type="button" className="btn btn--accent" disabled={status !== 'live' || capturing || !live} onClick={() => void capture()} title={!live ? t('camera.cal.notFound') : undefined}>
                {capturing ? t('camera.cal.capturing') : t('camera.cal.capture')}
              </button>
            </div>
            {captureError && <p className="hint hint--danger" role="alert">{t('camera.cal.captureFail')}</p>}
            {mock && <p className="hint">{t('camera.cal.mockNote')}</p>}
            <p className="hint">{t('camera.cal.size', { w: size?.[0] ?? '–', h: size?.[1] ?? '–' })}</p>
          </section>

          <div className="cam-cal__side">
            <section className="cam-cal__block">
              <h3 className="panel-title">{t('camera.cal.steps.title')}</h3>
              <ol className="cam-steps">
                <li>{t('camera.cal.step1', { cols: BOARD_CORNERS[0], rows: BOARD_CORNERS[1] })}</li>
                <li>{t('camera.cal.step2')}</li>
                <li>{t('camera.cal.step3', { min: RECOMMENDED_VIEWS, abs: MIN_VIEWS })}</li>
              </ol>
            </section>

            <BoardSection squareMm={squareMm} onSquare={setSquareMm} />

            <section className="cam-cal__block">
              <h3 className="panel-title">{t('camera.cal.model.title')}</h3>
              <div className="field">
                <label className="field__label" htmlFor="cam-model">{t('camera.cal.model.label')}</label>
                <select
                  id="cam-model"
                  className="select"
                  value={model}
                  onChange={(e) => {
                    setReport(null);
                    setModel(e.target.value as LensModelChoice);
                  }}
                >
                  <option value="auto">{t('camera.cal.model.auto')}</option>
                  <option value="brown_conrady">{t('camera.cal.model.brown')}</option>
                  <option value="fisheye">{t('camera.cal.model.fisheye')}</option>
                </select>
              </div>
              <p className="hint">{t(`camera.cal.model.help.${model}` as I18nKey)}</p>
              <p className="hint hint--warn">{t('camera.cal.model.unvalidated')}</p>
            </section>

            <section className="cam-cal__block" aria-live="polite">
              <h3 className="panel-title">{t('camera.cal.views', { n: views.length })}</h3>
              <ul className="cam-chips">
                {views.map((_, i) => (
                  <li key={i} className="chip cam-chip">
                    <span className="num">{i + 1}</span>
                    <button
                      type="button"
                      className="icon-btn"
                      aria-label={t('camera.cal.viewRemove', { n: i + 1 })}
                      title={t('camera.cal.viewRemove', { n: i + 1 })}
                      onClick={() => {
                        setReport(null);
                        setViews((p) => p.filter((_, k) => k !== i));
                      }}
                    >
                      <X size={12} aria-hidden="true" />
                    </button>
                  </li>
                ))}
              </ul>
              <p className={views.length < RECOMMENDED_VIEWS ? 'hint' : 'hint cam-ok'}>
                {views.length < RECOMMENDED_VIEWS ? t('camera.cal.viewsFew', { min: RECOMMENDED_VIEWS }) : t('camera.cal.viewsHint', { min: RECOMMENDED_VIEWS })}
              </p>
              <div className="cam-meter" role="meter" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(ratio * 100)} aria-label={t('camera.cal.coverage', { pct: Math.round(ratio * 100) })}>
                <div className="cam-meter__bar" style={{ width: `${Math.round(ratio * 100)}%` }} />
              </div>
              <p className="hint">
                {t('camera.cal.coverage', { pct: Math.round(ratio * 100) })}.{' '}
                {views.length === 0
                  ? t('camera.cal.coverageHint')
                  : missing.length === 0
                    ? t('camera.cal.coverageFull')
                    : t('camera.cal.coverageMissing', { zones: missing.slice(0, 3).map((r) => t(`camera.region.${r}` as I18nKey)).join(', ') })}
              </p>
              <button type="button" className="btn btn--accent" disabled={!enough || computing} title={!enough ? t('camera.cal.computeNeed', { min: MIN_VIEWS }) : undefined} onClick={() => void compute()}>
                {computing ? t('camera.cal.computing') : t('camera.cal.compute')}
              </button>
              {!enough && <p className="hint">{t('camera.cal.computeNeed', { min: MIN_VIEWS })}</p>}
              {error && <p className="hint hint--danger" role="alert">{error}</p>}
            </section>

            {report && <ResultBlock report={report} total={views.length} onUse={use} onDiscard={() => setReport(null)} />}
          </div>
        </div>
      </div>
    </Modal>
  );
}

function ResultBlock({ report, total, onUse, onDiscard }: { report: CalibrationReport; total: number; onUse: () => void; onDiscard: () => void }) {
  const c = report.calibration;
  const grade = rateRms(c.rms_error_px);
  const dropped = discardedViews(total, report.used_views);
  return (
    <section className="cam-cal__block cam-result" data-grade={grade} aria-live="polite">
      <h3 className="panel-title">{t('camera.cal.result.title')}</h3>
      <p className="cam-result__rms">
        <span className="num cam-result__value">{fmt(c.rms_error_px, 2)} px</span>
        <span className="cam-result__grade">{t(`camera.grade.${grade}` as I18nKey)}</span>
      </p>
      <p className="hint">{t(`camera.grade.${grade}Hint` as I18nKey)}</p>
      <dl className="kv cam-kv">
        <div><dt>{t('camera.cal.result.views')}</dt><dd className="num">{report.used_views.length} / {total}</dd></div>
        <div><dt>{t('camera.cal.result.focal')}</dt><dd className="num">{fmt(c.fx, 0)} / {fmt(c.fy, 0)}</dd></div>
        <div><dt>{t('camera.cal.result.model')}</dt><dd>{t(`camera.cal.model.name.${c.model ?? 'brown_conrady'}` as I18nKey)}</dd></div>
        <div><dt>k1 / k2</dt><dd className="num">{fmt(c.k1, 3)} / {fmt(c.k2, 3)}</dd></div>
      </dl>
      {(report.model_scores?.length ?? 0) > 1 && (
        <table className="cam-scores" aria-label={t('camera.cal.result.scores')}>
          <thead>
            <tr>
              <th scope="col">{t('camera.cal.result.model')}</th>
              <th scope="col">{t('camera.cal.result.fit')}</th>
              <th scope="col">{t('camera.cal.result.reserved')}</th>
            </tr>
          </thead>
          <tbody>
            {report.model_scores!.map((sc) => (
              <tr key={sc.model} data-chosen={sc.chosen}>
                <th scope="row">{t(`camera.cal.model.name.${sc.model as LensModel}` as I18nKey)}{sc.chosen ? ` · ${t('camera.cal.result.chosen')}` : ''}</th>
                <td className="num">{fmt(sc.rms_px, 2)} px</td>
                <td className="num">{sc.cv_rms_px === null ? '–' : Number.isFinite(sc.cv_rms_px) ? `${fmt(sc.cv_rms_px, 2)} px` : t('camera.cal.result.noConverge')}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {dropped.length > 0 && <p className="warn-line">{t('camera.cal.result.discarded', { list: dropped.map((i) => i + 1).join(', ') })}</p>}
      {report.view_rms_px.length > 0 && (
        <p className="hint num">{t('camera.cal.result.perView', { list: report.view_rms_px.map((v, i) => `${report.used_views[i]! + 1}: ${fmt(v, 2)}`).join('  ') })}</p>
      )}
      {report.warnings.map((w, i) => <p key={i} className="warn-line">{w}</p>)}
      <div className="row">
        <button type="button" className="btn btn--accent grow" onClick={onUse}>{t('camera.cal.apply')}</button>
        <button type="button" className="btn" onClick={onDiscard}>{t('camera.cal.discard')}</button>
      </div>
    </section>
  );
}

function BoardSection({ squareMm, onSquare }: { squareMm: number; onSquare: (mm: number) => void }) {
  const [page, setPage] = useState<PageKind>('a4');
  const [msg, setMsg] = useState<{ text: string; error: boolean } | null>(null);
  const layout = useMemo(() => boardLayout(page, squareMm), [page, squareMm]);
  const caption = t('camera.board.caption', { cols: BOARD_CORNERS[0], rows: BOARD_CORNERS[1], size: fmt(squareMm, 1) });
  const [printing, setPrinting] = useState(false);

  const save = async (format: 'pdf' | 'svg') => {
    try {
      const path = await saveBoard(format, format === 'pdf' ? boardPdf(layout, caption) : boardSvg(layout, caption));
      if (path) setMsg({ text: t('camera.board.saved', { path }), error: false });
    } catch (e) {
      setMsg({ text: t('camera.board.saveFail', { error: errorMessage(e) }), error: true });
    }
  };

  const print = () => {
    setPrinting(true);
    const root = document.documentElement;
    root.classList.add('camera-printing');
    const done = () => {
      root.classList.remove('camera-printing');
      setPrinting(false);
      window.removeEventListener('afterprint', done);
    };
    window.addEventListener('afterprint', done);
    // El SVG se pinta en un tick; después el diálogo de impresión del sistema.
    setTimeout(() => window.print(), 50);
  };

  const shown = Math.round(squareMm * 10) / 10;
  return (
    <section className="cam-cal__block">
      <h3 className="panel-title">{t('camera.board.title')}</h3>
      <div className="field-grid">
        <div className="field">
          <label className="field__label" htmlFor="cam-sq">{t('camera.board.square')}</label>
          <div className="field__box">
            <input
              id="cam-sq"
              className="field__input num"
              type="number"
              min={5}
              max={maxSquare(page)}
              step={0.5}
              value={shown}
              onChange={(e) => onSquare(Number(e.target.value) || 0)}
            />
            <span className="field__unit">mm</span>
          </div>
        </div>
        <div className="field">
          <label className="field__label" htmlFor="cam-page">{t('camera.board.page')}</label>
          <select id="cam-page" className="select" value={page} onChange={(e) => setPage(e.target.value as PageKind)}>
            <option value="a4">{t('camera.board.a4')}</option>
            <option value="letter">{t('camera.board.letter')}</option>
          </select>
        </div>
      </div>
      {!layout.fits && <p className="hint hint--danger" role="alert">{t('camera.board.tooBig', { max: fmt(layout.maxSquareMm, 1) })}</p>}
      <p className="hint">{t('camera.board.printHint', { size: fmt(squareMm, 1) })}</p>
      <div className="row cam-wrap">
        <button type="button" className="btn" disabled={!layout.fits} onClick={() => void save('pdf')}>{t('camera.board.savePdf')}</button>
        <button type="button" className="btn" disabled={!layout.fits} onClick={() => void save('svg')}>{t('camera.board.saveSvg')}</button>
        <button type="button" className="btn" disabled={!layout.fits || printing} onClick={print}>{t('camera.board.print')}</button>
      </div>
      {msg && <p className={msg.error ? 'hint hint--danger' : 'hint'} role={msg.error ? 'alert' : 'status'}>{msg.text}</p>}
      {printing &&
        createPortal(
          <div id="camera-print-root" style={{ width: `${PAGES[page][0]}mm`, height: `${PAGES[page][1]}mm` }} dangerouslySetInnerHTML={{ __html: boardSvg(layout, caption).replace(/^<\?xml[^>]*\?>\s*/, '') }} />,
          document.body,
        )}
    </section>
  );
}
