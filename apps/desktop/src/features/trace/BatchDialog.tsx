import { FilePlus2, TriangleAlert, X } from 'lucide-react';
import { useId, useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { t } from '@/i18n';
import { NumberField } from '@/ui/NumberField';
import { colorName } from '@/lib/palette';
import { useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { BATCH_DPI, entrySize } from './batch';
import { addImages, cancelBatch, runBatchTrace } from './batchActions';
import { useBatchDialog } from './batchStore';
import { Controls } from './Controls';
import { traceLayerFor } from './placement';
import { Preview } from './Preview';
import { useTraceRun } from './useTraceRun';
import './trace.css';

/** Vista previa del archivo elegido con los ajustes del lote. */
function BatchPreview({ path, size }: { path: string; size: [number, number] }) {
  const settings = useBatchDialog((s) => s.settings);
  const run = useTraceRun(path, size, settings);
  return (
    <>
      <Preview sourcePath={path} size={size} paths={run.result?.paths ?? []} showImage onShowImage={() => undefined} stale={run.status === 'computing'} />
      {run.status === 'error' && (
        <p className="warn-line" role="alert">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('trace.error', { msg: run.error ?? '' })}</span>
        </p>
      )}
    </>
  );
}

/** Diálogo "Vectorizar varios archivos": mismos ajustes para todos, con avance. */
export function BatchDialog() {
  const titleId = useId();
  const d = useBatchDialog(
    useShallow((s) => ({ entries: s.entries, selected: s.selected, settings: s.settings, dpi: s.dpi, progress: s.progress, failures: s.failures })),
  );
  const { close, removeEntry, select, patchSettings, resetSettings, setDpi } = useBatchDialog.getState();
  const layers = useStore((s) => s.doc.layers);
  const activeLayer = useStore((s) => s.activeLayer);
  const targetColor = traceLayerFor(layers, activeLayer);
  const targetName = layers.find((l) => l.color.toLowerCase() === targetColor.toLowerCase())?.name ?? colorName(targetColor);
  const running = d.progress !== null;
  const current = useMemo(() => d.entries.find((e) => e.path === d.selected) ?? null, [d.entries, d.selected]);
  const size = current ? entrySize(current, d.dpi) : null;
  const failedPaths = new Set(d.failures.map((f) => f.path));

  const onClose = () => {
    if (running) cancelBatch();
    close();
  };

  return (
    <Modal labelId={titleId} onClose={onClose} portal fallbackFocus="card" passKey={(e) => (e.metaKey || e.ctrlKey) && e.key === '.'} backdropClassName="tr-backdrop" cardClassName="tr-modal tr-batch">
      <header className="tr-head">
        <div>
          <h2 id={titleId} className="tr-title">{t('trace.batch.title')}</h2>
          <p className="hint">{t('trace.batch.subtitle')}</p>
        </div>
        <button type="button" className="icon-btn" title={t('trace.close')} aria-label={t('trace.close')} onClick={onClose}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>

      <div className="tr-body">
        <div className="tr-left">
          {current && size ? (
            <BatchPreview key={`${current.path}-${size[0]}`} path={current.path} size={size} />
          ) : (
            <p className="hint tr-empty">{t('trace.batch.empty')}</p>
          )}
          <ul className="tr-files" aria-label={t('trace.batch.files')}>
            {d.entries.map((e) => (
              <li key={e.path} className="tr-file" aria-current={e.path === d.selected} data-status={failedPaths.has(e.path) ? 'error' : undefined}>
                <button type="button" className="tr-file__name" title={e.path} onClick={() => select(e.path)}>{e.name}</button>
                <span className="tr-file__state hint num">{e.width_px} × {e.height_px} px</span>
                <button type="button" className="icon-btn" disabled={running} title={t('trace.batch.remove', { name: e.name })} aria-label={t('trace.batch.remove', { name: e.name })} onClick={() => removeEntry(e.path)}>
                  <X size={14} aria-hidden="true" />
                </button>
              </li>
            ))}
          </ul>
          {d.failures.length > 0 && (
            <ul className="tr-warnings" aria-label={t('trace.batch.failures')}>
              {d.failures.map((f) => (
                <li key={f.path}>{f.name}: {f.error || t('trace.batch.noShapes')}</li>
              ))}
            </ul>
          )}
        </div>

        <div className="tr-controls">
          <button type="button" className="btn" disabled={running} onClick={() => void addImages()}>
            <FilePlus2 size={14} aria-hidden="true" /> {t('trace.batch.add')}
          </button>
          <NumberField
            label={t('trace.batch.dpi')}
            value={d.dpi}
            min={BATCH_DPI.min}
            max={BATCH_DPI.max}
            step={1}
            unit="DPI"
            disabled={running}
            onCommit={setDpi}
          />
          <Controls image={{ size_mm: size ?? [100, 100], width_px: current?.width_px ?? 100 }} settings={d.settings} onChange={patchSettings} onReset={resetSettings} />
        </div>
      </div>

      <footer className="tr-foot">
        <div className="tr-foot__opts">
          {running ? (
            <>
              <progress className="tr-progress" max={d.progress!.total} value={d.progress!.done} aria-label={t('trace.batch.progress')} />
              <p className="hint num" aria-live="polite">
                {t('trace.batch.running', { done: d.progress!.done, total: d.progress!.total, name: d.progress!.current ?? '' })}
              </p>
            </>
          ) : (
            <p className="tr-layer">
              <span className="tr-swatch" style={{ background: targetColor }} aria-hidden="true" />
              {t('trace.batch.layer', { name: targetName })}
            </p>
          )}
        </div>
        <button type="button" className="btn" onClick={onClose}>{running ? t('trace.batch.stop') : t('trace.cancel')}</button>
        <button type="button" className="btn btn--accent" disabled={running || d.entries.length === 0} onClick={() => void runBatchTrace()}>
          {t('trace.batch.accept', { n: d.entries.length })}
        </button>
      </footer>
    </Modal>
  );
}

export function BatchDialogHost() {
  const open = useBatchDialog((s) => s.open);
  return open ? <BatchDialog /> : null;
}
