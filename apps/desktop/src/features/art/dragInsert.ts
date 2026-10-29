import { useRef, useState } from 'react';
import type { Point } from '@/api/types';
import { screenToWorld } from '@/lib/viewport';
import { useStore } from '@/store';

/** Cuánto hay que mover el puntero para que un clic pase a ser un arrastre. */
const DRAG_PX = 6;

/** El lienzo bajo el puntero, y el punto del documento (mm) sobre el que está. */
export function dropPoint(clientX: number, clientY: number): Point | null {
  const el = document.elementFromPoint(clientX, clientY);
  const canvas = el instanceof HTMLCanvasElement && el.classList.contains('canvas') ? el : el?.closest('.canvas-area')?.querySelector('canvas.canvas');
  if (!(canvas instanceof HTMLCanvasElement)) return null;
  const rect = canvas.getBoundingClientRect();
  const { zoom, panX, panY } = useStore.getState();
  return screenToWorld({ zoom, panX, panY }, [clientX - rect.left, clientY - rect.top]);
}

/**
 * Un botón que se inserta con un clic (o con Intro/Espacio) y, si se arrastra, se suelta sobre el
 * lienzo en el punto de destino. `insert(undefined)` = clic; `insert(punto)` = soltado en el lienzo.
 * Devuelve los manejadores del botón y la posición del fantasma que sigue al puntero.
 */
export function useDragInsert(insert: (at?: Point) => void) {
  const [ghost, setGhost] = useState<{ x: number; y: number } | null>(null);
  const start = useRef<{ x: number; y: number; id: number } | null>(null);
  const dragging = useRef(false);

  const finish = (e: React.PointerEvent<HTMLElement>, cancelled: boolean) => {
    const s = start.current;
    start.current = null;
    setGhost(null);
    if (!s) return;
    if (e.currentTarget.hasPointerCapture(s.id)) e.currentTarget.releasePointerCapture(s.id);
    if (cancelled) return;
    if (!dragging.current) {
      insert();
      return;
    }
    dragging.current = false;
    const at = dropPoint(e.clientX, e.clientY);
    if (at) insert(at);
  };

  const handlers = {
    onPointerDown: (e: React.PointerEvent<HTMLElement>) => {
      if (e.button !== 0) return;
      start.current = { x: e.clientX, y: e.clientY, id: e.pointerId };
      dragging.current = false;
      e.currentTarget.setPointerCapture(e.pointerId);
    },
    onPointerMove: (e: React.PointerEvent<HTMLElement>) => {
      const s = start.current;
      if (!s) return;
      if (!dragging.current && Math.hypot(e.clientX - s.x, e.clientY - s.y) > DRAG_PX) dragging.current = true;
      if (dragging.current) setGhost({ x: e.clientX, y: e.clientY });
    },
    onPointerUp: (e: React.PointerEvent<HTMLElement>) => finish(e, false),
    onPointerCancel: (e: React.PointerEvent<HTMLElement>) => finish(e, true),
    onKeyDown: (e: React.KeyboardEvent<HTMLElement>) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        insert();
      }
    },
  };
  return { handlers, ghost };
}
