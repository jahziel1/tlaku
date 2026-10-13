import { useEffect, useMemo, useRef } from 'react';
import { FileUp } from 'lucide-react';
import { t } from '@/i18n';
import { useCommand } from '@/commands/useCommand';
import { selectActiveDevice, useStore, type AppState } from '@/store';
import { readCanvasPalette } from './palette';
import { createRenderer } from './createRenderer';
import type { CanvasPalette, Scene } from './renderer';
import { Ruler, RULER_SIZE } from './Ruler';
import { useViewportControls } from './useViewportControls';
import { ZoomControls } from './ZoomControls';

/** Campos del store que, al cambiar, obligan a volver a dibujar. */
const WATCHED: (keyof AppState)[] = [
  'zoom', 'panX', 'panY', 'viewport', 'doc', 'selectedIds', 'wireframe', 'theme',
  'preview', 'previewVisible', 'scrub', 'devices', 'activeDeviceId',
];

function buildScene(s: AppState, palette: CanvasPalette, dpr: number): Scene {
  const dev = selectActiveDevice(s);
  return {
    width: s.viewport.width,
    height: s.viewport.height,
    dpr,
    view: { zoom: s.zoom, panX: s.panX, panY: s.panY },
    bed: { width: dev.bed[0], height: dev.bed[1], origin: dev.origin },
    objects: s.doc.objects,
    layers: new Map(s.doc.layers.map((l) => [l.color, l])),
    selectedIds: new Set(s.selectedIds),
    wireframe: s.wireframe,
    preview: s.previewVisible && s.preview ? { data: s.preview, scrub: s.scrub } : null,
    palette,
  };
}

export function CanvasView() {
  const areaRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  useViewportControls(areaRef);

  const viewport = useStore((s) => s.viewport);
  const bed = useStore((s) => selectActiveDevice(s).bed);
  const deviceName = useStore((s) => selectActiveDevice(s).name);
  const empty = useStore((s) => s.doc.objects.length === 0);
  const importCmd = useCommand('file.import');

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const renderer = createRenderer();
    renderer.attach(canvas);
    let palette = readCanvasPalette();
    let frame = 0;
    let lastW = 0;
    let lastH = 0;
    let lastDpr = 0;

    const draw = () => {
      frame = 0;
      const s = useStore.getState();
      const dpr = window.devicePixelRatio || 1;
      if (s.viewport.width !== lastW || s.viewport.height !== lastH || dpr !== lastDpr) {
        renderer.resize(s.viewport.width, s.viewport.height, dpr);
        lastW = s.viewport.width;
        lastH = s.viewport.height;
        lastDpr = dpr;
      }
      renderer.render(buildScene(s, palette, dpr));
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(draw);
    };

    const unsub = useStore.subscribe((s, prev) => {
      if (s.theme !== prev.theme) palette = readCanvasPalette();
      if (WATCHED.some((k) => s[k] !== prev[k])) schedule();
    });
    schedule();
    return () => {
      unsub();
      if (frame) cancelAnimationFrame(frame);
      renderer.dispose();
    };
  }, []);

  const rulerW = Math.max(0, viewport.width);
  const rulerH = Math.max(0, viewport.height);
  const bedSize = useMemo(() => ({ x: bed[0], y: bed[1] }), [bed]);

  return (
    <div className="canvas-view" style={{ ['--ruler' as string]: `${RULER_SIZE}px` }}>
      <div className="canvas-corner" aria-hidden="true">mm</div>
      <Ruler axis="x" length={rulerW} bedLength={bedSize.x} />
      <Ruler axis="y" length={rulerH} bedLength={bedSize.y} />
      <div ref={areaRef} className="canvas-area" data-testid="canvas-area">
        <canvas ref={canvasRef} className="canvas" style={{ width: viewport.width, height: viewport.height }} aria-label={t('canvas.label')} role="img" />
        <div className="canvas-hud canvas-hud--tl" aria-hidden="true">
          <span className="canvas-hud__name">{deviceName}</span>
          <span className="canvas-hud__dim num">{bed[0]} × {bed[1]} mm</span>
        </div>
        <ZoomControls />
        {empty && (
          <div className="canvas-empty">
            <p className="canvas-empty__title">{t('canvas.empty.title')}</p>
            <p className="canvas-empty__text">{t('canvas.empty.text')}</p>
            <button type="button" className="btn btn--accent" onClick={importCmd.run}>
              <FileUp size={15} aria-hidden="true" />
              {importCmd.label}
              <kbd className="kbd">{importCmd.shortcut}</kbd>
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
