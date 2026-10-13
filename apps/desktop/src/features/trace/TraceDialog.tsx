import { TriangleAlert, X } from 'lucide-react';
import { useId, useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import type { ImageObject } from '@/api/types';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { colorName } from '@/lib/palette';
import { useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { Controls } from './Controls';
import { applyTraceToDoc, traceLayerFor } from './placement';
import { Preview } from './Preview';
import { looksRough, traceStats } from './stats';
import { useTraceDialog } from './traceStore';
import { useTraceRun } from './useTraceRun';
import './trace.css';

/** Diálogo "Vectorizar imagen": vista previa grande, ajustes y colocación del resultado. */
export function TraceDialog({ imageId, image }: { imageId: string; image: ImageObject }) {
  const titleId = useId();
  const close = useTraceDialog((s) => s.close);
  const { settings, deleteImage, showImage } = useTraceDialog(
    useShallow((s) => ({ settings: s.settings, deleteImage: s.deleteImage, showImage: s.showImage })),
  );
  const layers = useStore((s) => s.doc.layers);
  const activeLayer = useStore((s) => s.activeLayer);
  const locked = useStore((s) => s.doc.objects.find((o) => o.id === imageId)?.locked ?? false);

  const run = useTraceRun(image.source_path, image.size_mm, settings);
  const paths = run.result?.paths;
  const stats = useMemo(() => traceStats(paths ?? []), [paths]);
  const targetColor = traceLayerFor(layers, activeLayer);
  const targetName = layers.find((l) => l.color.toLowerCase() === targetColor.toLowerCase())?.name ?? colorName(targetColor);

  const blocked =
    run.status === 'computing'
      ? t('trace.accept.busy')
      : run.status === 'error' || !paths
        ? t('trace.accept.error')
        : paths.length === 0
          ? t('trace.accept.empty')
          : null;

  const accept = () => {
    if (blocked || !paths) return;
    const s = useStore.getState();
    const out: { id: string | null } = { id: null };
    s.applyDocChange('trace.history', (doc) => {
      const r = applyTraceToDoc(doc, { imageId, paths, layer: targetColor, deleteImage });
      out.id = r.id;
      return r.doc;
    });
    if (out.id) {
      s.select([out.id]);
      s.notify('success', t('trace.done', { shapes: stats.shapes, nodes: stats.nodes }));
    }
    close();
  };

  const [wMm, hMm] = image.size_mm;
  const warnings = run.result?.warnings ?? [];

  return (
    <Modal
      labelId={titleId}
      onClose={close}
      portal
      closeOnBackdrop
      fallbackFocus="card"
      passKey={(e) => (e.metaKey || e.ctrlKey) && e.key === '.'}
      backdropClassName="tr-backdrop"
      cardClassName="tr-modal"
  >
      <header className="tr-head">
        <div>
          <h2 id={titleId} className="tr-title">{t('trace.title')}</h2>
          <p className="hint num">
            {t('trace.subtitle', { mm: `${fmt(wMm)} × ${fmt(hMm)}`, px: `${image.width_px} × ${image.height_px}` })}
          </p>
        </div>
        <button type="button" className="icon-btn" title={t('trace.close')} aria-label={t('trace.close')} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>

      <div className="tr-body">
        <div className="tr-left">
          <Preview
            sourcePath={image.source_path}
            size={image.size_mm}
            paths={paths ?? []}
            showImage={showImage}
            onShowImage={useTraceDialog.getState().setShowImage}
            stale={run.status === 'computing'}
          />
          <div className="tr-stats" aria-live="polite">
            {stats.shapes === 0 ? (
              <span className="tr-stats__none">{run.result ? t('trace.stats.none') : t('trace.preview.computing')}</span>
            ) : (
              <>
                <span className="num">{t('trace.stats.shapes', { n: fmt(stats.shapes, 0) })}</span>
                <span className="num">{t('trace.stats.nodes', { n: fmt(stats.nodes, 0) })}</span>
              </>
            )}
          </div>
          {run.status === 'error' && (
            <p className="warn-line" role="alert">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{t('trace.error', { msg: run.error ?? '' })}</span>
            </p>
          )}
          {looksRough(stats) && (
            <p className="warn-line" role="status">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{t('trace.rough')}</span>
            </p>
          )}
          {warnings.length > 0 && (
            <ul className="tr-warnings" aria-label={t('trace.warnings')}>
              {warnings.map((w) => (
                <li key={w}>{w}</li>
              ))}
            </ul>
          )}
        </div>

        <div className="tr-controls">
          <Controls
            image={image}
            settings={settings}
            onChange={useTraceDialog.getState().patchSettings}
            onReset={useTraceDialog.getState().resetSettings}
          />
        </div>
      </div>

      <footer className="tr-foot">
        <div className="tr-foot__opts">
          <p className="tr-layer">
            <span className="tr-swatch" style={{ background: targetColor }} aria-hidden="true" />
            {t('trace.out.layer', { name: targetName })}
          </p>
          <label className="tr-check">
            <input
              type="checkbox"
              checked={deleteImage && !locked}
              disabled={locked}
              onChange={(e) => useTraceDialog.getState().setDeleteImage(e.target.checked)}
            />
            <span>{t('trace.out.deleteImage')}</span>
          </label>
          {locked && <p className="hint">{t('trace.out.deleteLocked')}</p>}
        </div>
        <button type="button" className="btn" onClick={close}>{t('trace.cancel')}</button>
        <button type="button" className="btn btn--accent" disabled={blocked !== null} title={blocked ?? undefined} onClick={accept}>
          {t('trace.accept')}
        </button>
      </footer>
    </Modal>
  );
}
