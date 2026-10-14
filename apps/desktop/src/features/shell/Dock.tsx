import { AppWindow, ArrowDown, ArrowUp, PanelLeft, PanelRight, SlidersHorizontal } from 'lucide-react';
import { useEffect, useLayoutEffect, useRef, useState, type DragEvent, type KeyboardEvent, type PointerEvent, type RefObject } from 'react';
import { useCommand } from '@/commands/useCommand';
import { t, type I18nKey } from '@/i18n';
import { DEFAULT_DOCK_LAYOUT, dockFloatingTab, floatDockTab, isFloating, moveDockTab, nudgeDockTab, setDockSide, setDockTabHidden, setToolsSide, visibleTabs } from '@/lib/dockLayout';
import { usePref, useStore } from '@/store';
import { DOCK_MAX, DOCK_MIN, type DockTab } from '@/store/view';
import { PanelBody, TAB_BY_ID, type DockTabDef } from './dockPanels';

/** Qué pestaña se arrastra y sobre cuál va a caer (antes o después de ella). */
interface DropState {
  over: DockTab;
  side: 'before' | 'after';
}

interface TabDnd {
  dragging: DockTab | null;
  drop: DropState | null;
  start: (id: DockTab, e: DragEvent) => void;
  over: (id: DockTab, e: DragEvent) => void;
  drop_: (id: DockTab, e: DragEvent) => void;
  end: () => void;
}

function Tab({ id, icon: Icon, cmd, label, onKey, compact, dnd, onFloat }: DockTabDef & { onFloat: (id: DockTab) => void; onKey: (e: KeyboardEvent, id: DockTab) => void; compact: boolean; dnd: TabDnd }) {
  const c = useCommand(cmd);
  return (
    <button
      type="button"
      role="tab"
      id={`tab-${id}`}
      draggable
      data-dragging={dnd.dragging === id || undefined}
      data-drop={dnd.drop?.over === id ? dnd.drop.side : undefined}
      onDragStart={(e) => dnd.start(id, e)}
      onDragOver={(e) => dnd.over(id, e)}
      onDrop={(e) => dnd.drop_(id, e)}
      onDragEnd={dnd.end}
      aria-selected={c.checked}
      aria-controls={`tabpanel-${id}`}
      tabIndex={c.checked ? 0 : -1}
      className={compact ? 'tab tab--icon' : 'tab'}
      title={c.tooltip}
      // Sin texto visible, el nombre accesible sale de aquí (y el tooltip de `title`).
      aria-label={compact ? t(label) : undefined}
      onClick={c.run}
      onDoubleClick={() => onFloat(id)}
      onKeyDown={(e) => onKey(e, id)}
    >
      <Icon size={15} aria-hidden="true" />
      {!compact && t(label)}
    </button>
  );
}

/**
 * ¿Las pestañas con texto caben en el ancho disponible? Si no, pasan a solo ícono. Se mide el
 * desborde real (la tipografía y los idiomas cambian el ancho) y se recuerda cuánto ocupaban con
 * texto para volver a mostrarlo en cuanto el panel se ensancha.
 */
