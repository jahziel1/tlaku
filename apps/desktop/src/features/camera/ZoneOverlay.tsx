import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { t } from '@/i18n';
import { screenToWorld, worldToScreen } from '@/lib/viewport';
import { formatSize } from '@/lib/units';
import { selectActiveDevice, usePref, useStore } from '@/store';
import type { Vec2 } from './geometry';
import { useHeadUi } from './headState';
import { rectFromDrag, zoneSize } from './zone';

/** Contenedor del lienzo del diseño (lo monta el área `canvas`). Se busca hasta que aparezca. */
function useCanvasArea(): HTMLElement | null {
  const [area, setArea] = useState<HTMLElement | null>(null);
  useEffect(() => {
    let tries = 0;
    let timer = 0;
    const look = () => {
      const el = document.querySelector<HTMLElement>('.canvas-area');
      if (el) setArea(el);
      else if (tries++ < 50) timer = window.setTimeout(look, 100);
    };
    look();
    return () => window.clearTimeout(timer);
  }, []);
  return area;
}

/**
 * Capa sobre el lienzo para la zona del recorrido de captura (M36): muestra el rectángulo elegido
 * (punteado, con su tamaño) y, mientras el usuario "dibuja la zona", recibe el arrastre. Se monta
 * dentro del contenedor del lienzo con un portal para no tocar el código del lienzo; sus eventos
 * se atienden con oyentes nativos para que el arrastre no llegue a las herramientas del lienzo.
 */
export function ZoneOverlay() {
  const area = useCanvasArea();
  const zone = useHeadUi((s) => s.zone);
  const drawing = useHeadUi((s) => s.drawingZone);
  const showing = useHeadUi((s) => s.useZone);
  const setZone = useHeadUi((s) => s.setZone);
  const setDrawing = useHeadUi((s) => s.setDrawingZone);
  const zoom = useStore((s) => s.zoom);
  const panX = useStore((s) => s.panX);
  const panY = useStore((s) => s.panY);
  const bed = useStore((s) => selectActiveDevice(s).bed);
  const headOn = useStore((s) => s.cameraSetup.head?.enabled === true);
  const units = usePref('units');
  const layer = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<{ a: Vec2; b: Vec2 } | null>(null);

  useEffect(() => {
    const el = layer.current;
    if (!el || !drawing) return;
    const view = () => ({ zoom: useStore.getState().zoom, panX: useStore.getState().panX, panY: useStore.getState().panY });
    const world = (e: PointerEvent): Vec2 => {
      const r = el.getBoundingClientRect();
      return screenToWorld(view(), [e.clientX - r.left, e.clientY - r.top]);
    };
    let start: Vec2 | null = null;
    const down = (e: PointerEvent) => {
      if (e.button !== 0) return;
      e.stopPropagation();
      e.preventDefault();
      el.setPointerCapture(e.pointerId);
      start = world(e);
      setDrag({ a: start, b: start });
    };
    const move = (e: PointerEvent) => {
      e.stopPropagation();
      if (start) setDrag({ a: start, b: world(e) });
    };
    const up = (e: PointerEvent) => {
      e.stopPropagation();
      if (!start) return;
      const rect = rectFromDrag(start, world(e), selectActiveDevice(useStore.getState()).bed);
      start = null;
      setDrag(null);
      if (rect) {
        setZone(rect);
        setDrawing(false);
      }
    };
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      start = null;
      setDrag(null);
      setDrawing(false);
    };
    el.addEventListener('pointerdown', down);
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
    window.addEventListener('keydown', key);
    return () => {
      el.removeEventListener('pointerdown', down);
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerup', up);
      window.removeEventListener('keydown', key);
      setDrag(null);
    };
  }, [drawing, setZone, setDrawing]);

  // La zona es del recorrido de la cámara del cabezal: sin ese modo no se muestra.
  if (!area || !headOn) return null;
  const view = { zoom, panX, panY };
  const live = drag ? rectFromDrag(drag.a, drag.b, bed) : null;
  const shown = live ?? (showing ? zone : null);
  const box = (() => {
    if (drag && !live) {
      const [p, q] = [worldToScreen(view, drag.a), worldToScreen(view, drag.b)];
      return { x: Math.min(p[0], q[0]), y: Math.min(p[1], q[1]), w: Math.abs(p[0] - q[0]), h: Math.abs(p[1] - q[1]) };
    }
    if (!shown) return null;
    const [p, q] = [worldToScreen(view, [shown[0], shown[3]]), worldToScreen(view, [shown[2], shown[1]])];
    return { x: p[0], y: p[1], w: q[0] - p[0], h: q[1] - p[1] };
  })();
  if (!drawing && !box) return null;
  const size = shown ? zoneSize(shown) : null;

  return createPortal(
    <div ref={layer} className="cam-zone" data-drawing={drawing || undefined} data-testid="cam-zone-layer">
      {box && (
        <div className="cam-zone__rect" style={{ left: box.x, top: box.y, width: box.w, height: box.h }}>
          {size && <span className="cam-zone__label num">{formatSize(size[0], size[1], units, 1)}</span>}
        </div>
      )}
      {drawing && <p className="cam-zone__tip">{t('camera.zone.tip')}</p>}
    </div>,
    area,
  );
}
