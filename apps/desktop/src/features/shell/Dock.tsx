import { Cpu, History, Layers, Terminal } from 'lucide-react';
import { useRef, type KeyboardEvent, type PointerEvent } from 'react';
import { useCommand } from '@/commands/useCommand';
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
];

function Tab({ id, icon: Icon, cmd, label, onKey }: (typeof TABS)[number] & { onKey: (e: KeyboardEvent) => void }) {
  const c = useCommand(cmd);
  return (
    <button
      type="button"
      role="tab"
      id={`tab-${id}`}
      aria-selected={c.checked}
      aria-controls={`tabpanel-${id}`}
      tabIndex={c.checked ? 0 : -1}
      className="tab"
      title={c.tooltip}
      onClick={c.run}
      onKeyDown={onKey}
    >
      <Icon size={15} aria-hidden="true" />
      {t(label)}
    </button>
  );
}

/** Panel acoplado a la derecha con pestañas Capas / Máquina / Consola / Historial. */
export function Dock() {
  const tab = useStore((s) => s.dockTab);
  const setTab = useStore((s) => s.setDockTab);
  const width = useStore((s) => s.dockWidth);
  const setWidth = useStore((s) => s.setDockWidth);
  const drag = useRef<{ x: number; w: number } | null>(null);

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
      <div role="tablist" aria-label={t('dock.label')} className="tabs">
        {TABS.map((x) => (
          <Tab key={x.id} {...x} onKey={onTabKey} />
        ))}
      </div>
      <div role="tabpanel" id={`tabpanel-${tab}`} aria-labelledby={`tab-${tab}`} className="dock__body">
        {tab === 'layers' && <LayersPanel />}
        {tab === 'machine' && <MachinePanel />}
        {tab === 'console' && <ConsolePanel />}
        {tab === HISTORY_TAB && <HistoryPanel />}
      </div>
    </aside>
  );
}
