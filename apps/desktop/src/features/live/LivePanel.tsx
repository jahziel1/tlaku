import { Maximize } from 'lucide-react';
import { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore, type KeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode, type RefObject } from 'react';
import { useNow } from '@/features/machine/hooks';
import { remainingSeconds } from '@/features/machine/jobSummary';
import { t } from '@/i18n';
import { fmtClock } from '@/lib/platform';
import { clampZoom, fitBounds, zoomAround, type ViewTransform } from '@/lib/viewport';
import { jobElapsedSeconds } from '@/store/machine';
import { drawLive, readLivePalette } from './draw';
import { LiveLayers } from './layers';
import type { LiveSnapshot, LiveSource } from './source';
import { liveSummary, routeNote, stateText } from './text';
import './live.css';

/** Milisegundos de cada cuadro que se dedican a trazar lo quemado nuevo y lo pendiente. */
const BURN_BUDGET_MS = 5;
const PENDING_BUDGET_MS = 3;
/** Cuántas veces puede cambiar el zoom respecto del de las capas antes de armarlas de nuevo. */
export const ZOOM_REBUILD = 2;
/** Margen (px) alrededor de la cama al encajarla. */
export const FIT_MARGIN = 10;
/** Tamaño de la vista mientras no se pueda medir (pruebas sin layout). */
const FALLBACK_SIZE = { w: 400, h: 240 };
const KEY_PAN = 32;

const makeCanvas = () => document.createElement('canvas');
const LAYER_STYLE = { fallbackCut: '#c0392b', fallbackEngrave: '#2e86c1', travel: '#6b5a48' };

const bedBounds = (bed: readonly [number, number]): [[number, number], [number, number]] => [[0, 0], [bed[0], bed[1]]];
export const fitBed = (bed: readonly [number, number], size: { w: number; h: number }): ViewTransform => fitBounds(bedBounds(bed), size.w, size.h, FIT_MARGIN);

/** La foto de la fuente, al día (se vuelve a pintar cuando cambia). */
export function useLiveSnapshot(source: LiveSource): LiveSnapshot {
  const subscribe = useCallback((fn: () => void) => source.subscribe(fn), [source]);
  const get = useCallback(() => source.snapshot(), [source]);
  return useSyncExternalStore(subscribe, get, get);
}

/** Tamaño (px CSS) de un elemento, siguiéndolo si cambia. Sin layout (pruebas) queda en `fallback`. */
function useBoxSize(ref: RefObject<HTMLElement | null>, fallback: { w: number; h: number }): { w: number; h: number } {
  const [size, setSize] = useState(fallback);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const read = () => {
      const r = el.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) setSize((cur) => (cur.w === Math.round(r.width) && cur.h === Math.round(r.height) ? cur : { w: Math.round(r.width), h: Math.round(r.height) }));
    };
    read();
    if (typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(read);
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref]);
  return size;
}

interface CanvasState {
  view: ViewTransform;
  size: { w: number; h: number };
  snap: LiveSnapshot;
}

/**
 * Lienzo de la simulación en el que se dibuja SOLO la vista en vivo, con su propia transformación
 * de vista (encajada en el tamaño de la ventana; no la del lienzo del diseño). El bucle de dibujo
 * (`requestAnimationFrame`) corre únicamente mientras algo se mueve, se está poniendo al día lo
 * ya hecho o cambia la vista, y nunca lee ni envía nada a la máquina.
 */
