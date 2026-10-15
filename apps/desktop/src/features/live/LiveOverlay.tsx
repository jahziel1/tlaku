import { useEffect, useMemo, useRef } from 'react';
import { createPortal } from 'react-dom';
import { EyeOff } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { remainingSeconds } from '@/features/machine/jobSummary';
import { useNow } from '@/features/machine/hooks';
import { t } from '@/i18n';
import { fmtClock } from '@/lib/platform';
import { selectActiveDevice, useStore, usePref } from '@/store';
import { jobElapsedSeconds } from '@/store/machine';
import { drawLive, readLivePalette } from './draw';
import { LiveLayers } from './layers';
import { getLiveService, useLive } from './service';
import { liveSummary, routeNote, stateText } from './text';
import { useCanvasArea } from './useCanvasArea';
import './live.css';

/** Milisegundos de cada cuadro que se dedican a trazar lo quemado nuevo y lo pendiente. */
const BURN_BUDGET_MS = 5;
const PENDING_BUDGET_MS = 3;

const makeCanvas = () => document.createElement('canvas');
const LAYER_STYLE = { fallbackCut: '#c0392b', fallbackEngrave: '#2e86c1', travel: '#6b5a48' };
/** Cuántas veces puede cambiar el zoom respecto del de las capas antes de armarlas de nuevo. */
export const ZOOM_REBUILD = 2;

/**
 * Lienzo de la vista en vivo: lo pendiente, lo quemado y el cabezal sobre el diseño, en las mismas
 * coordenadas y con el mismo zoom y paneo. Solo se monta con la vista encendida; el bucle de
 * dibujo (`requestAnimationFrame`) corre únicamente mientras algo se mueve, se está poniendo al día
 * lo ya hecho o cambia la vista, y nunca lee ni envía nada a la máquina.
 */
function LiveCanvas({ jobSeq, routeReady }: { jobSeq: number; routeReady: boolean }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const viewport = useStore((s) => s.viewport);
  const info = useLive(useShallow((s) => ({ fraction: s.fraction, source: s.source, ended: s.ended, route: s.route })));
  const job = useStore((s) => s.jobState);
  const summary = liveSummary(info, job, routeReady);

  useEffect(() => {
    const canvas = ref.current;
    const svc = getLiveService();
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx || !svc) return;
    const route = routeReady ? svc.route : null;
    const store = useStore;
    const travelOn = () => store.getState().previewView.showTravel;
    let dpr = window.devicePixelRatio || 1;
    const build = () => (route ? new LiveLayers(route, LAYER_STYLE, makeCanvas, { showTravel: travelOn(), zoom: store.getState().zoom, wantScale: store.getState().zoom * dpr * 1.25 }) : null);
    let layers = build();
    // Con un zoom muy distinto del de las capas se arma otro juego en segundo plano (grosores y resolución
    // al zoom nuevo); mientras no se pone al día se sigue dibujando el anterior, y entonces se cambia.
    let next: LiveLayers | null = null;
    let palette = readLivePalette();
    let raf = 0;
    let lastT = -1;
    // Solo en desarrollo: deja ver las capas a `scripts/live-verify.mjs`.
    if (import.meta.env.DEV) (window as unknown as { __liveLayers?: () => LiveLayers | null }).__liveLayers = () => layers;

    const size = () => {
      const { width, height } = store.getState().viewport;
      dpr = window.devicePixelRatio || 1;
      canvas.width = Math.max(1, Math.round(width * dpr));
      canvas.height = Math.max(1, Math.round(height * dpr));
    };
    const run = () => {
      raf = 0;
      const s = store.getState();
      const frame = svc.frame();
      let more = false;
      if (layers) {
        layers.setShowTravel(travelOn());
        if (frame && !layers.advance(frame.move, frame.frac, BURN_BUDGET_MS)) more = true;
        if (!layers.pendingDone && !layers.renderPending(PENDING_BUDGET_MS)) more = true;
        const z = s.zoom;
        if (!next && layers.builtZoom > 0 && (z / layers.builtZoom > ZOOM_REBUILD || z / layers.builtZoom < 1 / ZOOM_REBUILD)) next = build();
        if (next) {
          next.setShowTravel(travelOn());
          const caught = !frame || next.advance(frame.move, frame.frac, BURN_BUDGET_MS);
          const pend = next.pendingDone || next.renderPending(PENDING_BUDGET_MS);
          if (caught && pend && layers.pendingDone) {
            layers = next;
            next = null;
          } else more = true;
        }
      }
      if (frame && (frame.mode === 'running' || Math.abs(frame.t - lastT) > 1e-4)) more = true;
      lastT = frame?.t ?? lastT;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const device = selectActiveDevice(s);
      drawLive({ ctx, width: s.viewport.width, height: s.viewport.height, view: { zoom: s.zoom, panX: s.panX, panY: s.panY }, bed: device.bed, layers, frame, palette });
      if (more) raf = requestAnimationFrame(run);
    };
    const kick = () => {
      if (!raf) raf = requestAnimationFrame(run);
    };
    size();
    kick();
    const unsubStore = store.subscribe((s, prev) => {
      if (s.viewport !== prev.viewport) {
        size();
        kick();
      } else if (s.zoom !== prev.zoom || s.panX !== prev.panX || s.panY !== prev.panY || s.status !== prev.status || s.jobState !== prev.jobState || s.previewView.showTravel !== prev.previewView.showTravel) {
        kick();
      }
      if (s.prefs.theme !== prev.prefs.theme) {
        // Los tokens CSS cambian después del cambio de tema: se lee en el siguiente cuadro.
        requestAnimationFrame(() => {
          palette = readLivePalette();
          kick();
        });
      }
    });
    const unsubLive = useLive.subscribe(kick);
    return () => {
      cancelAnimationFrame(raf);
      unsubStore();
      unsubLive();
    };
    // `jobSeq`: un trabajo nuevo trae una ruta y capas nuevas.
  }, [jobSeq, routeReady]);

  return <canvas ref={ref} className="live-canvas" data-testid="live-canvas" role="img" aria-label={summary} style={{ width: viewport.width, height: viewport.height }} />;
}

