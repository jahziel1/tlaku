import { AppWindow, Maximize2, Minimize2, X } from 'lucide-react';
import { useEffect, useId, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { t } from '@/i18n';
import { defaultLiveRect, fitLiveRect, LIVE_MIN_H, LIVE_MIN_W, maxLiveRect, resizeLiveRect, type LiveRect, type LiveWindowPref } from '@/lib/liveWindow';
import { usePref, useStore } from '@/store';
import { openDetached, useLiveWindow } from './detach';
import { LivePanel } from './LivePanel';
import { localSource } from './localSource';
import './live.css';

/** Paso, en píxeles, del teclado al mover (flechas) o cambiar el tamaño (Mayús + flechas). */
const KEY_STEP = 16;
const MAX_ARIA = 3000;

type Gesture = { kind: 'move' | 'resize'; pointerId: number; x: number; y: number; from: LiveRect };

/** Tamaño de la ventana de la app: la flotante se mantiene dentro. */
function useWindowSize(): { w: number; h: number } {
  const read = () => ({ w: typeof window === 'undefined' ? 1440 : window.innerWidth, h: typeof window === 'undefined' ? 900 : window.innerHeight });
  const [size, setSize] = useState(read);
  useEffect(() => {
    const on = () => setSize(read());
    window.addEventListener('resize', on);
    return () => window.removeEventListener('resize', on);
  }, []);
  return size;
}

/** El área del lienzo del diseño (donde aparece y hasta donde se agranda); si no se encuentra, toda la ventana. */
function areaRect(bounds: { w: number; h: number }): LiveRect {
  const el = typeof document === 'undefined' ? null : document.querySelector<HTMLElement>('.canvas-area');
  const r = el?.getBoundingClientRect();
  if (r && r.width > 0 && r.height > 0) return { x: Math.round(r.left), y: Math.round(r.top), w: Math.round(r.width), h: Math.round(r.height) };
  return { x: 0, y: 0, w: bounds.w, h: bounds.h };
}

/**
 * Ventana flotante de la vista en vivo («imagen en imagen»): la cama simulada completa, el porcentaje,
 * el estado y los tiempos en una ventana propia dentro de la app. NO va sobre el lienzo del diseño ni lo
 * modifica: se puede mover (barra de título) y cambiar de tamaño (esquina) a cualquier lugar de la
 * ventana, agrandar casi a toda el área, abrir en una ventana del sistema o cerrar (= apagar la vista).
 * La posición y el tamaño se recuerdan (preferencia `liveWindow`). Misma mecánica y estilos que los
 * paneles flotantes (`shell/FloatingPanels.tsx`).
 */
function FloatingLive() {
  const stored = usePref('liveWindow');
  const setPref = useStore((s) => s.setPref);
  const bounds = useWindowSize();
  const [live, setLive] = useState<LiveRect | null>(null);
  // El último rectángulo del gesto, para guardarlo al soltar aunque el cuadro de React no haya llegado todavía.
  const latest = useRef<LiveRect | null>(null);
  const gesture = useRef<Gesture | null>(null);
  const hintId = useId();
  const area = areaRect(bounds);
  const maximized = stored?.max ?? false;
  const normal = fitLiveRect(stored ?? defaultLiveRect(area), bounds);
  const rect = maximized ? fitLiveRect(maxLiveRect(area), bounds) : fitLiveRect(live ?? normal, bounds);

  const commit = (r: LiveRect, max = false) => {
    const fitted = fitLiveRect(r, bounds);
    const next: LiveWindowPref = { ...fitted, max };
    const cur = useStore.getState().prefs.liveWindow;
    if (!cur || cur.x !== next.x || cur.y !== next.y || cur.w !== next.w || cur.h !== next.h || cur.max !== next.max) setPref('liveWindow', next);
  };
  const toggleMax = () => commit(normal, !maximized);
  const begin = (kind: Gesture['kind']) => (e: PointerEvent<HTMLElement>) => {
    if (e.button !== 0 || maximized || (e.target as Element).closest('button')) return;
    e.currentTarget.setPointerCapture?.(e.pointerId);
    gesture.current = { kind, pointerId: e.pointerId, x: e.clientX, y: e.clientY, from: rect };
  };
  const move = (e: PointerEvent<HTMLElement>) => {
    const g = gesture.current;
    if (!g || g.pointerId !== e.pointerId) return;
    const dx = e.clientX - g.x;
    const dy = e.clientY - g.y;
    const next = g.kind === 'move' ? fitLiveRect({ ...g.from, x: g.from.x + dx, y: g.from.y + dy }, bounds) : resizeLiveRect(g.from, dx, dy, bounds);
    latest.current = next;
    setLive(next);
  };
  const end = (e: PointerEvent<HTMLElement>) => {
    const g = gesture.current;
    if (!g || g.pointerId !== e.pointerId) return;
    gesture.current = null;
    if (latest.current) commit(latest.current);
    latest.current = null;
    setLive(null);
  };
  /** Flechas: mueven; con Mayús (o en el asa de la esquina) cambian el tamaño. */
  const keys = (resizeOnly: boolean) => (e: KeyboardEvent<HTMLElement>) => {
    if (e.altKey || e.ctrlKey || e.metaKey || maximized) return;
    const dx = e.key === 'ArrowLeft' ? -KEY_STEP : e.key === 'ArrowRight' ? KEY_STEP : 0;
    const dy = e.key === 'ArrowUp' ? -KEY_STEP : e.key === 'ArrowDown' ? KEY_STEP : 0;
    if (dx === 0 && dy === 0) return;
    e.preventDefault();
    e.stopPropagation();
    commit(resizeOnly || e.shiftKey ? resizeLiveRect(rect, dx, dy, bounds) : { ...rect, x: rect.x + dx, y: rect.y + dy });
  };
  const close = () => setPref('liveView', false);

  return (
    <section className="floatwin livewin" role="dialog" aria-modal="false" aria-label={t('live.hud.title')} style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h, zIndex: 28 }} data-testid="live-window" data-maximized={maximized ? 'true' : 'false'}>
      {/* Asa de teclado: la cabecera se enfoca y las flechas la mueven (Mayús+flechas cambian el tamaño). */}
      {/* eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions */}
      <header
        className="floatwin__head"
        role="group"
        aria-label={t('live.win.handle')}
        aria-describedby={hintId}
        // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
        aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight Shift+ArrowUp Shift+ArrowDown Shift+ArrowLeft Shift+ArrowRight"
        title={t('prefs.dock.moveHint')}
        onPointerDown={begin('move')}
        onPointerMove={move}
        onPointerUp={end}
        onPointerCancel={end}
        onKeyDown={keys(false)}
        onDoubleClick={(e) => !(e.target as Element).closest('button') && toggleMax()}
        data-testid="live-window-head"
      >
        <span id={hintId} className="sr-only">{t('prefs.dock.moveHint')}</span>
        <span className="floatwin__title">{t('live.hud.title')}</span>
        <button type="button" className="icon-btn" title={t('live.win.detachTip')} aria-label={t('live.win.detach')} onClick={() => void openDetached()}>
          <AppWindow size={14} aria-hidden="true" />
        </button>
        <button type="button" className="icon-btn" title={maximized ? t('live.win.restore') : t('live.win.maximize')} aria-label={maximized ? t('live.win.restore') : t('live.win.maximize')} aria-pressed={maximized} onClick={toggleMax}>
          {maximized ? <Minimize2 size={14} aria-hidden="true" /> : <Maximize2 size={14} aria-hidden="true" />}
        </button>
        <button type="button" className="icon-btn" title={t('live.hud.hideTip')} aria-label={t('live.hud.hide')} onClick={close}>
          <X size={14} aria-hidden="true" />
        </button>
      </header>
      <div className="floatwin__body">
        <LivePanel source={localSource()} />
      </div>
      {!maximized && (
        <div
          className="floatwin__grip"
          role="separator"
          aria-label={t('live.win.resize')}
          aria-valuemin={LIVE_MIN_W}
          aria-valuemax={MAX_ARIA}
          aria-valuenow={Math.round(rect.w)}
          aria-valuetext={`${Math.round(rect.w)} × ${Math.round(rect.h)} px (${LIVE_MIN_W} × ${LIVE_MIN_H} px min)`}
          aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight"
          // Separador enfocable (patrón de ventana divisoria): las flechas cambian el tamaño.
          // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
          tabIndex={0}
          onPointerDown={begin('resize')}
          onPointerMove={move}
          onPointerUp={end}
          onPointerCancel={end}
          onKeyDown={keys(true)}
        />
      )}
    </section>
  );
}

/**
 * La ventana flotante de la vista en vivo. Con la vista apagada no monta nada; mientras la ventana
 * aparte está abierta se oculta (y vuelve al cerrarla si la vista sigue encendida).
 */
export function LiveWindow() {
  const enabled = usePref('liveView');
  const detached = useLiveWindow((s) => s.detached);
  if (!enabled || detached) return null;
  return <FloatingLive />;
}