function useCompactTabs(list: RefObject<HTMLDivElement | null>): boolean {
  const [compact, setCompact] = useState(false);
  const needed = useRef(0);
  useLayoutEffect(() => {
    const el = list.current;
    if (!el) return;
    const fit = () => {
      if (!compact) {
        if (el.scrollWidth > el.clientWidth) {
          needed.current = el.scrollWidth + parseFloat(getComputedStyle(el).paddingRight);
          setCompact(true);
        }
      } else if (el.clientWidth >= needed.current) {
        setCompact(false);
      }
    };
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, [compact, list]);
  return compact;
}

/** Saca una pestaña del panel a una ventana flotante y la deja abierta (si no es la última acoplada). */
function floatTab(id: DockTab): void {
  const s = useStore.getState();
  const next = floatDockTab(s.prefs.dockLayout, id);
  if (next === s.prefs.dockLayout) return;
  s.setPref('dockLayout', next);
  s.setDockTab(id);
}

/**
 * Menú para personalizar la distribución: mostrar u ocultar cada pestaña, cambiar su orden con
 * flechas (lo mismo que arrastrar las pestañas, para teclado y para quien no use el ratón), sacarla
 * a una ventana flotante o devolverla, y elegir el borde del panel y de las herramientas. Se cierra
 * con Esc o al hacer clic fuera.
 */
function TabsMenu({ onClose }: { onClose: () => void }) {
  const layout = usePref('dockLayout');
  const setPref = useStore((s) => s.setPref);
  const ref = useRef<HTMLDivElement>(null);
  const shown = visibleTabs(layout);

  useEffect(() => {
    const down = (e: globalThis.PointerEvent) => {
      if (!ref.current?.contains(e.target as Node) && !(e.target as Element).closest?.('[data-tabs-menu-button]')) onClose();
    };
    window.addEventListener('pointerdown', down, true);
    return () => window.removeEventListener('pointerdown', down, true);
  }, [onClose]);

  // Al abrir, el foco entra al menú; con Esc se cierra y el foco vuelve al botón que lo abrió.
  useEffect(() => {
    ref.current?.querySelector<HTMLElement>('input:not([disabled]), button:not([disabled])')?.focus();
  }, []);

  const hide = (id: DockTab, hidden: boolean) => {
    const next = setDockTabHidden(layout, id, hidden);
    setPref('dockLayout', next);
    // Si se oculta la pestaña abierta, pasa a la primera visible.
    const s = useStore.getState();
    if (hidden && s.dockTab === id) s.setDockTab(visibleTabs(next)[0] ?? 'layers');
  };

  return (
    <div
      ref={ref}
      className="dock__menu"
      role="group"
      aria-label={t('canvas.dock.customize')}
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          onClose();
          document.querySelector<HTMLElement>('[data-tabs-menu-button]')?.focus();
        }
      }}
    >
      <p className="dock__menu-title">{t('canvas.dock.customize')}</p>
      <ul className="dock__menu-list">
        {layout.order.map((id) => {
          const tab = TAB_BY_ID.get(id)!;
          const floating = isFloating(layout, id);
          const isShown = shown.includes(id);
          const checked = isShown || (floating && !layout.hidden.includes(id));
          const i = shown.indexOf(id);
          const canFloat = floating || (isShown && shown.length > 1);
          const floatLabel = floating ? t('prefs.dock.dockBack') : t('prefs.dock.float');
          return (
            <li key={id} className="dock__menu-row">
              <label className="dock__menu-check">
                <input
                  type="checkbox"
                  checked={checked}
                  disabled={isShown && shown.length <= 1}
                  onChange={(e) => hide(id, !e.target.checked)}
                />
                <span>{t(tab.label)}</span>
              </label>
              <button
                type="button"
                className="icon-btn"
                aria-pressed={floating}
                disabled={!canFloat || (!floating && !isShown)}
                title={floatLabel}
                aria-label={`${floatLabel}: ${t(tab.label)}`}
                onClick={() => (floating ? setPref('dockLayout', dockFloatingTab(layout, id)) : floatTab(id))}
              >
                <AppWindow size={14} aria-hidden="true" />
              </button>
              <button type="button" className="icon-btn" disabled={!isShown || i <= 0} title={t('canvas.dock.moveUp')} aria-label={`${t('canvas.dock.moveUp')}: ${t(tab.label)}`} onClick={() => setPref('dockLayout', nudgeDockTab(layout, id, -1))}>
                <ArrowUp size={14} aria-hidden="true" />
              </button>
              <button type="button" className="icon-btn" disabled={!isShown || i < 0 || i >= shown.length - 1} title={t('canvas.dock.moveDown')} aria-label={`${t('canvas.dock.moveDown')}: ${t(tab.label)}`} onClick={() => setPref('dockLayout', nudgeDockTab(layout, id, 1))}>
                <ArrowDown size={14} aria-hidden="true" />
              </button>
            </li>
          );
        })}
      </ul>
      <SideChoice label="prefs.dock.panelSide" value={layout.side} onChange={(side) => setPref('dockLayout', setDockSide(layout, side))} />
      <SideChoice label="prefs.dock.toolsSide" value={layout.toolsSide} onChange={(side) => setPref('dockLayout', setToolsSide(layout, side))} />
      <p className="hint">{t('canvas.dock.hint')} {t('prefs.dock.hintFloat')}</p>
      <button type="button" className="btn" onClick={() => setPref('dockLayout', DEFAULT_DOCK_LAYOUT)}>
        {t('canvas.dock.reset')}
      </button>
    </div>
  );
}

/** Elegir entre el borde izquierdo y el derecho. */
function SideChoice({ label, value, onChange }: { label: I18nKey; value: 'left' | 'right'; onChange: (side: 'left' | 'right') => void }) {
  return (
    <div className="dock__side" role="radiogroup" aria-label={t(label)}>
      <span className="dock__side-label">{t(label)}</span>
      {(['left', 'right'] as const).map((side) => {
        const Icon = side === 'left' ? PanelLeft : PanelRight;
        const text = t(side === 'left' ? 'prefs.dock.left' : 'prefs.dock.right');
        return (
          <button key={side} type="button" role="radio" aria-checked={value === side} className="icon-btn" title={text} aria-label={`${t(label)}: ${text}`} onClick={() => onChange(side)}>
            <Icon size={14} aria-hidden="true" />
          </button>
        );
      })}
    </div>
  );
}

/**
 * Panel acoplado al borde izquierdo o derecho con pestañas (Capas, Máquina, Consola, Historial,
 * Cámara, Materiales y Notas). Las pestañas se reordenan arrastrándolas (o con Alt+Mayús+flechas),
 * se pueden mostrar u ocultar desde el botón de personalizar y se sacan a una ventana flotante con
 * doble clic (o desde ese mismo menú); todo se guarda en las preferencias.
 */
