import { useEffect, type CSSProperties } from 'react';
import { getApi } from '@/api';
import { CanvasView } from '@/features/canvas';
import { DropOverlay, PromptHost, useCloseGuard, useFileDrop, useWindowTitle } from '@/features/history';
import { ColorPalette } from '@/features/layers';
import { ArrangeDialogHost } from '@/features/arrange';
import { DesignDialogHost } from '@/features/arrange/DesignDialogHost';
import { CameraDialogHost } from '@/features/camera';
import { DevicesDialogHost } from '@/features/devices';
import { MaterialsDialogHost } from '@/features/materials';
import { QueueHost } from '@/features/queue';
import { RecoveryHost } from '@/features/recovery';
import { BundleHost } from '@/features/bundle';
import { SettingsHost } from '@/features/settings';
import { AutomationHost } from '@/features/automation';
import { LiveHost } from '@/features/live';
import { RemoteHost } from '@/features/remote';
import { CostDialog } from '@/features/costs';
import { VersionsHost } from '@/features/versions';
import { PartsDialog } from '@/features/parts';
import { HelpDialogHost } from '@/features/help';
import { AlarmBanner, MachineDialogHost } from '@/features/machine';
import { TextPanel } from '@/features/text';
import { NestDialogHost } from '@/features/nest';
import { ReviewHost } from '@/features/review';
import { TilingHost } from '@/features/tiling';
import { PrintCutHost } from '@/features/printcut';
import { ArtPanelHost } from '@/features/art';
import { TraceDialogHost } from '@/features/trace';
import { PreviewBar } from '@/features/preview';
import { TabBar, TabPanel, TabsHost } from '@/features/tabs';
import { FirstRunDialog, SimplePanel, useUiMode } from '@/features/simple';
import { t } from '@/i18n';
import { shellColumns, type DockLayout } from '@/lib/dockLayout';
import { usePref, useStore } from '@/store';
import './shell-extra.css';
import { Dock } from './Dock';
import { FloatingPanels } from './FloatingPanels';
import { LiveAnnouncer } from './LiveAnnouncer';
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
    // Los avisos y errores del motor (Rust) salen en el mismo idioma que la interfaz.
    getApi()
      .setEngineLocale({ locale: language })
      .catch(() => undefined);
  }, [language]);
  return (
    <>
      <ShellBody key={language} />
      <SettingsHost />
      <AutomationHost />
      <RemoteHost />
      <LiveHost />
      <BundleHost />
    </>
  );
}

const COLUMN_WIDTH = { tools: 'var(--tools-w)', main: 'minmax(0, 1fr)', dock: 'auto' } as const;

/** Columnas de la ventana según el borde del panel y de las herramientas (preferencia `dockLayout`). */
export function shellGrid(layout: Pick<DockLayout, 'side' | 'toolsSide'>): CSSProperties {
  const cols = shellColumns(layout);
  return {
    gridTemplateColumns: cols.map((c) => COLUMN_WIDTH[c]).join(' '),
    gridTemplateAreas: `'top top top' '${cols.join(' ')}' 'palette palette palette' 'status status status'`,
  };
}

/** Estructura de la ventana: barra superior, herramientas, lienzo, panel, paleta de capas y estado, más avisos y diálogos. */
function ShellBody() {
  const previewVisible = useStore((s) => s.previewVisible);
  // En la pestaña Máquina ya se muestra la versión completa del aviso de alarma.
  const machineTab = useStore((s) => s.dockTab === 'machine');
  // Modo sencillo: flujo guiado en lugar de herramientas, panel con pestañas y paleta de capas.
  const simple = useUiMode() === 'simple';
  // La barra de pestañas se ve siempre en el modo completo; en el sencillo solo si ya hay varias.
  const showTabs = useStore((s) => !simple || s.tabs.length > 1);
  useWindowTitle();
  useCloseGuard();
  useFileDrop();
  useShortcutsHotkey();
  const layout = usePref('dockLayout');
  return (
    <div className={simple ? 'shell shell--simple' : 'shell'} style={simple ? undefined : shellGrid(layout)} data-tools-side={simple ? undefined : layout.toolsSide}>
      <a
        className="skip-link"
        href="#canvas-main"
        onClick={(e) => {
          e.preventDefault();
          document.getElementById('canvas-main')?.focus();
        }}
      >
        {t('a11y.skip.canvas')}
      </a>
      <TopBar />
      {!simple && <ToolPalette />}
      <main className="shell__main">
        {!machineTab && <AlarmBanner compact />}
        {showTabs && <TabBar />}
        <TabPanel>
          <CanvasView />
        </TabPanel>
        <TextPanel />
        {previewVisible && <PreviewBar />}
      </main>
      {simple ? <SimplePanel /> : <Dock />}
      {!simple && <FloatingPanels />}
      {!simple && (
        <div className="shell__palette">
          <ColorPalette />
        </div>
      )}
      <StatusBar />
      <Toasts />
      <LiveAnnouncer />
      <DropOverlay />
      <RecoveryHost />
      <TabsHost />
      <VersionsHost />
      <QueueHost />
      <PromptHost />
      <ShortcutsDialog />
      <CostDialog />
      <PartsDialog />
      <MachineDialogHost />
      <ArrangeDialogHost />
      <DesignDialogHost />
      <TraceDialogHost />
      <ArtPanelHost />
      <NestDialogHost />
      <TilingHost />
      <ReviewHost />
      <PrintCutHost />
      <CameraDialogHost />
      <MaterialsDialogHost />
      <DevicesDialogHost />
      <HelpDialogHost />
      <TemplatesHost />
      <FirstRunDialog />
    </div>
  );
}