/** Porcentaje, tiempos y estado, en un cuadro sobre el lienzo. */
function LiveHud() {
  const info = useLive(useShallow((s) => ({ fraction: s.fraction, source: s.source, ended: s.ended, route: s.route, emitting: s.emitting })));
  const job = useStore((s) => s.jobState);
  const timing = useStore(useShallow((s) => ({ jobStartedAt: s.jobStartedAt, jobPausedAt: s.jobPausedAt, jobPausedMs: s.jobPausedMs, jobEndedAt: s.jobEndedAt })));
  const setPref = useStore((s) => s.setPref);
  const now = useNow(job === 'running');
  const routeReady = info.route === 'ready';
  const elapsed = jobElapsedSeconds(timing, now);
  const active = job === 'running' || job === 'paused';
  const total = getLiveService()?.route?.total ?? null;
  const remaining = active ? remainingSeconds(info.fraction, total, elapsed) : 0;
  const percent = Math.round(info.fraction * 100);
  const state = stateText(info, job, routeReady);
  // La región en vivo solo cambia cada 10 % y con el estado: no se anuncia cada cuadro.
  const bucket = info.ended === 'finished' ? 100 : Math.floor(percent / 10) * 10;
  const announce = useMemo(() => t('live.canvas.aria', { percent: bucket, state, source: '' }).trim(), [bucket, state]);
  const note = routeNote(info.route);
  return (
    <div className="live-hud" data-testid="live-hud">
      <div className="live-hud__head">
        <strong>{t('live.hud.title')}</strong>
        {info.emitting && <span className="live-hud__laser">{t('live.laser.on')}</span>}
        <button type="button" className="icon-btn" title={t('live.hud.hideTip')} aria-label={t('live.hud.hide')} onClick={() => setPref('liveView', false)}>
          <EyeOff size={14} aria-hidden="true" />
        </button>
      </div>
      {note ? (
        <p className="live-hud__note">{note}</p>
      ) : (
        <dl className="live-hud__meta num" aria-hidden="true">
          <div className="live-hud__pct">
            <dt className="sr-only">{t('machine.progress')}</dt>
            <dd>{t('live.hud.percent', { percent })}</dd>
          </div>
          <div>
            <dt>{t('live.hud.elapsed')}</dt>
            <dd>{fmtClock(elapsed)}</dd>
          </div>
          <div>
            <dt>{t('live.hud.remaining')}</dt>
            <dd>{remaining === null ? '—' : fmtClock(remaining)}</dd>
          </div>
        </dl>
      )}
      <p className="sr-only" role="status" aria-live="polite">{announce}</p>
    </div>
  );
}

/**
 * Vista en vivo sobre el lienzo del diseño (portal en `.canvas-area`, como la rejilla del galvo).
 * Apagada o sin trabajo no monta nada.
 */
export function LiveOverlay() {
  const area = useCanvasArea();
  const enabled = usePref('liveView');
  const jobSeq = useLive((s) => s.jobSeq);
  const route = useLive((s) => s.route);
  if (!area || !enabled || jobSeq === 0) return null;
  return createPortal(
    <>
      {route === 'ready' && <LiveCanvas key={jobSeq} jobSeq={jobSeq} routeReady />}
      <LiveHud />
    </>,
    area,
  );
}
