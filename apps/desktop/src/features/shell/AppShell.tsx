import { useEffect } from 'react';
import { CanvasView } from '@/features/canvas';
import { DropOverlay, PromptHost, useCloseGuard, useFileDrop, useWindowTitle } from '@/features/history';
import { ColorPalette } from '@/features/layers';
import { ArrangeDialogHost } from '@/features/arrange';
import { DesignDialogHost } from '@/features/arrange/DesignDialogHost';
import { CameraDialogHost } from '@/features/camera';
import { DevicesDialogHost } from '@/features/devices';
import { MaterialsDialogHost } from '@/features/materials';
import { RecoveryHost } from '@/features/recovery';
import { BundleHost } from '@/features/bundle';
import { SettingsHost } from '@/features/settings';
import { AutomationHost } from '@/features/automation';
import { HelpDialogHost } from '@/features/help';
import { AlarmBanner, MachineDialogHost } from '@/features/machine';
import { TextPanel } from '@/features/text';
import { NestDialogHost } from '@/features/nest';
import { ArtPanelHost } from '@/features/art';
import { TraceDialogHost } from '@/features/trace';
import { PreviewBar } from '@/features/preview';
import { usePref, useStore } from '@/store';
import './shell-extra.css';
import { Dock } from './Dock';
import { ShortcutsDialog, useShortcutsHotkey } from './ShortcutsDialog';
import { StatusBar } from './StatusBar';
import { TemplatesHost } from './TemplatesDialog';
import { ToolPalette } from './ToolPalette';
import { TopBar } from './TopBar';
import { Toasts } from './Toasts';

/**
 * Raíz de la ventana. Al cambiar el idioma se vuelve a montar todo el contenido (la `key`), así cada
 * texto se traduce de nuevo. El documento, el historial y las preferencias viven en el store, no en
 * estos componentes, así que no se pierde nada. `SettingsHost` queda fuera de la `key`: su arranque
 * lee las preferencias del disco y no debe repetirse al cambiar de idioma.
 */
export function AppShell() {
  const language = usePref('language');
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);
  return (
    <>
      <ShellBody key={language} />
      <SettingsHost />
      <AutomationHost />
      <BundleHost />
    </>
  );
}

/** Estructura de la ventana: barra superior, herramientas, lienzo, panel, paleta de capas y estado, más avisos y diálogos. */
function ShellBody() {
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
        <TextPanel />
        {previewVisible && <PreviewBar />}
      </main>
      <Dock />
      <div className="shell__palette">
        <ColorPalette />
      </div>
      <StatusBar />
      <Toasts />
      <DropOverlay />
      <RecoveryHost />
      <PromptHost />
      <ShortcutsDialog />
      <MachineDialogHost />
      <ArrangeDialogHost />
      <DesignDialogHost />
      <TraceDialogHost />
      <ArtPanelHost />
      <NestDialogHost />
      <CameraDialogHost />
      <MaterialsDialogHost />
      <DevicesDialogHost />
      <HelpDialogHost />
      <TemplatesHost />
    </div>
  );
}
