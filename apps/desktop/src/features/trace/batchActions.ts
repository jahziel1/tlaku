import { getApi, getApiKind } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { baseName } from '@/features/history/paths';
import { useStore } from '@/store';
import { applyBatchToDoc, runBatch, type BatchEntry } from './batch';
import { useBatchDialog } from './batchStore';
import { traceLayerFor } from './placement';

const state = () => useStore.getState();

export const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'bmp', 'webp', 'tif', 'tiff', 'gif', 'tga'];

/** Elige varias imágenes con el diálogo del sistema (en el modo simulado, dos de ejemplo). */
export async function pickImages(): Promise<string[]> {
  if (getApiKind() !== 'tauri') return ['ejemplos/logo-a.png', 'ejemplos/logo-b.png'];
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.open({ multiple: true, directory: false, filters: [{ name: t('trace.batch.filter'), extensions: IMAGE_EXTENSIONS }] });
  return Array.isArray(picked) ? picked : typeof picked === 'string' ? [picked] : [];
}

/** Lee el tamaño en píxeles de cada archivo (con el importador, que ya sabe leer todos los formatos). */
export async function loadEntries(paths: string[]): Promise<{ entries: BatchEntry[]; errors: string[] }> {
  const entries: BatchEntry[] = [];
  const errors: string[] = [];
  for (const path of paths) {
    try {
      const r = await getApi().importFile({ path });
      const img = r.objects.map((o) => o.kind).find((k) => k.type === 'image');
      if (img?.type !== 'image') {
        errors.push(t('trace.batch.notImage', { file: baseName(path) }));
        continue;
      }
      entries.push({ path: img.source_path || path, name: baseName(path), width_px: img.width_px, height_px: img.height_px });
    } catch (e) {
      errors.push(`${baseName(path)}: ${errorMessage(e)}`);
    }
  }
  return { entries, errors };
}

export async function addImages(): Promise<void> {
  const paths = await pickImages();
  if (paths.length === 0) return;
  const { entries, errors } = await loadEntries(paths);
  useBatchDialog.getState().addEntries(entries);
  if (errors.length > 0) state().notify('warning', t('trace.batch.addErrors', { n: errors.length }), errors);
}

let stop = false;

/** Pide detener el lote en marcha después del archivo actual. */
export function cancelBatch(): void {
  stop = true;
}

/** Vectoriza todos los archivos con los mismos ajustes y agrega los resultados, en filas, al documento. */
export async function runBatchTrace(): Promise<void> {
  const d = useBatchDialog.getState();
  if (d.entries.length === 0 || d.progress) return;
  stop = false;
  d.setFailures([]);
  d.setProgress({ done: 0, total: d.entries.length, current: null });
  const r = await runBatch({
    entries: d.entries,
    settings: d.settings,
    dpi: d.dpi,
    trace: (a) => getApi().traceImage(a),
    onProgress: (p) => useBatchDialog.getState().setProgress(p),
    cancelled: () => stop,
    message: errorMessage,
  });
  const dlg = useBatchDialog.getState();
  if (r.cancelled) {
    dlg.setProgress(null);
    state().notify('info', t('trace.batch.cancelled'));
    return;
  }
  const layer = traceLayerFor(state().doc.layers, state().activeLayer);
  const out: { ids: string[] } = { ids: [] };
  if (r.outcomes.length > 0) {
    state().applyDocChange('trace.batch.history', (doc) => {
      const applied = applyBatchToDoc(doc, r.outcomes, layer, [0, 0]);
      out.ids = applied.ids;
      return applied.doc;
    });
    state().select(out.ids);
    state().fitContent();
    const warnings = r.outcomes.flatMap((o) => o.warnings.map((w) => `${o.name}: ${w}`));
    if (warnings.length > 0) state().notify('warning', t('trace.batch.doneWarnings', { n: r.outcomes.length, w: warnings.length }), warnings);
    else state().notify('success', t('trace.batch.done', { n: r.outcomes.length }));
  }
  dlg.setFailures(r.failures);
  dlg.setProgress(null);
  // Con fallos el diálogo se queda abierto para verlos; si todo salió bien, se cierra.
  if (r.failures.length === 0) dlg.close();
}
