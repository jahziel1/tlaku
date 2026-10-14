import { Eye, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { getApi } from '@/api';
import type { LayerSettings } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { usePreviewUi } from './previewStore';
import type { ImagePreview } from './types';
import { clampCenter, imagesOfLayer, MAX_PX, panCenter, pickImage, pxPerMm, viewRegion, VIEW_H, VIEW_W, ZOOMS, type Zoom } from './view';
import './images.css';

const DEBOUNCE_MS = 200;

const ZOOM_LABEL: Record<string, I18nKey> = {
  fit: 'layers.preview.zoom.fit',
  '1': 'layers.preview.zoom.real',
  '2': 'layers.preview.zoom.2',
  '4': 'layers.preview.zoom.4',
};

/** Botón del editor de capa que abre (o cierra) la vista previa por imagen, y el panel flotante. */
export function ImagePreviewButton({ layer, disabled }: { layer: LayerSettings; disabled?: boolean }) {
  const doc = useStore((s) => s.doc);
  const open = usePreviewUi((s) => s.open);
  const set = usePreviewUi((s) => s.set);
  const none = imagesOfLayer(doc, layer.color).length === 0;
  return (
    <div className="lyr-field">
      <button type="button" className="btn" aria-pressed={open} disabled={disabled || none} title={none ? t('layers.preview.none') : t('layers.preview.hint')} onClick={() => set({ open: !open })}>
        <Eye size={14} aria-hidden="true" />{t('layers.preview.open')}
      </button>
      {none && <p className="lyr-help">{t('layers.preview.none')}</p>}
      {open && !none && typeof document !== 'undefined' && createPortal(<ImagePreviewPanel layer={layer} />, document.body)}
    </div>
  );
}

interface Shown {
  key: string;
  preview: ImagePreview;
  /** Centro (mm) y zoom con que se pidió, para colocar la imagen en el visor. */
  center: [number, number];
  zoom: Zoom;
}

/**
 * Panel flotante con lo que se grabaría de una imagen con los ajustes de la capa, antes de
 * calcular el trabajo. Sigue los cambios de los controles; a escala real y con zoom muestra la
 * rejilla de grabado exacta (arrastra para moverte), y entera, una versión más gruesa y rápida.
 */
export function ImagePreviewPanel({ layer }: { layer: LayerSettings }) {
  const doc = useStore((s) => s.doc);
  const selected = useStore((s) => s.selectedIds);
  const { imageId, zoom, center, set } = usePreviewUi();
  const images = imagesOfLayer(doc, layer.color);
  const obj = pickImage(images, imageId, selected);
  const [shown, setShown] = useState<Shown | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const drag = useRef<{ x: number; y: number } | null>(null);

  const size: [number, number] | null = obj ? obj.kind.size_mm : null;
  const at: [number, number] | null = size ? clampCenter(center ?? [size[0] / 2, size[1] / 2], zoom, size) : null;
  const region = size && at ? viewRegion(zoom, at, size) : null;
  // Lo que cambia la vista: la imagen, los ajustes que la procesan, el zoom y el trozo.
  const key = obj ? JSON.stringify([obj.id, obj.kind, layer.image, layer.interval, region]) : '';

  useEffect(() => {
    if (!obj || !at) return;
    let stale = false;
    setBusy(true);
    const timer = setTimeout(() => {
      getApi()
        .previewImage({ image: obj.kind, layer, region_mm: region, max_px: MAX_PX })
        .then(
          (preview) => {
            if (stale) return;
            setShown({ key, preview, center: at, zoom });
            setError(null);
            setBusy(false);
          },
          (e: unknown) => {
            if (stale) return;
            setError(errorMessage(e));
            setBusy(false);
          },
        );
    }, DEBOUNCE_MS);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
    // `key` resume imagen, ajustes y trozo; `zoom` cambia `region` salvo entre `fit` y `fit`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, zoom]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !(e.target instanceof HTMLInputElement)) set({ open: false });
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [set]);

  const p = shown?.preview;
  const k = shown && size ? pxPerMm(shown.zoom, size) : 1;
  let img: { width: number; height: number; left: number; top: number } | null = null;
  if (p && shown && size) {
    const [x0, y0, x1, y1] = p.covered_mm;
    const half = [VIEW_W / k / 2, VIEW_H / k / 2];
    img = {
      width: (x1 - x0) * k,
      height: (y1 - y0) * k,
      left: (x0 - (shown.center[0] - half[0]!)) * k,
      top: (shown.center[1] + half[1]! - y1) * k,
    };
    if (shown.zoom === 'fit') {
      img.left = (VIEW_W - img.width) / 2;
      img.top = (VIEW_H - img.height) / 2;
    }
  }

  return (
    <aside className="img-preview" role="dialog" aria-label={t('layers.preview.title')}>
      <header className="img-preview__head">
        <h2 className="panel-title">{t('layers.preview.title')}</h2>
        <button type="button" className="icon-btn" aria-label={t('layers.preview.close')} title={t('layers.preview.close')} onClick={() => set({ open: false })}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>
      {!obj ? (
        <p className="hint" role="status">{t('layers.preview.none')}</p>
      ) : (
        <>
          <div className="img-preview__bar">
            {images.length > 1 && (
              <select className="select" aria-label={t('layers.preview.image')} value={obj.id} onChange={(e) => set({ imageId: e.target.value, center: null })}>
                {images.map((i) => <option key={i.id} value={i.id}>{i.id}</option>)}
              </select>
            )}
            <div className="img-zoom" role="group" aria-label={t('layers.preview.zoom')}>
              {ZOOMS.map((z) => (
                <button key={String(z)} type="button" aria-pressed={zoom === z} className="btn" onClick={() => set({ zoom: z, center: z === zoom ? center : null })}>
                  {t(ZOOM_LABEL[String(z)]!)}
                </button>
              ))}
            </div>
          </div>
          <div
            className="img-preview__view"
            style={{ width: VIEW_W, height: VIEW_H }}
            data-pannable={zoom !== 'fit'}
            onPointerDown={(e) => {
              if (zoom === 'fit') return;
              drag.current = { x: e.clientX, y: e.clientY };
              e.currentTarget.setPointerCapture(e.pointerId);
            }}
            onPointerMove={(e) => {
              if (!drag.current || !size || !at) return;
              set({ center: panCenter(at, e.clientX - drag.current.x, e.clientY - drag.current.y, zoom, size) });
              drag.current = { x: e.clientX, y: e.clientY };
            }}
            onPointerUp={() => (drag.current = null)}
            onPointerCancel={() => (drag.current = null)}
          >
            {p && img && (
              <img
                className="img-preview__img"
                alt={t('layers.preview.alt', { name: obj.id })}
                src={`data:image/png;base64,${p.png_base64}`}
                draggable={false}
                style={{ width: img.width, height: img.height, left: img.left, top: img.top, imageRendering: shown!.zoom === 'fit' ? 'auto' : 'pixelated' }}
              />
            )}
            {busy && <span className="img-preview__busy" role="status">{t('layers.preview.updating')}</span>}
          </div>
          {error && <p className="warn-line" role="alert">{error}</p>}
          {p && (
            <p className="hint" role="status">
              {p.exact
                ? t('layers.preview.exact', { mm: fmt(p.interval_mm, 3), w: p.width_px, h: p.height_px })
                : t('layers.preview.approx', { mm: fmt(p.interval_mm, 2) })}
            </p>
          )}
          {zoom !== 'fit' && <p className="hint">{t('layers.preview.pan')}</p>}
        </>
      )}
    </aside>
  );
}
