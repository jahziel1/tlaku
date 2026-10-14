import { Camera, Cpu, History, Layers, Library, StickyNote, Terminal } from 'lucide-react';
import { CameraPanel } from '@/features/camera';
import { HistoryPanel } from '@/features/history';
import { ConsolePanel, MachinePanel } from '@/features/machine';
import { LayersPanel } from '@/features/layers';
import { MaterialsPanel } from '@/features/materials';
import type { I18nKey } from '@/i18n';
import type { DockTab } from '@/store/view';
import { HISTORY_TAB } from './dockTabs';
import { NotesPanel } from './NotesPanel';

export interface DockTabDef {
  id: DockTab;
  icon: typeof Layers;
  /** Comando que abre la pestaña (`useCommand`). */
  cmd: string;
  label: I18nKey;
}

/** Las pestañas del panel (acoplado o flotante), con su ícono, comando y nombre. */
export const TABS: DockTabDef[] = [
  { id: 'layers', icon: Layers, cmd: 'panel.layers', label: 'dock.layers' },
  { id: 'machine', icon: Cpu, cmd: 'panel.machine', label: 'dock.machine' },
  { id: 'console', icon: Terminal, cmd: 'panel.console', label: 'dock.console' },
  { id: HISTORY_TAB, icon: History, cmd: 'panel.history', label: 'history.dock.title' },
  { id: 'camera', icon: Camera, cmd: 'panel.camera', label: 'dock.camera' },
  { id: 'materials', icon: Library, cmd: 'panel.materials', label: 'dock.materials' },
  { id: 'notes', icon: StickyNote, cmd: 'panel.notes', label: 'canvas.dock.notes' },
];
export const TAB_BY_ID = new Map(TABS.map((x) => [x.id, x]));

/** Contenido de una pestaña: es el mismo en el panel acoplado y en una ventana flotante. */
export function PanelBody({ id }: { id: DockTab }) {
  switch (id) {
    case 'layers':
      return <LayersPanel />;
    case 'machine':
      return <MachinePanel />;
    case 'console':
      return <ConsolePanel />;
    case HISTORY_TAB:
      return <HistoryPanel />;
    case 'camera':
      return <CameraPanel />;
    case 'materials':
      return <MaterialsPanel />;
    case 'notes':
      return <NotesPanel />;
    default:
      return null;
  }
}
