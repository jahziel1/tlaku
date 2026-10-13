import { useEffect, useRef, type RefObject } from 'react';
import { isEditableTarget } from '@/commands/shortcuts';
import { screenToWorld } from '@/lib/viewport';
import { useStore } from '@/store';

/**
 * Zoom y paneo del lienzo:
 *  - Rueda del ratón: zoom en el cursor. Pellizco del trackpad (ctrl+rueda): zoom.
 *    Dos dedos en el trackpad (rueda con decimales o desplazamiento horizontal): paneo.
 *  - Paneo: botón central, Espacio + arrastrar, o la herramienta Mano.
 * También mantiene al día el tamaño del lienzo y la posición del cursor en mm.
 */
export function useViewportControls(areaRef: RefObject<HTMLElement | null>): void {
  const space = useRef(false);

  useEffect(() => {
    const el = areaRef.current;
    if (!el) return;

    const ro = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const { width, height } = entry.contentRect;
      useStore.getState().setViewport(Math.round(width), Math.round(height));
    });
    ro.observe(el);

    const local = (e: { clientX: number; clientY: number }): [number, number] => {
      const r = el.getBoundingClientRect();
      return [e.clientX - r.left, e.clientY - r.top];
    };

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const s = useStore.getState();
      const [sx, sy] = local(e);
      const trackpadScroll = !e.ctrlKey && e.deltaMode === 0 && (e.deltaX !== 0 || !Number.isInteger(e.deltaY));
      if (trackpadScroll) {
        s.panBy(-e.deltaX, -e.deltaY);
        return;
      }
      const k = e.ctrlKey ? 0.01 : 0.0016;
      s.zoomAt(Math.exp(-e.deltaY * k), sx, sy);
    };
    el.addEventListener('wheel', onWheel, { passive: false });

    let drag: { id: number; x: number; y: number } | null = null;
    const onDown = (e: PointerEvent) => {
      const s = useStore.getState();
      const wantsPan = e.button === 1 || (e.button === 0 && (space.current || s.activeTool === 'pan'));
      if (!wantsPan) return;
      e.preventDefault();
      el.setPointerCapture(e.pointerId);
      drag = { id: e.pointerId, x: e.clientX, y: e.clientY };
      el.dataset.panning = 'true';
    };
    const onMove = (e: PointerEvent) => {
      const s = useStore.getState();
      if (drag && drag.id === e.pointerId) {
        s.panBy(e.clientX - drag.x, e.clientY - drag.y);
        drag = { ...drag, x: e.clientX, y: e.clientY };
      }
      const [sx, sy] = local(e);
      s.setCursor(screenToWorld(s, [sx, sy]));
    };
    const onUp = (e: PointerEvent) => {
      if (drag?.id === e.pointerId) {
        drag = null;
        delete el.dataset.panning;
      }
    };
    const onLeave = () => useStore.getState().setCursor(null);
    el.addEventListener('pointerdown', onDown);
    el.addEventListener('pointermove', onMove);
    el.addEventListener('pointerup', onUp);
    el.addEventListener('pointercancel', onUp);
    el.addEventListener('pointerleave', onLeave);

    const onKey = (e: KeyboardEvent) => {
      if (e.code !== 'Space' || isEditableTarget(e.target)) return;
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === 'BUTTON' || tag === 'A') return; // Espacio activa botones
      space.current = e.type === 'keydown';
      el.dataset.space = String(space.current);
      if (e.type === 'keydown') e.preventDefault();
    };
    window.addEventListener('keydown', onKey);
    window.addEventListener('keyup', onKey);
    const onBlur = () => {
      space.current = false;
      el.dataset.space = 'false';
    };
    window.addEventListener('blur', onBlur);

    return () => {
      ro.disconnect();
      el.removeEventListener('wheel', onWheel);
      el.removeEventListener('pointerdown', onDown);
      el.removeEventListener('pointermove', onMove);
      el.removeEventListener('pointerup', onUp);
      el.removeEventListener('pointercancel', onUp);
      el.removeEventListener('pointerleave', onLeave);
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('keyup', onKey);
      window.removeEventListener('blur', onBlur);
    };
  }, [areaRef]);
}
