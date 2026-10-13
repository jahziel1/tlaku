import { ArrowDown, ArrowUp, Camera, Cpu, History, Layers, Library, SlidersHorizontal, StickyNote, Terminal } from 'lucide-react';
import { useEffect, useLayoutEffect, useRef, useState, type DragEvent, type KeyboardEvent, type PointerEvent, type RefObject } from 'react';
import { useCommand } from '@/commands/useCommand';
import { CameraPanel } from '@/features/camera';
import { HistoryPanel } from '@/features/history';
import { ConsolePanel, MachinePanel } from '@/features/machine';
import { LayersPanel } from '@/features/layers';
import { MaterialsPanel } from '@/features/materials';
import { t, type I18nKey } from '@/i18n';
import { DEFAULT_DOCK_LAYOUT, moveDockTab, nudgeDockTab, setDockTabHidden, visibleTabs } from '@/lib/dockLayout';
import { usePref, useStore } from '@/store';
import { DOCK_MAX, DOCK_MIN, type DockTab } from '@/store/view';
import { HISTORY_TAB } from './dockTabs';
import { NotesPanel } from './NotesPanel';

const TABS: { id: DockTab; icon: typeof Layers; cmd: string; label: I18nKey }[] = [
  { id: 'layers', icon: Layers, cmd: 'panel.layers', label: 'dock.layers' },
  { id: 'machine', icon: Cpu, cmd: 'panel.machine', label: 'dock.machine' },
  { id: 'console', icon: Terminal, cmd: 'panel.console', label: 'dock.console' },
  { id: HISTORY_TAB, icon: History, cmd: 'panel.history', label: 'history.dock.title' },
  { id: 'camera', icon: Camera, cmd: 'panel.camera', label: 'dock.camera' },
  { id: 'materials', icon: Library, cmd: 'panel.materials', label: 'dock.materials' },
  { id: 'notes', icon: StickyNote, cmd: 'panel.notes', label: 'canvas.dock.notes' },
];
const TAB_BY_ID = new Map(TABS.map((x) => [x.id, x]));

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

function Tab({ id, icon: Icon, cmd, label, onKey, compact, dnd }: (typeof TABS)[number] & { onKey: (e: KeyboardEvent, id: DockTab) => void; compact: boolean; dnd: TabDnd }) {
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

/**
 * Menú para mostrar u ocultar cada pestaña y cambiar su orden con flechas (lo mismo que arrastrar
 * las pestañas, para teclado y para quien no use el ratón). Se cierra con Esc o al hacer clic fuera.
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
        }
      }}
    >
      <p className="dock__menu-title">{t('canvas.dock.customize')}</p>
      <ul className="dock__menu-list">
        {layout.order.map((id) => {
          const tab = TAB_BY_ID.get(id)!;
          const isShown = shown.includes(id);
          const i = shown.indexOf(id);
          return (
            <li key={id} className="dock__menu-row">
              <label className="dock__menu-check">
                <input
                  type="checkbox"
                  checked={isShown}
                  disabled={isShown && shown.length <= 1}
                  onChange={(e) => hide(id, !e.target.checked)}
                />
                <span>{t(tab.label)}</span>
              </label>
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
      <p className="hint">{t('canvas.dock.hint')}</p>
      <button type="button" className="btn" onClick={() => setPref('dockLayout', DEFAULT_DOCK_LAYOUT)}>
        {t('canvas.dock.reset')}
      </button>
    </div>
  );
}

/**
 * Panel acoplado a la derecha con pestañas (Capas, Máquina, Consola, Historial, Cámara, Materiales y
 * Notas). Las pestañas se reordenan arrastrándolas (o con Alt+Mayús+flechas) y se pueden mostrar u
 * ocultar desde el botón de la derecha; el orden y lo oculto se guardan en las preferencias.
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

  const shown = visibleTabs(layout);
  const list = shown.map((id) => TAB_BY_ID.get(id)!);
  // Una pestaña oculta (por las preferencias) no puede estar abierta.
  const active: DockTab = shown.includes(tab) ? tab : (shown[0] ?? 'layers');
  useEffect(() => {
    if (active !== tab) setTab(active);
  }, [active, tab, setTab]);

  const onTabKey = (e: KeyboardEvent, id: DockTab) => {
    if (e.altKey && e.shiftKey && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      e.preventDefault();
      setPref('dockLayout', nudgeDockTab(layout, id, e.key === 'ArrowLeft' ? -1 : 1));
      return;
    }
    const i = shown.indexOf(active);
    const next = e.key === 'ArrowRight' ? i + 1 : e.key === 'ArrowLeft' ? i - 1 : null;
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

  const onGripDown = (e: PointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, w: width };
  };
  const onGripMove = (e: PointerEvent<HTMLDivElement>) => {
    if (drag.current) setWidth(drag.current.w + (drag.current.x - e.clientX));
  };

  return (
    <aside className="dock" style={{ width }} aria-label={t('dock.label')}>
      <div
        className="dock__grip"
        role="separator"
        aria-orientation="vertical"
        aria-label={t('dock.resize')}
        aria-valuemin={DOCK_MIN}
        aria-valuemax={DOCK_MAX}
        aria-valuenow={width}
        tabIndex={0}
        onPointerDown={onGripDown}
        onPointerMove={onGripMove}
        onPointerUp={() => (drag.current = null)}
        onKeyDown={(e) => {
          if (e.key === 'ArrowLeft') setWidth(width + 16);
          if (e.key === 'ArrowRight') setWidth(width - 16);
        }}
      />
      <div className="dock__tabbar">
        <div ref={listRef} role="tablist" aria-label={t('dock.label')} className="tabs">
          {list.map((x) => (
            <Tab key={x.id} {...x} onKey={onTabKey} compact={compact} dnd={dnd} />
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
        {active === 'layers' && <LayersPanel />}
        {active === 'machine' && <MachinePanel />}
        {active === 'console' && <ConsolePanel />}
        {active === HISTORY_TAB && <HistoryPanel />}
        {active === 'camera' && <CameraPanel />}
        {active === 'materials' && <MaterialsPanel />}
        {active === 'notes' && <NotesPanel />}
      </div>
    </aside>
  );
}
