import { Camera, Cpu, History, Layers, Terminal } from 'lucide-react';
import { useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type RefObject } from 'react';
import { useCommand } from '@/commands/useCommand';
import { CameraPanel } from '@/features/camera';
import { HistoryPanel } from '@/features/history';
import { ConsolePanel, MachinePanel } from '@/features/machine';
import { LayersPanel } from '@/features/layers';
import { t, type I18nKey } from '@/i18n';
import { useStore } from '@/store';
import { DOCK_MAX, DOCK_MIN, type DockTab } from '@/store/view';
import { HISTORY_TAB } from './dockTabs';

const TABS: { id: DockTab; icon: typeof Layers; cmd: string; label: I18nKey }[] = [
  { id: 'layers', icon: Layers, cmd: 'panel.layers', label: 'dock.layers' },
  { id: 'machine', icon: Cpu, cmd: 'panel.machine', label: 'dock.machine' },
  { id: 'console', icon: Terminal, cmd: 'panel.console', label: 'dock.console' },
  { id: HISTORY_TAB, icon: History, cmd: 'panel.history', label: 'history.dock.title' },
  { id: 'camera', icon: Camera, cmd: 'panel.camera', label: 'dock.camera' },
];

function Tab({ id, icon: Icon, cmd, label, onKey, compact }: (typeof TABS)[number] & { onKey: (e: KeyboardEvent) => void; compact: boolean }) {
  const c = useCommand(cmd);
  return (
    <button
      type="button"
      role="tab"
      id={`tab-${id}`}
      aria-selected={c.checked}
      aria-controls={`tabpanel-${id}`}
      tabIndex={c.checked ? 0 : -1}
      className={compact ? 'tab tab--icon' : 'tab'}
      title={c.tooltip}
      // Sin texto visible, el nombre accesible sale de aquí (y el tooltip de `title`).
      aria-label={compact ? t(label) : undefined}
      onClick={c.run}
      onKeyDown={onKey}
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

/** Panel acoplado a la derecha con pestañas Capas / Máquina / Consola / Historial / Cámara. */
export function Dock() {
  const tab = useStore((s) => s.dockTab);
  const setTab = useStore((s) => s.setDockTab);
  const width = useStore((s) => s.dockWidth);
  const setWidth = useStore((s) => s.setDockWidth);
  const drag = useRef<{ x: number; w: number } | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const compact = useCompactTabs(listRef);

  const onTabKey = (e: KeyboardEvent) => {
    const i = TABS.findIndex((x) => x.id === tab);
    const next = e.key === 'ArrowRight' ? i + 1 : e.key === 'ArrowLeft' ? i - 1 : null;
    if (next === null) return;
    e.preventDefault();
    const target = TABS[(next + TABS.length) % TABS.length]!;
    setTab(target.id);
    requestAnimationFrame(() => document.getElementById(`tab-${target.id}`)?.focus());
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
      <div ref={listRef} role="tablist" aria-label={t('dock.label')} className="tabs">
        {TABS.map((x) => (
          <Tab key={x.id} {...x} onKey={onTabKey} compact={compact} />
        ))}
      </div>
      <div role="tabpanel" id={`tabpanel-${tab}`} aria-labelledby={`tab-${tab}`} className="dock__body">
        {tab === 'layers' && <LayersPanel />}
        {tab === 'machine' && <MachinePanel />}
        {tab === 'console' && <ConsolePanel />}
        {tab === HISTORY_TAB && <HistoryPanel />}
        {tab === 'camera' && <CameraPanel />}
      </div>
    </aside>
  );
}
