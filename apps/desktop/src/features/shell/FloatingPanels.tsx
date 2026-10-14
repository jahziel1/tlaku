import { PanelRightClose } from 'lucide-react';
import { useEffect, useId, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { t } from '@/i18n';
import { dockFloatingTab, fitFloatRect, FLOAT_MAX_H, FLOAT_MAX_W, FLOAT_MIN_H, FLOAT_MIN_W, setFloatRect, shownFloating, type FloatRect, type FloatingPanel } from '@/lib/dockLayout';
import { usePref, useStore } from '@/store';
import type { DockTab } from '@/store/view';
import { PanelBody, TAB_BY_ID } from './dockPanels';

/** Paso, en píxeles, del teclado al mover (flechas) o cambiar el tamaño (Mayús + flechas). */
const KEY_STEP = 16;

type Gesture = { kind: 'move' | 'resize'; pointerId: number; x: number; y: number; from: FloatRect };

/** Tamaño de la ventana de la app: los paneles flotantes se mantienen dentro. */
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

/**
 * Una ventana flotante dentro de la app con el contenido de una pestaña. La barra de título la
 * arrastra; la esquina inferior derecha cambia su tamaño; «Acoplar» (o doble clic en el título) la
 * devuelve al panel. La posición y el tamaño se guardan en las preferencias al soltar.
 */
function FloatingWindow({ panel, bounds, z, onRaise }: { panel: FloatingPanel; bounds: { w: number; h: number }; z: number; onRaise: (id: DockTab) => void }) {
  const setPref = useStore((s) => s.setPref);
  const [live, setLive] = useState<FloatRect | null>(null);
  const gesture = useRef<Gesture | null>(null);
  const tab = TAB_BY_ID.get(panel.id)!;
  const rect = fitFloatRect(live ?? panel, bounds);

  const commit = (r: FloatRect) => {
    const layout = useStore.getState().prefs.dockLayout;
    setPref('dockLayout', setFloatRect(layout, panel.id, fitFloatRect(r, bounds)));
  };
  const begin = (kind: Gesture['kind']) => (e: PointerEvent<HTMLElement>) => {
    if (e.button !== 0 || (e.target as Element).closest('button')) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    gesture.current = { kind, pointerId: e.pointerId, x: e.clientX, y: e.clientY, from: rect };
    onRaise(panel.id);
  };
  const move = (e: PointerEvent<HTMLElement>) => {
    const g = gesture.current;
    if (!g || g.pointerId !== e.pointerId) return;
    const dx = e.clientX - g.x;
    const dy = e.clientY - g.y;
    if (g.kind === 'move') setLive(fitFloatRect({ ...g.from, x: g.from.x + dx, y: g.from.y + dy }, bounds));
    else setLive({ ...g.from, w: Math.min(FLOAT_MAX_W, Math.max(FLOAT_MIN_W, g.from.w + dx)), h: Math.min(FLOAT_MAX_H, Math.max(FLOAT_MIN_H, g.from.h + dy)) });
  };
  const end = (e: PointerEvent<HTMLElement>) => {
    const g = gesture.current;
    if (!g || g.pointerId !== e.pointerId) return;
    gesture.current = null;
    if (live) commit(live);
    setLive(null);
  };
  /** Flechas: mueven; con Mayús (o en el asa de la esquina) cambian el tamaño. */
  const keys = (resizeOnly: boolean) => (e: KeyboardEvent<HTMLElement>) => {
    if (e.altKey || e.ctrlKey || e.metaKey) return;
    const dx = e.key === 'ArrowLeft' ? -KEY_STEP : e.key === 'ArrowRight' ? KEY_STEP : 0;
    const dy = e.key === 'ArrowUp' ? -KEY_STEP : e.key === 'ArrowDown' ? KEY_STEP : 0;
    if (dx === 0 && dy === 0) return;
    e.preventDefault();
    commit(resizeOnly || e.shiftKey ? { ...rect, w: rect.w + dx, h: rect.h + dy } : { ...rect, x: rect.x + dx, y: rect.y + dy });
  };
  const dockBack = () => setPref('dockLayout', dockFloatingTab(useStore.getState().prefs.dockLayout, panel.id));
  const Icon = tab.icon;
  const hintId = useId();

  return (
    <section
      className="floatwin"
      role="dialog"
      aria-modal="false"
      aria-label={t(tab.label)}
      style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h, zIndex: z }}
      onPointerDownCapture={() => onRaise(panel.id)}
    >
      {/* Asa de teclado: la cabecera se enfoca y las flechas la mueven (Mayús+flechas cambian el tamaño). */}
      {/* eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions */}
      <header
        className="floatwin__head"
        role="group"
        aria-label={t('a11y.float.handle', { name: t(tab.label) })}
        aria-describedby={`${hintId}`}
        // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
        aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight Shift+ArrowUp Shift+ArrowDown Shift+ArrowLeft Shift+ArrowRight"
        title={t('prefs.dock.moveHint')}
        onPointerDown={begin('move')}
        onPointerMove={move}
        onPointerUp={end}
        onPointerCancel={end}
        onKeyDown={keys(false)}
        onDoubleClick={(e) => !(e.target as Element).closest('button') && dockBack()}
      >
        <Icon size={14} aria-hidden="true" />
        <span id={hintId} className="sr-only">{t('prefs.dock.moveHint')}</span>
        <span className="floatwin__title">{t(tab.label)}</span>
        <button type="button" className="icon-btn" title={t('prefs.dock.dockBack')} aria-label={`${t('prefs.dock.dockBack')}: ${t(tab.label)}`} onClick={dockBack}>
          <PanelRightClose size={14} aria-hidden="true" />
        </button>
      </header>
      <div className="floatwin__body">
        <PanelBody id={panel.id} />
      </div>
      <div
        className="floatwin__grip"
        role="separator"
        aria-label={t('prefs.dock.resizeFloat')}
        aria-valuemin={FLOAT_MIN_W}
        aria-valuemax={FLOAT_MAX_W}
        aria-valuenow={Math.round(rect.w)}
        aria-valuetext={`${Math.round(rect.w)} × ${Math.round(rect.h)} px`}
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
    </section>
  );
}

/** Los paneles que se sacaron del costado: cada uno en su ventana flotante, sobre el lienzo. */
export function FloatingPanels() {
  const layout = usePref('dockLayout');
  const tab = useStore((s) => s.dockTab);
  const bounds = useWindowSize();
  // Orden de apilado: el último de la lista queda encima.
  const [stack, setStack] = useState<DockTab[]>([]);
  const raise = (id: DockTab) => setStack((cur) => (cur[cur.length - 1] === id ? cur : [...cur.filter((x) => x !== id), id]));
  const panels = shownFloating(layout);
  // Abrir una pestaña que está flotando (atajo, menú) la trae al frente.
  useEffect(() => {
    if (panels.some((p) => p.id === tab)) raise(tab);
    // Solo importa cuándo cambia la pestaña elegida o la lista de paneles.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab, panels.length]);
  if (panels.length === 0) return null;
  return (
    <>
      {panels.map((p) => (
        <FloatingWindow key={p.id} panel={p} bounds={bounds} z={30 + Math.max(0, stack.indexOf(p.id))} onRaise={raise} />
      ))}
    </>
  );
}
