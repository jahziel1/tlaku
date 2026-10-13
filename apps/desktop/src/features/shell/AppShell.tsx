import { CanvasView } from '@/features/canvas';
import { ColorPalette } from '@/features/layers';
import { PreviewBar } from '@/features/preview';
import { useStore } from '@/store';
import { Dock } from './Dock';
import { StatusBar } from './StatusBar';
import { ToolPalette } from './ToolPalette';
import { TopBar } from './TopBar';
import { Toasts } from './Toasts';

/** Estructura de la ventana: barra superior, herramientas, lienzo, panel, paleta de capas y estado. */
export function AppShell() {
  const previewVisible = useStore((s) => s.previewVisible);
  return (
    <div className="shell">
      <TopBar />
      <ToolPalette />
      <main className="shell__main">
        <CanvasView />
        {previewVisible && <PreviewBar />}
      </main>
      <Dock />
      <div className="shell__palette">
        <ColorPalette />
      </div>
      <StatusBar />
      <Toasts />
    </div>
  );
}
