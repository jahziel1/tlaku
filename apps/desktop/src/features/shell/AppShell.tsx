import { CanvasView } from '@/features/canvas';
import { DropOverlay, PromptHost, useCloseGuard, useFileDrop, useWindowTitle } from '@/features/history';
import { ColorPalette } from '@/features/layers';
import { AlarmBanner, MachineDialogHost } from '@/features/machine';
import { PreviewBar } from '@/features/preview';
import { useStore } from '@/store';
import './shell-extra.css';
import { Dock } from './Dock';
import { ShortcutsDialog, useShortcutsHotkey } from './ShortcutsDialog';
import { StatusBar } from './StatusBar';
import { ToolPalette } from './ToolPalette';
import { TopBar } from './TopBar';
import { Toasts } from './Toasts';

/** Estructura de la ventana: barra superior, herramientas, lienzo, panel, paleta de capas y estado, más avisos y diálogos. */
export function AppShell() {
  const previewVisible = useStore((s) => s.previewVisible);
  // En la pestaña Máquina ya se muestra la versión completa del aviso de alarma.
  const machineTab = useStore((s) => s.dockTab === 'machine');
  useWindowTitle();
  useCloseGuard();
  useFileDrop();
  useShortcutsHotkey();
  return (
    <div className="shell">
      <TopBar />
      <ToolPalette />
      <main className="shell__main">
        {!machineTab && <AlarmBanner compact />}
        <CanvasView />
        {previewVisible && <PreviewBar />}
      </main>
      <Dock />
      <div className="shell__palette">
        <ColorPalette />
      </div>
      <StatusBar />
      <Toasts />
      <DropOverlay />
      <PromptHost />
      <ShortcutsDialog />
      <MachineDialogHost />
    </div>
  );
}
