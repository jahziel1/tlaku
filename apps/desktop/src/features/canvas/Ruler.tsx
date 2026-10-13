import { useEffect, useRef, type PointerEvent as ReactPointerEvent } from 'react';
import { t } from '@/i18n';
import { formatTick, gridSteps } from '@/lib/viewport';
import { useStore } from '@/store';
import { createRulerDrag } from './guideTool';
import { useGuideUi } from './guides';

export const RULER_SIZE = 22;

interface Props {
  axis: 'x' | 'y';
  /** Largo (px) del lado paralelo al eje. */
  length: number;
  /** Largo de la cama en mm, para no rotular fuera de ella. */
  bedLength: number;
}

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/**
 * Regla en mm. Se dibuja sola a partir del store (zoom, paneo, cursor y tema). Arrastrar desde ella
 * hacia el lienzo crea una guía (la regla de arriba crea guías horizontales; la de la izquierda,
 * verticales); el doble clic abre el diálogo de guías.
 */
export function Ruler({ axis, length, bedLength }: Props) {
  const ref = useRef<HTMLCanvasElement>(null);
  const drag = useRef<ReturnType<typeof createRulerDrag> | null>(null);
  const pointer = useRef<number | null>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas || length <= 0) return;
    const dpr = window.devicePixelRatio || 1;
    const w = axis === 'x' ? length : RULER_SIZE;
    const h = axis === 'x' ? RULER_SIZE : length;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);

    const draw = () => {
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      const { zoom, panX, panY, cursor } = useStore.getState();
      const bg = cssVar('--ruler-bg');
      const ink = cssVar('--ruler-ink');
      const tick = cssVar('--ruler-tick');
      const accent = cssVar('--accent');
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.fillStyle = bg;
      ctx.fillRect(0, 0, w, h);

      const { minor } = gridSteps(zoom, 8);
      // Posición en px del mm `mm` a lo largo del eje.
      const pos = (mm: number) => (axis === 'x' ? panX + mm * zoom : panY - mm * zoom);
      const lo = axis === 'x' ? (0 - panX) / zoom : (panY - h) / zoom;
      const hi = axis === 'x' ? (w - panX) / zoom : panY / zoom;
      const first = Math.max(0, Math.floor(lo / minor));
      const last = Math.min(Math.floor(bedLength / minor), Math.ceil(hi / minor));

      ctx.font = '500 9.5px "Spline Sans Mono Variable", ui-monospace, monospace';
      ctx.textBaseline = 'alphabetic';
      ctx.fillStyle = ink;
      ctx.strokeStyle = tick;
      ctx.lineWidth = 1;
      ctx.beginPath();
      const labels: [string, number][] = [];
      for (let i = first; i <= last; i++) {
        const p = Math.round(pos(i * minor)) + 0.5;
        const major = i % 10 === 0;
        const mid = i % 5 === 0;
        const len = major ? RULER_SIZE : mid ? 9 : 5;
        if (axis === 'x') {
          ctx.moveTo(p, h);
          ctx.lineTo(p, h - len);
        } else {
          ctx.moveTo(w, p);
          ctx.lineTo(w - len, p);
        }
        if (major) labels.push([formatTick(i * minor), p]);
      }
      ctx.stroke();
      for (const [text, p] of labels) {
        if (axis === 'x') ctx.fillText(text, p + 3, 10);
        else {
          ctx.save();
          ctx.translate(10, p - 3);
          ctx.rotate(-Math.PI / 2);
          ctx.fillText(text, 0, 0);
          ctx.restore();
        }
      }

      if (cursor) {
        const p = Math.round(pos(axis === 'x' ? cursor[0] : cursor[1])) + 0.5;
        ctx.strokeStyle = accent;
        ctx.lineWidth = 1;
        ctx.beginPath();
        if (axis === 'x') {
          ctx.moveTo(p, 0);
          ctx.lineTo(p, h);
        } else {
          ctx.moveTo(0, p);
          ctx.lineTo(w, p);
        }
        ctx.stroke();
      }
    };

    draw();
    return useStore.subscribe((s, prev) => {
      if (s.zoom !== prev.zoom || s.panX !== prev.panX || s.panY !== prev.panY || s.cursor !== prev.cursor || s.theme !== prev.theme) draw();
    });
  }, [axis, length, bedLength]);

  const rect = () => ref.current!.getBoundingClientRect();
  const onDown = (e: ReactPointerEvent<HTMLCanvasElement>) => {
    if (e.button !== 0 || !useStore.getState().showGuides) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    pointer.current = e.pointerId;
    drag.current = createRulerDrag(axis);
  };
  const onMove = (e: ReactPointerEvent<HTMLCanvasElement>) => {
    if (e.pointerId === pointer.current) drag.current?.move(rect(), e);
  };
  const onUp = (e: ReactPointerEvent<HTMLCanvasElement>) => {
    if (e.pointerId !== pointer.current) return;
    pointer.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
    drag.current?.finish(rect(), e);
    drag.current = null;
  };
  const onCancel = () => {
    pointer.current = null;
    drag.current?.cancel();
    drag.current = null;
  };

  const style = {
    ...(axis === 'x' ? { width: length, height: RULER_SIZE } : { width: RULER_SIZE, height: length }),
    cursor: axis === 'x' ? 'row-resize' : 'col-resize',
  };
  return (
    <canvas
      ref={ref}
      className="ruler"
      style={style}
      title={t('canvas.guide.rulerHint')}
      onPointerDown={onDown}
      onPointerMove={onMove}
      onPointerUp={onUp}
      onPointerCancel={onCancel}
      onDoubleClick={() => useGuideUi.getState().show()}
    />
  );
}
