import { useEffect, useMemo, useRef } from 'react';
import { FileUp } from 'lucide-react';
import { t } from '@/i18n';
import { useCommand } from '@/commands/useCommand';
import { selectActiveDevice, useStore, type AppState } from '@/store';
import { selectHeadDoc } from '@/store/machine';
import './canvas-overlay.css';
import { CanvasTip } from './CanvasTip';
import { useInteraction } from './interaction';
import { NumericBar } from './NumericBar';
import { readCanvasPalette } from './palette';
import { createRenderer } from './createRenderer';
import type { CanvasPalette, Scene } from './renderer';
import { Ruler, RULER_SIZE } from './Ruler';
import { SnapBar } from './SnapBar';
import { canRotate } from './transform';
import { useCanvasTools } from './useCanvasTools';
import { useViewportControls } from './useViewportControls';
import { ZoomControls } from './ZoomControls';

/** Campos del store que, al cambiar, obligan a volver a dibujar. */
const WATCHED: (keyof AppState)[] = [
  'zoom', 'panX', 'panY', 'viewport', 'doc', 'selectedIds', 'wireframe', 'theme',
  'preview', 'previewVisible', 'previewView', 'scrub', 'devices', 'activeDeviceId', 'status', 'connection',
];

// Estructuras derivadas que solo cambian con el documento / la selección: se recalculan por identidad.
let layersFor: AppState['doc']['layers'] | null = null;
let layersMap = new Map<string, AppState['doc']['layers'][number]>();
let selFor: string[] | null = null;
let selSet: ReadonlySet<string> = new Set();

function buildScene(s: AppState, palette: CanvasPalette, dpr: number): Scene {
  const dev = selectActiveDevice(s);
  if (layersFor !== s.doc.layers) {
    layersFor = s.doc.layers;
    layersMap = new Map(s.doc.layers.flatMap((l) => [[l.color, l] as const, [l.color.toLowerCase(), l] as const]));
  }
  if (selFor !== s.selectedIds) {
    selFor = s.selectedIds;
    selSet = new Set(s.selectedIds);
  }
  const bed = { width: dev.bed[0], height: dev.bed[1], origin: dev.origin };
  const ix = useInteraction.getState();
  const picked = s.selectedIds.length ? s.doc.objects.filter((o) => selSet.has(o.id) && !o.locked) : [];
  return {
    width: s.viewport.width,
    height: s.viewport.height,
    dpr,
    view: { zoom: s.zoom, panX: s.panX, panY: s.panY },
    bed,
    objects: s.doc.objects,
    layers: layersMap,
    selectedIds: selSet,
    wireframe: s.wireframe,
    preview: s.previewVisible && s.preview ? { data: s.preview, scrub: s.scrub } : null,
    palette,
    overlay: {
      hoverId: ix.hoverId,
      live: ix.live,
      rotating: ix.gesture === 'rotate',
      rotateMode: ix.rotateMode,
      canRotate: canRotate(picked),
      marquee: ix.marquee,
      guides: ix.guides,
      draft: ix.draft,
      pivot: ix.pivot,
    },
    // La posición de trabajo de la máquina se convierte al documento según el origen del perfil.
    head: selectHeadDoc(s),
  };
}

export function CanvasView() {
  const areaRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  useViewportControls(areaRef);
  useCanvasTools(areaRef, canvasRef);

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

    renderer.setInvalidate(schedule);
    const unsub = useStore.subscribe((s, prev) => {
      if (s.theme !== prev.theme) palette = readCanvasPalette();
      // Lo seleccionado que se bloquea, se oculta o desaparece deja de estar seleccionado.
      if (s.doc !== prev.doc) s.pruneSelection();
      if (WATCHED.some((k) => s[k] !== prev[k])) schedule();
    });
    // La interacción (hover, arrastres, guías) también repinta.
    const unsubInteraction = useInteraction.subscribe(schedule);
    schedule();
    return () => {
      unsub();
      unsubInteraction();
      if (frame) cancelAnimationFrame(frame);
      renderer.dispose();
    };
  }, []);

  const rulerW = Math.max(0, viewport.width);
  const rulerH = Math.max(0, viewport.height);
  const bedSize = useMemo(() => ({ x: bed[0], y: bed[1] }), [bed]);

  return (
    <div className="canvas-view" style={{ ['--ruler' as string]: `${RULER_SIZE}px` }}>
      <NumericBar />
      <div className="canvas-corner" aria-hidden="true">mm</div>
      <Ruler axis="x" length={rulerW} bedLength={bedSize.x} />
      <Ruler axis="y" length={rulerH} bedLength={bedSize.y} />
      <div ref={areaRef} className="canvas-area" data-testid="canvas-area">
        <canvas ref={canvasRef} className="canvas" style={{ width: viewport.width, height: viewport.height }} aria-label={t('canvas.label')} role="img" />
        <div className="canvas-hud canvas-hud--tl" aria-hidden="true">
          <span className="canvas-hud__name">{deviceName}</span>
          <span className="canvas-hud__dim num">{bed[0]} × {bed[1]} mm</span>
        </div>
        <SnapBar />
        <CanvasTip />
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
