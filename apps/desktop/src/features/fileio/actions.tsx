import { getApi, getApiKind } from '@/api';
import type { Document } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { pickImportPaths } from '@/features/history/dialogs';
import { baseName, extensionOf, isImportable, withExtension } from '@/features/history/paths';
import { useStore, type AppState } from '@/store';
import { ExportDialog, defaultRemembered, type ExportChoice, type RememberedOptions } from './ExportDialog';
import { ImportOptionsDialog, kindsOf } from './ImportOptionsDialog';
import { openDialog } from './host';
import { EXPORT_EXTENSION, type ExportFormat, type ExportSummary, type ImportDialogOptions } from './types';

const state = () => useStore.getState();

/** Cómo se hacen las preguntas al usuario; se sustituye en las pruebas. */
export interface FileioIo {
  askExport: (format: ExportFormat, print: boolean, selectionCount: number) => Promise<ExportChoice | null>;
  askImport: (paths: string[]) => Promise<ImportDialogOptions | null>;
  pickSavePath: (format: ExportFormat, defaultName: string) => Promise<string | null>;
  pickImportPaths: () => Promise<string[]>;
}

/** Opciones de la última exportación de cada formato (solo de esta sesión). */
let remembered: RememberedOptions = defaultRemembered();

export const defaultIo: FileioIo = {
  askExport: async (format, print, selectionCount) => {
    const choice = await openDialog<(ExportChoice & { options: RememberedOptions }) | null>((done) => (
      <ExportDialog format={format} print={print} selectionCount={selectionCount} initial={remembered} onDone={done} />
    ));
    if (choice) remembered = choice.options;
    return choice;
  },
  askImport: (paths) => {
    const exts = paths.map(extensionOf);
    const layers = state().doc.layers.filter((l) => l.mode !== 'image').map((l) => ({ color: l.color, name: l.name || l.color }));
    return openDialog<ImportDialogOptions | null>((done) => (
      <ImportOptionsDialog fileCount={paths.length} kinds={kindsOf(exts)} layers={layers} onDone={done} />
    ));
  },
  pickSavePath: async (format, defaultName) => {
    if (getApiKind() !== 'tauri') return `exportes/${defaultName}`;
    const dialog = await import('@tauri-apps/plugin-dialog');
    return dialog.save({
      filters: [{ name: t(`fileio.save.filter.${format}`), extensions: [EXPORT_EXTENSION[format]] }],
      defaultPath: defaultName,
    });
  },
  pickImportPaths,
};

/** Se puede exportar si hay algo visible en el diseño. */
export function exportEnabled(s: AppState): true | I18nKey {
  return s.doc.objects.some((o) => !o.hidden) ? true : 'fileio.reason.empty';
}

/** `1,5 MB`, `320 KB`... */
export function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1).replace('.', ',')} MB`;
}

const mm = (v: number) => (Math.round(v * 10) / 10).toString().replace('.', ',');

/** Nombre sugerido: el del proyecto (o "Sin título") con la extensión del formato. */
export function suggestedName(filePath: string | null, format: ExportFormat): string {
  const base = filePath ? baseName(filePath).replace(/\.[^.]+$/, '') : t('doc.untitled');
  return `${base}.${EXPORT_EXTENSION[format]}`;
}

function report(summary: ExportSummary): void {
  const text = summary.warnings.length > 0
    ? t('fileio.notice.exportedWithWarnings', { path: summary.path, w: summary.warnings.length })
    : t('fileio.notice.exported', { path: summary.path, w: mm(summary.size_mm[0]), h: mm(summary.size_mm[1]), size: fmtBytes(summary.bytes) });
  if (summary.warnings.length > 0) state().notify('warning', text, summary.warnings);
  else state().notify('success', text);
}

/** Ids de la selección que existen y se pueden exportar, o `null` si se exporta todo. */
function scopeIds(choice: ExportChoice): string[] | null {
  return choice.scope === 'selection' ? [...state().selectedIds] : null;
}

/** Exporta el diseño (o la selección) al formato elegido. `true` si se escribió el archivo. */
export async function exportAs(format: ExportFormat, io: FileioIo = defaultIo): Promise<boolean> {
  if (exportEnabled(state()) !== true) return false;
  const choice = await io.askExport(format, false, state().selectedIds.length);
  if (!choice) return false;
  const picked = await io.pickSavePath(format, suggestedName(state().filePath, format));
  if (!picked) return false;
  const path = withExtension(picked, EXPORT_EXTENSION[format]);
  const doc: Document = state().doc;
  try {
    const summary = await getApi().exportDesign({ doc, selection: scopeIds(choice), path, request: choice.request });
    report(summary);
    return true;
  } catch (e) {
    state().notify('error', t('fileio.notice.exportFailed', { error: errorMessage(e) }));
    return false;
  }
}

/** Imprimir: genera el PDF a escala 1:1 y lo abre en el visor del sistema. */
export async function printDesign(io: FileioIo = defaultIo): Promise<boolean> {
  if (exportEnabled(state()) !== true) return false;
  const choice = await io.askExport('pdf', true, state().selectedIds.length);
  if (!choice || choice.request.format !== 'pdf') return false;
  const { format: _format, ...options } = choice.request;
  try {
    const summary = await getApi().printDesign({ doc: state().doc, selection: scopeIds(choice), options });
    if (summary.warnings.length > 0) {
      state().notify('warning', t('fileio.notice.printed', { path: summary.path }), summary.warnings);
    } else {
      state().notify('success', t('fileio.notice.printed', { path: summary.path }));
    }
    return true;
  } catch (e) {
    state().notify('error', t('fileio.notice.printFailed', { error: errorMessage(e) }));
    return false;
  }
}

/**
 * Importa con el diálogo de opciones (escala, DPI, unidades del DXF, página del PDF, agrupar,
 * capa destino). Hace lo mismo que `importPaths` de History (avisos persistentes, selección y
 * ajuste de la vista), pero con `importFileWith`. Devuelve cuántos objetos entraron.
 */
export async function importWithOptions(io: FileioIo = defaultIo): Promise<number> {
  const paths = (await io.pickImportPaths()).filter((p) => {
    const ok = isImportable(p);
    if (!ok) state().notify('error', t('history.notice.unsupported', { file: baseName(p) }));
    return ok;
  });
  if (paths.length === 0) return 0;
  const options = await io.askImport(paths);
  if (!options) return 0;
  const ids: string[] = [];
  for (const path of paths) {
    const file = baseName(path);
    try {
      const result = await getApi().importFileWith({ path, options });
      if (result.objects.length === 0) {
        state().notify('warning', t('history.notice.importedNothing', { file }), result.warnings);
        continue;
      }
      ids.push(...state().applyImport(result));
      const n = result.objects.length;
      if (result.warnings.length > 0) {
        state().notify('warning', t('history.notice.importedWithWarnings', { n, file, w: result.warnings.length }), result.warnings);
      } else {
        state().notify('success', t('history.notice.importedFile', { n, file }));
      }
    } catch (e) {
      state().notify('error', t('history.notice.importFailed', { file, error: errorMessage(e) }));
    }
  }
  if (ids.length > 0) {
    state().select(ids);
    state().fitContent();
  }
  return ids.length;
}
