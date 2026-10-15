import { Plus, Printer, X } from 'lucide-react';
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from 'react';
import { createPortal } from 'react-dom';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';
import { tabLabel, tabViews, useStore, type TabView } from '@/store';
import { closeOtherTabs, closeTab } from './actions';
import { sendTabToNewWindow } from './handoff';
import './tabs.css';

/** Id de la zona del diseño (el `tabpanel` que controla la pestaña activa). */
export const TAB_PANEL_ID = 'tabpanel-design';
const tabDomId = (id: string) => `tab-${id}`;

/** Menos de esto (px) y es un clic, no un arrastre. */
const DRAG_THRESHOLD = 5;

interface MenuState {
  id: string;
  x: number;
  y: number;
}

/**
 * Barra de pestañas de documentos, encima del lienzo. Patrón WAI-ARIA de pestañas con activación
 * automática: ←/→ mueven el foco y activan, Inicio/Fin van a los extremos, Supr cierra, la tecla de menú
 * (o Mayús+F10) abre el menú de la pestaña. Se reordenan arrastrando (con el puntero, no con arrastrar-y-soltar
 * de HTML: Tauri lo reserva para soltar archivos) o con las entradas «Mover» del menú.
 */
export function TabBar() {
  const tabs = useStore((s) => s.tabs);
  const activeTabId = useStore((s) => s.activeTabId);
  const filePath = useStore((s) => s.filePath);
  const dirty = useStore((s) => s.dirty);
  const jobTabId = useStore((s) => (s.jobState === 'running' || s.jobState === 'paused' ? (s.jobOrigin?.tabId ?? null) : null));
  const moveTab = useStore((s) => s.moveTab);
  const activateTab = useStore((s) => s.activateTab);
  const newTab = useCommand('tabs.new');
  const views = useMemo(() => tabViews({ tabs, activeTabId, filePath, dirty }), [tabs, activeTabId, filePath, dirty]);

  const listRef = useRef<HTMLDivElement>(null);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  // Tras un arrastre, el clic que sigue al soltar no debe activar nada.
  const justDragged = useRef(false);
  // Foco pendiente tras activar con el teclado.
  const focusAfter = useRef<string | null>(null);

  useLayoutEffect(() => {
    const id = focusAfter.current;
    if (!id) return;
    focusAfter.current = null;
    document.getElementById(tabDomId(id))?.focus();
  });

  // La pestaña activa siempre queda a la vista (la barra se desplaza si hay muchas).
  useEffect(() => {
    const el = document.getElementById(tabDomId(activeTabId));
    el?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
  }, [activeTabId, views.length]);

  const focusTab = useCallback(
    (id: string) => {
      focusAfter.current = id;
      activateTab(id);
    },
    [activateTab],
  );

  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>, v: TabView) => {
    if (e.altKey || e.ctrlKey || e.metaKey) return;
    const at = views.findIndex((x) => x.id === v.id);
    const go = (i: number) => {
      e.preventDefault();
      const target = views[(i + views.length) % views.length]!;
      focusTab(target.id);
    };
    if (e.key === 'ArrowRight') go(at + 1);
    else if (e.key === 'ArrowLeft') go(at - 1);
    else if (e.key === 'Home') go(0);
    else if (e.key === 'End') go(views.length - 1);
    else if (e.key === 'Delete') {
      e.preventDefault();
      void closeTab(v.id);
    } else if (e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)) {
      e.preventDefault();
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      setMenu({ id: v.id, x: r.left, y: r.bottom });
    }
  };

  // Arrastrar para reordenar: el puntero se sigue en la ventana; al pasar sobre otra pestaña se intercambia el lugar.
  const startDrag = (e: ReactPointerEvent, id: string) => {
    if (e.button !== 0 || views.length < 2) return;
    const startX = e.clientX;
    let active = false;
    const onMove = (ev: PointerEvent) => {
      if (!active && Math.abs(ev.clientX - startX) < DRAG_THRESHOLD) return;
      if (!active) {
        active = true;
        setDragging(id);
      }
      const slots = [...(listRef.current?.querySelectorAll<HTMLElement>('[data-tab-slot]') ?? [])];
      const over = slots.findIndex((el) => {
        const r = el.getBoundingClientRect();
        return ev.clientX >= r.left && ev.clientX <= r.right;
      });
      if (over >= 0) moveTab(id, over);
    };
    const end = () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', end);
      window.removeEventListener('pointercancel', end);
      if (active) {
        justDragged.current = true;
        setTimeout(() => (justDragged.current = false), 0);
      }
      setDragging(null);
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', end);
    window.addEventListener('pointercancel', end);
  };

  return (
    <div className="tabbar">
      <div ref={listRef} className="tabbar__list" role="tablist" aria-label={t('tabs.bar.label')} aria-orientation="horizontal">
        {views.map((v) => {
          // En la barra siempre lleva número («Sin título 1»), aunque sea la única pestaña.
          const name = tabLabel(v, true);
          const running = jobTabId === v.id;
          return (
            <div
              key={v.id}
              className="tab"
              role="presentation"
              data-tab-slot=""
              data-active={v.active || undefined}
              data-dragging={dragging === v.id || undefined}
              onContextMenu={(e) => {
                e.preventDefault();
                setMenu({ id: v.id, x: e.clientX, y: e.clientY });
              }}
              onAuxClick={(e) => {
                // Clic con la rueda cierra la pestaña (como en un navegador).
                if (e.button === 1) {
                  e.preventDefault();
                  void closeTab(v.id);
                }
              }}
            >
              <button
                type="button"
                role="tab"
                id={tabDomId(v.id)}
                className="tab__main"
                aria-selected={v.active}
                aria-controls={TAB_PANEL_ID}
                aria-keyshortcuts="Delete"
                tabIndex={v.active ? 0 : -1}
                title={v.filePath ?? name}
                onPointerDown={(e) => startDrag(e, v.id)}
                onClick={() => {
                  if (!justDragged.current) activateTab(v.id);
                }}
                onKeyDown={(e) => onKeyDown(e, v)}
              >
                {running && (
                  <span className="tab__job" title={t('tabs.tab.runningHint')}>
                    <Printer size={13} aria-hidden="true" />
                    <span className="sr-only">{t('tabs.tab.running')}</span>
                  </span>
                )}
                <span className="tab__name">{name}</span>
                {v.dirty && (
                  <>
                    <span className="tab__dirty" aria-hidden="true" />
                    <span className="sr-only">{t('tabs.tab.unsaved')}</span>
                  </>
                )}
                {/* Cerrar con el puntero. No es un botón de verdad: un botón dentro de una pestaña (role="tab") rompe el
                    patrón ARIA. Con el teclado se cierra con Supr o con Cmd/Ctrl+W, y también desde el menú de la pestaña. */}
                <span
                  className="tab__close"
                  aria-hidden="true"
                  title={t('tabs.cmd.close')}
                  onPointerDown={(e) => e.stopPropagation()}
                  onClick={(e) => {
                    e.stopPropagation();
                    void closeTab(v.id);
                  }}
                >
                  <X size={13} aria-hidden="true" />
                </span>
              </button>
            </div>
          );
        })}
      </div>
      <button type="button" className="tabbar__new" aria-label={t('tabs.bar.new')} title={newTab.tooltip} onClick={newTab.run}>
        <Plus size={15} aria-hidden="true" />
      </button>
      {menu && <TabMenu menu={menu} views={views} onClose={() => setMenu(null)} />}
    </div>
  );
}

