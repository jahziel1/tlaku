import { getApi } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { effectiveImportDpi } from '@/lib/importOptions';
import { fmtClock } from '@/lib/platform';
import { DEFAULT_IMPORT_OPTIONS } from '@/features/fileio/types';
import { resolveJobArgs } from '@/features/variables/jobResolve';
import { buildJobArgs, useStore } from '@/store';
import { pickImportPaths, pickPath } from './dialogs';
import { baseName, isImportable, withExtension } from './paths';
import { askChoice } from './prompt';
import { useRecents } from './recent';
import { guardUnsaved, type GuardChoice, type GuardIntent, type GuardIo } from './unsaved';

const state = () => useStore.getState();

export function documentName(path: string | null = state().filePath): string {
  return path ? baseName(path) : t('doc.untitled');
}

/** Pregunta guardar, descartar o cancelar con el diálogo propio de la app. */
export async function askUnsaved(intent: GuardIntent): Promise<GuardChoice> {
  const id = await askChoice({
    title: t('history.unsaved.title'),
    message: t(`history.unsaved.ask.${intent}`, { name: documentName() }),
    choices: [
      { id: 'save', label: t('history.unsaved.save'), tone: 'accent' },
      { id: 'discard', label: t('history.unsaved.discard'), tone: 'danger' },
      { id: 'cancel', label: t('history.unsaved.cancel') },
    ],
    cancelId: 'cancel',
  });
  return id === 'save' || id === 'discard' ? id : 'cancel';
}

/** Operaciones reales del flujo de cambios sin guardar. */
export const unsavedIo: GuardIo = {
  isDirty: () => state().dirty,
  ask: askUnsaved,
  save: () => saveProject(false),
};

/** `true` si se puede seguir (sin cambios, o ya se guardaron o descartaron). Úsalo antes de acciones que sustituyen el documento. */
export function ensureSaved(intent: GuardIntent): Promise<boolean> {
  return guardUnsaved(intent, unsavedIo);
}

export async function newProject(): Promise<boolean> {
  if (!(await ensureSaved('new'))) return false;
  state().newDocument();
  state().fitBed();
  void state().computePreview();
  return true;
}

/** Carga `path` en el editor. Con `forget`, un fallo quita el archivo de la lista de recientes. */
async function loadInto(path: string, forget: boolean): Promise<boolean> {
  try {
    const doc = await getApi().loadProject({ path });
    state().replaceDocument(doc, path);
    state().fitContent();
    void state().computePreview();
    useRecents.getState().add(path);
    state().notify('success', t('history.notice.opened', { name: baseName(path) }));
    return true;
  } catch (e) {
    const name = baseName(path);
    if (forget) {
      useRecents.getState().remove(path);
      state().notify('error', t('history.notice.recentMissing', { name, error: errorMessage(e) }));
    } else {
      state().notify('error', t('history.notice.openFailed', { name, error: errorMessage(e) }));
    }
    return false;
  }
}

/** Abre un proyecto; sin `path` muestra el diálogo. Devuelve `false` si se canceló o falló. */
export async function openProject(path?: string): Promise<boolean> {
  if (!(await ensureSaved('open'))) return false;
  const target = path ?? (await pickPath('openProject'));
  return target ? loadInto(target, false) : false;
}

/** Abre un archivo de la lista de recientes; si ya no se puede abrir, lo quita de la lista. */
export async function openRecent(path: string): Promise<boolean> {
  if (!(await ensureSaved('open'))) return false;
  return loadInto(path, true);
}

/** Guarda en la ruta actual, o pregunta una si no hay o se pidió "Guardar como". `true` si quedó guardado. */
export async function saveProject(asNew: boolean): Promise<boolean> {
  let path = state().filePath;
  if (asNew || !path) {
    const picked = await pickPath('saveProject', path ?? `${t('doc.untitled')}.tlaku`);
    if (!picked) return false;
    path = withExtension(picked, 'tlaku');
  }
  // Lo que se guarda es esta versión; si se edita durante el guardado, el documento sigue sucio.
  const saved = state().doc;
  try {
    await getApi().saveProject({ path, doc: saved });
  } catch (e) {
    state().notify('error', t('history.notice.saveFailed', { name: baseName(path), error: errorMessage(e) }));
    return false;
  }
  state().markSaved(path, saved);
  useRecents.getState().add(path);
  state().notify('success', t('notice.saved', { path }));
  return true;
}

/**
 * Importa un archivo con las opciones de fábrica. Si en Preferencias se cambió la resolución por
 * omisión (SVG sin unidades e imágenes), va por la ruta con opciones para aplicarla.
 */
async function importOne(path: string) {
  const dpi = effectiveImportDpi(null, state().prefs.importDpi);
  if (dpi === null) return getApi().importFile({ path });
  const { auto_group: _autoGroup, ...engineOptions } = DEFAULT_IMPORT_OPTIONS;
  return getApi().importFileWith({ path, options: { ...engineOptions, dpi } });
}

/**
 * Importa archivos de diseño uno tras otro. Las advertencias del importador salen siempre en un
 * aviso persistente (nunca en silencio). Lo importado queda seleccionado y la vista se ajusta a ello.
 * Devuelve cuántos objetos entraron.
 */
export async function importPaths(paths: string[]): Promise<number> {
  const ids: string[] = [];
  for (const path of paths) {
    const file = baseName(path);
    if (!isImportable(path)) {
      state().notify('error', t('history.notice.unsupported', { file }));
      continue;
    }
    try {
      const result = await importOne(path);
      const n = result.objects.length;
      if (n === 0) {
        state().notify('warning', t('history.notice.importedNothing', { file }), result.warnings);
        continue;
      }
      ids.push(...state().applyImport(result));
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

/** Diálogo de importación (permite varios archivos). */
export async function importDesign(): Promise<void> {
  const paths = await pickImportPaths();
  if (paths.length > 0) await importPaths(paths);
}

export async function exportGcodeFile(): Promise<boolean> {
  const base = state().filePath ? baseName(state().filePath!).replace(/\.[^.]+$/, '') : t('doc.untitled');
  const picked = await pickPath('exportGcode', `${base}.gcode`);
  if (!picked) return false;
  const path = withExtension(picked, 'gcode');
  try {
    const r = await getApi().exportGcode({ ...(await resolveJobArgs(buildJobArgs(state()))), path });
    const text = t('notice.exported', { lines: r.lines, time: fmtClock(r.estimate_s), path });
    if (r.warnings.length > 0) state().notify('warning', text, r.warnings);
    else state().notify('success', text);
    return true;
  } catch (e) {
    state().notify('error', t('history.notice.exportFailed', { error: errorMessage(e) }));
    return false;
  }
}