function LiveCanvas({ source, state, jobSeq, routeReady, label }: { source: LiveSource; state: CanvasState; jobSeq: number; routeReady: boolean; label: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const latest = useRef(state);
  const kickRef = useRef<() => void>(() => undefined);
  useLayoutEffect(() => {
    latest.current = state;
    kickRef.current();
  });

  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx) return;
    const route = routeReady ? source.route() : null;
    let dpr = window.devicePixelRatio || 1;
    const build = () => {
      const { view, snap } = latest.current;
      return route ? new LiveLayers(route, LAYER_STYLE, makeCanvas, { showTravel: snap.showTravel, zoom: view.zoom, wantScale: view.zoom * dpr * 1.25 }) : null;
    };
    let layers = build();
    // Con un zoom muy distinto del de las capas se arma otro juego en segundo plano (grosores y resolución
    // al zoom nuevo); mientras no se pone al día se sigue dibujando el anterior, y entonces se cambia.
    let next: LiveLayers | null = null;
    let palette = readLivePalette();
    let theme = latest.current.snap.theme;
    let raf = 0;
    let lastT = -1;
    let sized = '';
    // Solo en desarrollo: deja ver las capas y la vista a `scripts/live-verify.mjs`.
    if (import.meta.env.DEV) {
      const dev = window as unknown as { __liveLayers?: () => LiveLayers | null; __liveView?: () => { zoom: number; panX: number; panY: number; w: number; h: number } };
      dev.__liveLayers = () => layers;
      dev.__liveView = () => ({ ...latest.current.view, w: latest.current.size.w, h: latest.current.size.h });
    }

    const run = () => {
      raf = 0;
      const { view, size, snap } = latest.current;
      dpr = window.devicePixelRatio || 1;
      const key = `${size.w}x${size.h}@${dpr}`;
      if (key !== sized) {
        sized = key;
        canvas.width = Math.max(1, Math.round(size.w * dpr));
        canvas.height = Math.max(1, Math.round(size.h * dpr));
      }
      if (snap.theme !== theme) {
        // Los tokens CSS cambian después del cambio de tema: se lee en el siguiente cuadro.
        theme = snap.theme;
        requestAnimationFrame(() => {
          palette = readLivePalette();
          kick();
        });
      }
      const frame = source.frame();
      let more = false;
      if (layers) {
        layers.setShowTravel(snap.showTravel);
        if (frame && !layers.advance(frame.move, frame.frac, BURN_BUDGET_MS)) more = true;
        if (!layers.pendingDone && !layers.renderPending(PENDING_BUDGET_MS)) more = true;
        const z = view.zoom;
        if (!next && layers.builtZoom > 0 && (z / layers.builtZoom > ZOOM_REBUILD || z / layers.builtZoom < 1 / ZOOM_REBUILD)) next = build();
        if (next) {
          next.setShowTravel(snap.showTravel);
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
      drawLive({ ctx, width: size.w, height: size.h, view, bed: snap.bed, layers, frame, palette });
      if (more) raf = requestAnimationFrame(run);
    };
    const kick = () => {
      if (!raf) raf = requestAnimationFrame(run);
    };
    kickRef.current = kick;
    kick();
    const off = source.subscribe(kick);
    return () => {
      cancelAnimationFrame(raf);
      off();
      kickRef.current = () => undefined;
    };
    // `jobSeq`: un trabajo nuevo trae una ruta y capas nuevas.
  }, [source, jobSeq, routeReady]);

  return <canvas ref={ref} className="live-canvas" data-testid="live-canvas" role="img" aria-label={label} style={{ width: state.size.w, height: state.size.h }} />;
}

/** Porcentaje, estado del láser y tiempos, en una franja bajo la cama. */
function LiveFoot({ snap, children }: { snap: LiveSnapshot; children?: ReactNode }) {
  const now = useNow(snap.job === 'running');
  const routeReady = snap.route === 'ready';
  const elapsed = jobElapsedSeconds(snap.timing, now);
  const active = snap.job === 'running' || snap.job === 'paused';
  const remaining = active ? remainingSeconds(snap.fraction, snap.total, elapsed) : 0;
  const percent = Math.round(snap.fraction * 100);
  const state = stateText(snap, snap.job, routeReady);
  // La región en vivo solo cambia cada 10 % y con el estado: no se anuncia cada cuadro.
  const bucket = snap.ended === 'finished' ? 100 : Math.floor(percent / 10) * 10;
  const announce = useMemo(() => t('live.canvas.aria', { percent: bucket, state, source: '' }).trim(), [bucket, state]);
  const note = routeNote(snap.route);
  // «Láser encendido/apagado» solo tiene sentido mientras el trabajo corre o está en pausa.
  const laser = active ? (snap.emitting ? t('live.laser.on') : t('live.laser.off')) : null;
  return (
    <div className="live-foot" data-testid="live-hud">
      {note ? (
        <p className="live-foot__note">{note}</p>
      ) : (
        <>
          <div className="live-foot__main num" aria-hidden="true">
            <span className="live-foot__pct">{t('live.hud.percent', { percent })}</span>
            <span className="live-foot__state">
              {state}
              {laser && <span className={snap.emitting ? 'live-foot__laser live-foot__laser--on' : 'live-foot__laser'}> · {laser}</span>}
            </span>
          </div>
          <dl className="live-foot__times num" aria-hidden="true">
            <div>
              <dt>{t('live.hud.elapsed')}</dt>
              <dd>{fmtClock(elapsed)}</dd>
            </div>
            <div>
              <dt>{t('live.hud.remaining')}</dt>
              <dd>{remaining === null ? '—' : fmtClock(remaining)}</dd>
            </div>
          </dl>
        </>
      )}
      {children}
      <p className="sr-only" role="status" aria-live="polite">{announce}{laser ? `, ${laser}` : ''}</p>
    </div>
  );
}

