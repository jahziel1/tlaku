import { useEffect } from 'react';
import type { Point } from '@/api/types';
import { dropPoint } from '@/features/art/ArtPanel';

/**
 * Mientras `active`, el siguiente clic en el lienzo devuelve el punto en mm del documento en lugar
 * de seleccionar o arrastrar: se atiende en la fase de captura del contenedor del lienzo, antes que
 * las herramientas. Con la cámara alineada, la imagen está dibujada en coordenadas de la cama, así
 * que ese punto es la posición física de lo que se ve bajo el puntero. Esc cancela.
 */
export function useCanvasPick(active: boolean, onPick: (point: Point) => void, onCancel: () => void): void {
  useEffect(() => {
    if (!active) return;
    const area = document.querySelector<HTMLElement>('.canvas-area');
    if (!area) return;
    const previousCursor = area.style.cursor;
    area.style.cursor = 'crosshair';
    const swallow = (e: Event) => {
      e.stopPropagation();
      e.preventDefault();
    };
    const onDown = (e: PointerEvent) => {
      if (e.button !== 0) return;
      swallow(e);
      const point = dropPoint(e.clientX, e.clientY);
      if (point) onPick(point);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      swallow(e);
      onCancel();
    };
    area.addEventListener('pointerdown', onDown, true);
    window.addEventListener('keydown', onKey, true);
    return () => {
      area.removeEventListener('pointerdown', onDown, true);
      window.removeEventListener('keydown', onKey, true);
      area.style.cursor = previousCursor;
    };
  }, [active, onPick, onCancel]);
}