export function Dock() {
  const tab = useStore((s) => s.dockTab);
  const setTab = useStore((s) => s.setDockTab);
  const width = useStore((s) => s.dockWidth);
  const setWidth = useStore((s) => s.setDockWidth);
  const layout = usePref('dockLayout');
  const setPref = useStore((s) => s.setPref);
  const drag = useRef<{ x: number; w: number } | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const compact = useCompactTabs(listRef);
  const [menu, setMenu] = useState(false);
  const [dragging, setDragging] = useState<DockTab | null>(null);
  const [drop, setDrop] = useState<DropState | null>(null);
  const leftSide = layout.side === 'left';

  const shown = visibleTabs(layout);
  const list = shown.map((id) => TAB_BY_ID.get(id)!);
  // Una pestaña oculta (por las preferencias) no puede estar abierta. Si la elegida está flotando,
  // se ve en su ventana y el panel muestra la primera pestaña acoplada.
  const active: DockTab = shown.includes(tab) ? tab : (shown[0] ?? 'layers');
  const tabFloating = isFloating(layout, tab) && !layout.hidden.includes(tab);
  useEffect(() => {
    if (active !== tab && !tabFloating) setTab(active);
  }, [active, tab, tabFloating, setTab]);

  const onTabKey = (e: KeyboardEvent, id: DockTab) => {
    if (e.altKey && e.shiftKey && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      e.preventDefault();
      setPref('dockLayout', nudgeDockTab(layout, id, e.key === 'ArrowLeft' ? -1 : 1));
      return;
    }
    const i = shown.indexOf(active);
    const next = e.key === 'ArrowRight' ? i + 1 : e.key === 'ArrowLeft' ? i - 1 : e.key === 'Home' ? 0 : e.key === 'End' ? list.length - 1 : null;
    if (next === null) return;
    e.preventDefault();
    const target = list[(next + list.length) % list.length]!;
    setTab(target.id);
    requestAnimationFrame(() => document.getElementById(`tab-${target.id}`)?.focus());
  };

  const dnd: TabDnd = {
    dragging,
    drop,
    start: (id, e) => {
      e.dataTransfer.effectAllowed = 'move';
      e.dataTransfer.setData('text/plain', id);
      setDragging(id);
    },
    over: (id, e) => {
      if (!dragging || dragging === id) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      // Hacia la derecha cae después de la pestaña; hacia la izquierda, antes (como mover una ficha).
      const side = shown.indexOf(dragging) < shown.indexOf(id) ? 'after' : 'before';
      if (drop?.over !== id || drop.side !== side) setDrop({ over: id, side });
    },
    drop_: (id, e) => {
      e.preventDefault();
      if (dragging && dragging !== id) {
        const before = shown.indexOf(dragging) < shown.indexOf(id) ? (layout.order[layout.order.indexOf(id) + 1] ?? null) : id;
        setPref('dockLayout', moveDockTab(layout, dragging, before));
      }
      setDragging(null);
      setDrop(null);
    },
    end: () => {
      setDragging(null);
      setDrop(null);
    },
  };

  // El borde que se arrastra para cambiar el ancho está del lado del lienzo.
  const dir = leftSide ? -1 : 1;
  const onGripDown = (e: PointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, w: width };
  };
  const onGripMove = (e: PointerEvent<HTMLDivElement>) => {
    if (drag.current) setWidth(drag.current.w + dir * (drag.current.x - e.clientX));
  };

  return (
    <aside className={leftSide ? 'dock dock--left' : 'dock'} style={{ width }} aria-label={t('dock.label')}>
      {/* Separador enfocable (patrón «ventana divisoria» de WAI-ARIA): flechas cambian el ancho. */}
      <div
        className="dock__grip"
        role="separator"
        aria-orientation="vertical"
        aria-label={t('dock.resize')}
        aria-valuemin={DOCK_MIN}
        aria-valuemax={DOCK_MAX}
        aria-valuenow={width}
        // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
        onPointerDown={onGripDown}
        onPointerMove={onGripMove}
        onPointerUp={() => (drag.current = null)}
        onKeyDown={(e) => {
          if (e.key === 'ArrowLeft') setWidth(width + 16 * dir);
          if (e.key === 'ArrowRight') setWidth(width - 16 * dir);
        }}
      />
      <div className="dock__tabbar">
        <div ref={listRef} role="tablist" aria-label={t('dock.label')} className="tabs">
          {list.map((x) => (
            <Tab key={x.id} {...x} onKey={onTabKey} compact={compact} dnd={dnd} onFloat={floatTab} />
          ))}
        </div>
        <button
          type="button"
          className="icon-btn dock__tabs-menu"
          data-tabs-menu-button
          aria-haspopup="true"
          aria-expanded={menu}
          title={t('canvas.dock.customize')}
          aria-label={t('canvas.dock.customize')}
          onClick={() => setMenu((v) => !v)}
        >
          <SlidersHorizontal size={14} aria-hidden="true" />
        </button>
        {menu && <TabsMenu onClose={() => setMenu(false)} />}
      </div>
      <div role="tabpanel" id={`tabpanel-${active}`} aria-labelledby={`tab-${active}`} className="dock__body">
        <PanelBody id={active} />
      </div>
    </aside>
  );
}