/**
 * Contenido de la vista en vivo: la cama simulada (encajada en el espacio disponible, con zoom y
 * desplazamiento propios: rueda y arrastre; teclado: + - 0 y flechas), el botón «encajar» y la franja
 * con porcentaje, estado, transcurrido y restante. La usan la ventana flotante y la ventana aparte;
 * `source` dice de dónde salen los datos.
 */
export function LivePanel({ source, actions }: { source: LiveSource; actions?: ReactNode }) {
  const snap = useLiveSnapshot(source);
  const boxRef = useRef<HTMLDivElement>(null);
  const size = useBoxSize(boxRef, FALLBACK_SIZE);
  const [view, setView] = useState<ViewTransform>(() => fitBed(snap.bed, size));
  const [fitted, setFitted] = useState(true);
  const drag = useRef<{ id: number; x: number; y: number } | null>(null);
  const hintId = useId();
  const [bx, by] = snap.bed;

  // Mientras no se haya tocado el zoom, la cama sigue encajada al tamaño de la ventana.
  useEffect(() => {
    if (fitted) setView(fitBed([bx, by], size));
  }, [fitted, size, bx, by]);

  const fit = useCallback(() => {
    setFitted(true);
    setView(fitBed([bx, by], size));
  }, [bx, by, size]);
  const zoomBy = useCallback((factor: number, sx: number, sy: number) => {
    setFitted(false);
    setView((v) => zoomAround(v, factor, sx, sy));
  }, []);

  // La rueda se escucha aparte para poder evitar que se desplace la página (los eventos de React son pasivos).
  useEffect(() => {
    const el = boxRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const r = el.getBoundingClientRect();
      zoomBy(Math.exp(-e.deltaY * (e.deltaMode === 1 ? 0.05 : 0.0018)), e.clientX - r.left, e.clientY - r.top);
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  }, [zoomBy]);

  const down = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || (e.target as Element).closest('button')) return;
    e.currentTarget.setPointerCapture?.(e.pointerId);
    drag.current = { id: e.pointerId, x: e.clientX, y: e.clientY };
  };
  const move = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d || d.id !== e.pointerId) return;
    const dx = e.clientX - d.x;
    const dy = e.clientY - d.y;
    drag.current = { ...d, x: e.clientX, y: e.clientY };
    setFitted(false);
    setView((v) => ({ ...v, panX: v.panX + dx, panY: v.panY + dy }));
  };
  const up = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (drag.current?.id === e.pointerId) drag.current = null;
  };
  const keys = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.altKey || e.ctrlKey || e.metaKey) return;
    const cx = size.w / 2;
    const cy = size.h / 2;
    const pan = (dx: number, dy: number) => {
      setFitted(false);
      setView((v) => ({ ...v, panX: v.panX + dx, panY: v.panY + dy }));
    };
    switch (e.key) {
      case '+':
      case '=':
        zoomBy(1.25, cx, cy);
        break;
      case '-':
      case '_':
        zoomBy(0.8, cx, cy);
        break;
      case '0':
        fit();
        break;
      case 'ArrowLeft':
        pan(KEY_PAN, 0);
        break;
      case 'ArrowRight':
        pan(-KEY_PAN, 0);
        break;
      case 'ArrowUp':
        pan(0, KEY_PAN);
        break;
      case 'ArrowDown':
        pan(0, -KEY_PAN);
        break;
      default:
        return;
    }
    // Estas teclas son de la cama simulada: no deben llegar a los atajos del lienzo del diseño.
    e.preventDefault();
    e.stopPropagation();
  };

  const routeReady = snap.route === 'ready';
  const label = liveSummary(snap, snap.job, routeReady);
  const state: CanvasState = { view: { zoom: clampZoom(view.zoom), panX: view.panX, panY: view.panY }, size, snap };

  return (
    <div className="live-panel">
      {/* Cama simulada: zoom propio (rueda, + y -), desplazamiento (arrastre, flechas) y 0 para encajar. */}
      <div
        ref={boxRef}
        className="live-panel__view"
        role="group"
        aria-label={t('live.view.label')}
        aria-describedby={hintId}
        // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
        aria-keyshortcuts="+ - 0 ArrowUp ArrowDown ArrowLeft ArrowRight"
        onPointerDown={down}
        onPointerMove={move}
        onPointerUp={up}
        onPointerCancel={up}
        onKeyDown={keys}
        data-testid="live-view"
      >
        <span id={hintId} className="sr-only">{t('live.view.hint')}</span>
        <LiveCanvas source={source} state={state} jobSeq={snap.jobSeq} routeReady={routeReady} label={label} />
        <button type="button" className="icon-btn live-panel__fit" title={t('live.view.fitTip')} aria-label={t('live.view.fit')} onClick={fit}>
          <Maximize size={14} aria-hidden="true" />
        </button>
      </div>
      <LiveFoot snap={snap}>{actions}</LiveFoot>
    </div>
  );
}
