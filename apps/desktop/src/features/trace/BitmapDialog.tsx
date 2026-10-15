import { TriangleAlert, X } from 'lucide-react';
import { useId, useMemo, useState } from 'react';
import { getApi } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { objectBounds, unionBounds } from '@/lib/geometry';
import { fmt } from '@/lib/platform';
import { useShallow } from 'zustand/react/shallow';
import { pinTab, selectSelectedObjects, useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { NumberField } from '@/ui/NumberField';
import { applyBitmapToDoc, BITMAP_DPI_PRESETS, BITMAP_LIMITS, bitmapSourceIds, projectDir, projectStem } from './bitmap';
import { useBitmapDialog } from './bitmapStore';
import './trace.css';

/** Lado máximo de la imagen que acepta el motor (píxeles). */
const MAX_PIXELS = 100_000_000;

/** Diálogo "Convertir a mapa de bits": resolución, relleno o contorno, negativo y reemplazo. */
export function BitmapDialog() {
  const titleId = useId();
  const { options, replace, close, patch, setReplace } = useBitmapDialog();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const filePath = useStore((s) => s.filePath);
  const picked = useStore(useShallow((s) => selectSelectedObjects(s)));
  const sources = useMemo(() => picked.filter((o) => bitmapSourceIds([o]).length > 0), [picked]);
  const box = useMemo(() => unionBounds(sources.map(objectBounds)), [sources]);
  const unlocked = sources.some((o) => !o.locked);

  const mmPx = 25.4 / options.dpi;
  const wMm = box ? box[1][0] - box[0][0] : 0;
  const hMm = box ? box[1][1] - box[0][1] : 0;
  const wPx = Math.ceil(wMm / mmPx);
  const hPx = Math.ceil(hMm / mmPx);
  const tooBig = wPx * hPx > MAX_PIXELS;
  const dir = projectDir(filePath);

  const accept = async () => {
    setBusy(true);
    setError(null);
    try {
      const s = useStore.getState();
      const inTab = pinTab(s);
      const ids = bitmapSourceIds(selectSelectedObjects(s));
      const result = await getApi().vectorsToBitmap({
        doc: s.doc,
        selection: ids,
        dir,
        stem: projectStem(s.filePath, t('doc.untitled')),
        options,
      });
      const out: { id: string | null } = { id: null };
      const placed = inTab(() => {
        s.applyDocChange('trace.bitmap.history', (doc) => {
          const r = applyBitmapToDoc(doc, result, { sourceIds: ids, replace });
          out.id = r.id;
          return r.doc;
        });
        if (out.id) s.select([out.id]);
        return true;
      });
      if (!placed) throw new Error(t('tabs.targetClosedGeneric'));
      s.notify('success', t('trace.bitmap.done', { w: result.width_px, h: result.height_px, path: result.path }));
      if (result.in_temp_dir) s.notify('warning', t('trace.bitmap.temp'));
      void s.computePreview();
      close();
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  };

  return (
    <Modal labelId={titleId} onClose={close} portal closeOnBackdrop fallbackFocus="first" backdropClassName="tr-backdrop" cardClassName="tr-modal tr-modal--sm">
      <header className="tr-head">
        <div>
          <h2 id={titleId} className="tr-title">{t('trace.bitmap.title')}</h2>
          <p className="hint">{t('trace.bitmap.subtitle', { n: sources.length })}</p>
        </div>
        <button type="button" className="icon-btn" title={t('trace.close')} aria-label={t('trace.close')} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>

      <div className="tr-form">
        <div className="tr-field">
          <span className="tr-field__label">{t('trace.bitmap.dpi')}</span>
          <div className="tr-presets" role="group" aria-label={t('trace.bitmap.dpi')}>
            {BITMAP_DPI_PRESETS.map((d) => (
              <button key={d} type="button" className="tr-chip" aria-pressed={options.dpi === d} onClick={() => patch({ dpi: d })}>
                {d}
              </button>
            ))}
          </div>
          <NumberField
            label={t('trace.bitmap.dpiCustom')}
            value={options.dpi}
            min={BITMAP_LIMITS.dpi.min}
            max={BITMAP_LIMITS.dpi.max}
            step={10}
            unit="DPI"
            onCommit={(dpi) => patch({ dpi })}
          />
          <p className="hint num">{t('trace.bitmap.size', { w: fmt(wMm), h: fmt(hMm), wpx: fmt(wPx, 0), hpx: fmt(hPx, 0) })}</p>
          {tooBig && (
            <p className="warn-line" role="alert">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{t('trace.bitmap.tooBig')}</span>
            </p>
          )}
        </div>

        <div className="tr-field">
          <span className="tr-field__label">{t('trace.bitmap.style')}</span>
          <div className="tr-seg" role="radiogroup" aria-label={t('trace.bitmap.style')}>
            {([true, false] as const).map((filled) => (
              <label key={String(filled)}>
                <input type="radio" name="bitmap-style" checked={options.filled === filled} onChange={() => patch({ filled })} />
                <span>{t(filled ? 'trace.bitmap.filled' : 'trace.bitmap.outline')}</span>
              </label>
            ))}
          </div>
          <p className="hint">{t(options.filled ? 'trace.bitmap.filled.help' : 'trace.bitmap.outline.help')}</p>
        </div>

        {!options.filled && (
          <NumberField
            label={t('trace.bitmap.stroke')}
            value={options.stroke_mm}
            min={BITMAP_LIMITS.stroke_mm.min}
            max={BITMAP_LIMITS.stroke_mm.max}
            step={0.1}
            unit="mm"
            onCommit={(stroke_mm) => patch({ stroke_mm })}
          />
        )}

        <label className="tr-check">
          <input type="checkbox" checked={options.invert} onChange={(e) => patch({ invert: e.target.checked })} />
          <span>{t('trace.bitmap.invert')}</span>
        </label>
        <label className="tr-check">
          <input type="checkbox" checked={replace && unlocked} disabled={!unlocked} onChange={(e) => setReplace(e.target.checked)} />
          <span>{t('trace.bitmap.replace')}</span>
        </label>
        {!unlocked && <p className="hint">{t('trace.bitmap.locked')}</p>}

        <p className="hint">{dir ? t('trace.bitmap.where', { dir }) : t('trace.bitmap.whereTemp')}</p>
        {error && (
          <p className="warn-line" role="alert">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{t('trace.bitmap.error', { msg: error })}</span>
          </p>
        )}
      </div>

      <footer className="tr-foot">
        <button type="button" className="btn" onClick={close}>{t('trace.cancel')}</button>
        <button type="button" className="btn btn--accent" disabled={busy || tooBig || sources.length === 0} onClick={() => void accept()}>
          {busy ? t('trace.bitmap.busy') : t('trace.bitmap.accept')}
        </button>
      </footer>
    </Modal>
  );
}

export function BitmapDialogHost() {
  const open = useBitmapDialog((s) => s.open);
  return open ? <BitmapDialog /> : null;
}