/** Menú contextual de una pestaña (clic derecho o tecla de menú). Las acciones valen para ESA pestaña. */
function TabMenu({ menu, views, onClose }: { menu: MenuState; views: TabView[]; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: menu.x, top: menu.y });
  const moveTab = useStore((s) => s.moveTab);
  const duplicateTab = useStore((s) => s.duplicateTab);
  const at = views.findIndex((v) => v.id === menu.id);
  const view = views[at];
  const many = views.length > 1;
  const name = view ? tabLabel(view, many) : '';

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setPos({
      left: Math.max(4, Math.min(menu.x, window.innerWidth - el.offsetWidth - 4)),
      top: Math.max(4, Math.min(menu.y, window.innerHeight - el.offsetHeight - 4)),
    });
    el.querySelector<HTMLElement>('[role="menuitem"]:not([aria-disabled="true"])')?.focus();
  }, [menu]);

  useEffect(() => {
    const down = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const key = (e: globalThis.KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        onClose();
        document.getElementById(tabDomId(menu.id))?.focus();
      }
    };
    document.addEventListener('pointerdown', down, true);
    window.addEventListener('keydown', key, true);
    window.addEventListener('blur', onClose);
    window.addEventListener('resize', onClose);
    return () => {
      document.removeEventListener('pointerdown', down, true);
      window.removeEventListener('keydown', key, true);
      window.removeEventListener('blur', onClose);
      window.removeEventListener('resize', onClose);
    };
  }, [menu.id, onClose]);

  if (!view) return null;
  const items: { id: string; label: string; enabled: boolean; reason?: string; run: () => void }[] = [
    { id: 'close', label: t('tabs.cmd.close'), enabled: true, run: () => void closeTab(menu.id) },
    { id: 'others', label: t('tabs.cmd.closeOthers'), enabled: many, reason: t('tabs.reason.onlyOne'), run: () => void closeOtherTabs(menu.id) },
    {
      id: 'dup',
      label: t('tabs.cmd.duplicate'),
      enabled: true,
      run: () => {
        if (duplicateTab(menu.id)) {
          useStore.getState().notifyKey('info', 'tabs.duplicated');
          void useStore.getState().computePreview();
        }
      },
    },
    { id: 'left', label: t('tabs.menu.moveLeft'), enabled: at > 0, reason: t('tabs.reason.atStart'), run: () => moveTab(menu.id, at - 1) },
    { id: 'right', label: t('tabs.menu.moveRight'), enabled: at < views.length - 1, reason: t('tabs.reason.atEnd'), run: () => moveTab(menu.id, at + 1) },
    { id: 'window', label: t('tabs.cmd.moveToWindow'), enabled: many, reason: t('tabs.reason.onlyOne'), run: () => void sendTabToNewWindow(menu.id) },
  ];

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const els = [...(ref.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
    const i = els.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      const n = e.key === 'Home' ? 0 : e.key === 'End' ? els.length - 1 : (i + (e.key === 'ArrowDown' ? 1 : -1) + els.length) % els.length;
      els[n]?.focus();
    } else if (e.key === 'Tab') {
      e.preventDefault();
    }
  };

  return createPortal(
    <div ref={ref} className="tabmenu" role="menu" tabIndex={-1} aria-label={t('tabs.menu.label', { name })} style={{ left: pos.left, top: pos.top }} onKeyDown={onKeyDown}>
      {items.map((it) => (
        <button
          key={it.id}
          type="button"
          role="menuitem"
          className="tabmenu__item"
          aria-disabled={!it.enabled || undefined}
          title={it.enabled ? undefined : it.reason}
          onClick={() => {
            if (!it.enabled) return;
            onClose();
            it.run();
          }}
        >
          {it.label}
        </button>
      ))}
    </div>,
    document.body,
  );
}

/** Zona del diseño: el `tabpanel` de la pestaña activa. */
export function TabPanel({ children }: { children: React.ReactNode }) {
  const activeId = useStore((s) => s.activeTabId);
  return (
    <div className="tabpanel" id={TAB_PANEL_ID} role="tabpanel" aria-labelledby={tabDomId(activeId)}>
      {children}
    </div>
  );
}
