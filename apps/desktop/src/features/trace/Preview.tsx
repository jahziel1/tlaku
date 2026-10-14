import { Maximize2, ZoomIn, ZoomOut } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';
import { imageUrl, type BezPath } from '@/api';
import { t } from '@/i18n';
import { bezToSvgD } from './stats';
import { fitView, panBy, viewBoxOf, zoomAt, type TraceView } from './view';

interface Props {
  sourcePath: string;
  /** Tamaño de la imagen en mm. */
  size: [number, number];
  paths: BezPath[];
  showImage: boolean;
  onShowImage: (v: boolean) => void;
  /** Hay un cálculo en curso: el resultado dibujado es el anterior. */
  stale: boolean;
}

/**
 * Imagen y trazado en un mismo SVG (unidades: mm, Y hacia abajo), así se alinean exactamente.
 * Las curvas del motor tienen Y hacia arriba: se voltean con una matriz.
 */
export function Preview({ sourcePath, size, paths, showImage, onShowImage, stale }: Props) {
  const [w, h] = size;
  const [view, setView] = useState<TraceView>(() => fitView(w, h));
  const [panning, setPanning] = useState(false);
  const stage = useRef<HTMLDivElement>(null);
  const svg = useRef<SVGSVGElement>(null);
  const drag = useRef<{ x: number; y: number } | null>(null);
  const d = useMemo(() => bezToSvgD(paths), [paths]);

  // Pantalla a unidades de imagen: la escala sale del propio SVG (letterbox incluido).
  const unitsPerPx = () => {
    const ctm = svg.current?.getScreenCTM();
    return ctm && ctm.a !== 0 ? 1 / ctm.a : 1;
  };
  const pointAt = (clientX: number, clientY: number): [number, number] => {
    const ctm = svg.current?.getScreenCTM();
    if (!ctm) return [w / 2, h / 2];
    const p = new DOMPoint(clientX, clientY).matrixTransform(ctm.inverse());
    return [p.x, p.y];
  };

  useEffect(() => {
    const el = stage.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const [px, py] = pointAt(e.clientX, e.clientY);
      setView((v) => zoomAt(v, Math.exp(-e.deltaY * 0.0015), px, py, w, h));
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [w, h]);

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 && e.button !== 1) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, y: e.clientY };
    setPanning(true);
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const last = drag.current;
    if (!last) return;
    const k = unitsPerPx();
    drag.current = { x: e.clientX, y: e.clientY };
    setView((v) => panBy(v, -(e.clientX - last.x) * k, -(e.clientY - last.y) * k, w, h));
  };
  const endDrag = () => {
    drag.current = null;
    setPanning(false);
  };

  const zoomBy = (factor: number) => setView((v) => zoomAt(v, factor, v.cx, v.cy, w, h));
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === '+' || e.key === '=') zoomBy(1.4);
    else if (e.key === '-') zoomBy(1 / 1.4);
    else if (e.key === '0') setView(fitView(w, h));
    else return;
    e.preventDefault();
  };

  return (
    <section className="tr-preview">
      <div className="tr-preview__bar">
        <button type="button" className="icon-btn icon-btn--boxed" title={t('trace.preview.zoomIn')} aria-label={t('trace.preview.zoomIn')} onClick={() => zoomBy(1.4)}>
          <ZoomIn size={15} aria-hidden="true" />
        </button>
        <button type="button" className="icon-btn icon-btn--boxed" title={t('trace.preview.zoomOut')} aria-label={t('trace.preview.zoomOut')} onClick={() => zoomBy(1 / 1.4)}>
          <ZoomOut size={15} aria-hidden="true" />
        </button>
        <button type="button" className="icon-btn icon-btn--boxed" title={t('trace.preview.fit')} aria-label={t('trace.preview.fit')} onClick={() => setView(fitView(w, h))}>
          <Maximize2 size={15} aria-hidden="true" />
        </button>
        <label className="tr-check">
          <input type="checkbox" checked={showImage} onChange={(e) => onShowImage(e.target.checked)} />
          <span>{t('trace.preview.showImage')}</span>
        </label>
        <span className="hint">{t('trace.preview.hint')}</span>
      </div>
      {/* Zona de pan/zoom enfocable: flechas y +/- ya tienen su manejador de teclado (onKeyDown). */}
      {/* eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions */}
      <div
        ref={stage}
        className="tr-stage"
        data-panning={panning}
        data-stale={stale}
        // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
        role="img"
        aria-label={t('trace.preview.label')}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onKeyDown={onKeyDown}
        onDoubleClick={() => setView(fitView(w, h))}
      >
        <svg ref={svg} className="tr-svg" viewBox={viewBoxOf(view, w, h)} preserveAspectRatio="xMidYMid meet">
          <rect className="tr-paper" x={0} y={0} width={w} height={h} />
          {showImage && <image href={imageUrl(sourcePath)} x={0} y={0} width={w} height={h} preserveAspectRatio="none" />}
          <g transform={`matrix(1 0 0 -1 0 ${h})`}>
            <path className="tr-ink" d={d} vectorEffect="non-scaling-stroke" />
          </g>
        </svg>
        {stale && <span className="tr-busy" role="status">{t('trace.preview.computing')}</span>}
      </div>
    </section>
  );
}
