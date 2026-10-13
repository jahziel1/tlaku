import { useEffect, useRef, type PointerEvent } from 'react';
import { t } from '@/i18n';
import { useStore } from '@/store';
import type { Vec2 } from './geometry';
import { getCorrectedCanvas, usesGpu } from './session';

interface Props {
  /** Tamaño del espacio de la imagen corregida (los puntos van en esos píxeles). */
  space: [number, number] | null;
  points?: (Vec2 | null)[];
  active?: number;
  live: boolean;
  onPlace?: (p: Vec2) => void;
  onMove?: (i: number, p: Vec2) => void;
  onSelect?: (i: number) => void;
}

/** Muestra el canvas corregido de la sesión y, encima, los puntos de alineación arrastrables. */
export function CameraView({ space, points = [], active = -1, live, onPlace, onMove, onSelect }: Props) {
  const box = useRef<HTMLDivElement>(null);
  const drag = useRef<number | null>(null);
  const frozen = useStore((s) => s.cameraFrozen);
  const showing = live || frozen;

  useEffect(() => {
    const host = box.current;
    if (!host) return;
    const canvas = getCorrectedCanvas();
    canvas.className = 'cam-view__canvas';
    host.prepend(canvas);
    return () => {
      canvas.className = '';
      canvas.remove();
    };
  }, []);

  const toSpace = (e: { clientX: number; clientY: number }): Vec2 | null => {
    const canvas = getCorrectedCanvas();
    const r = canvas.getBoundingClientRect();
    if (!space || r.width === 0) return null;
    const x = ((e.clientX - r.left) / r.width) * space[0];
    const y = ((e.clientY - r.top) / r.height) * space[1];
    return [Math.min(space[0], Math.max(0, x)), Math.min(space[1], Math.max(0, y))];
  };

  const onDown = (e: PointerEvent<HTMLDivElement>) => {
    if (!onPlace || (e.target as HTMLElement).closest('.cam-pt')) return;
    const p = toSpace(e);
    if (p) onPlace(p);
  };

  const onPointDown = (e: PointerEvent<HTMLButtonElement>, i: number) => {
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = i;
    onSelect?.(i);
  };
  const onPointMove = (e: PointerEvent<HTMLButtonElement>) => {
    if (drag.current === null) return;
    const p = toSpace(e);
    if (p) onMove?.(drag.current, p);
  };

  return (
    <div
      ref={box}
      className="cam-view"
      data-live={showing}
      style={{ aspectRatio: space ? `${space[0]} / ${space[1]}` : '4 / 3', cursor: onPlace && showing ? 'crosshair' : 'default' }}
      role="img"
      aria-label={t('camera.view.label')}
      onPointerDown={onDown}
    >
      {!showing && <p className="cam-view__off">{t('camera.view.off')}</p>}
      {space &&
        showing &&
        points.map((p, i) =>
          p ? (
            <button
              key={i}
              type="button"
              className="cam-pt"
              data-active={active === i}
              style={{ left: `${(p[0] / space[0]) * 100}%`, top: `${(p[1] / space[1]) * 100}%` }}
              title={t('camera.align.movePoint', { n: i + 1 })}
              aria-label={t('camera.align.movePoint', { n: i + 1 })}
              onPointerDown={(e) => onPointDown(e, i)}
              onPointerMove={onPointMove}
              onPointerUp={() => (drag.current = null)}
            >
              <span className="num">{i + 1}</span>
            </button>
          ) : null,
        )}
      {showing && !usesGpu() && <p className="cam-view__warn">{t('camera.view.gpuMissing')}</p>}
    </div>
  );
}
