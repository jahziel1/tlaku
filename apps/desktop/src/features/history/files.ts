import { getApi } from '@/api';
import type { Document } from '@/api/types';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { effectiveImportDpi } from '@/lib/importOptions';
import { fmtClock } from '@/lib/platform';
import { DEFAULT_IMPORT_OPTIONS } from '@/features/fileio/types';
import { resolveJobArgs } from '@/features/variables/jobResolve';
import { activeTabLabel, buildJobArgs, findTabByPath, isPristine, tabDoc, tabViews, useStore } from '@/store';
import { pickImportPaths, pickPath } from './dialogs';
import { baseName, isImportable, withExtension } from './paths';
import { askChoice } from './prompt';
import { useRecents } from './recent';
import { guardUnsaved, type GuardChoice, type GuardIntent, type GuardIo } from './unsaved';
import { captureBeforeSave } from '@/features/versions/actions';

const state = () => useStore.getState();

/** Nombre del documento. Sin argumento, el de la pestaña activa tal como la ve el usuario («Sin título 2» si hay varias pestañas). */
export function documentName(path?: string | null): string {
  if (path === undefined) return activeTabLabel(state());
  return path ? baseName(path) : t('doc.untitled');
}

/** Pregunta guardar, descartar o cancelar con el diálogo propio de la app. `name`: el documento por el que se pregunta. */
export async function askUnsaved(intent: GuardIntent, name: string = documentName()): Promise<GuardChoice> {
  const closeTab = intent === 'tab';
  const id = await askChoice({
    title: closeTab ? t('tabs.close.title') : t('history.unsaved.title'),
    message: closeTab ? t('tabs.close.ask', { name }) : t(`history.unsaved.ask.${intent}`, { name }),
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

/** «Nuevo»: una pestaña vacía (si la activa ya lo está, se queda en ella). No hace falta preguntar nada: no se pierde ningún documento. */
export async function newProject(): Promise<boolean> {
  if (!isPristine(state())) state().newTab();
  void state().computePreview();
  return true;
}

/** Carga `path` en una pestaña. Si ya está abierto, activa su pestaña. Con `forget`, un fallo quita el archivo de la lista de recientes. */
async function loadInto(path: string, forget: boolean): Promise<boolean> {
  const openTab = findTabByPath(state(), path);
  if (openTab) return showOpenTab(openTab, path);
  try {
    const doc = await getApi().loadProject({ path });
    // Mientras se leía el archivo pudo abrirse en otra pestaña (dos aperturas seguidas).
    const again = findTabByPath(state(), path);
    if (again) return showOpenTab(again, path);
    state().openInTab(doc, path);
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

function showOpenTab(id: string, path: string): boolean {
  state().activateTab(id);
  state().notify('info', t('tabs.alreadyOpen', { name: baseName(path) }));
  return true;
}

/** Abre un proyecto en una pestaña; sin `path` muestra el diálogo. Devuelve `false` si se canceló o falló. */
export async function openProject(path?: string): Promise<boolean> {
  const target = path ?? (await pickPath('openProject'));
  return target ? loadInto(target, false) : false;
}

/** Abre un archivo de la lista de recientes; si ya no se puede abrir, lo quita de la lista. */
export async function openRecent(path: string): Promise<boolean> {
  return loadInto(path, true);
}

/**
 * Guarda la pestaña activa en su ruta, o pregunta una si no tiene o se pidió «Guardar como». `true` si quedó guardado.
 * La pestaña se identifica al empezar: si el usuario cambia de pestaña mientras se elige el archivo o se escribe,
 * el guardado sigue siendo el de la pestaña que lo pidió.
 */
export async function saveProject(asNew: boolean): Promise<boolean> {
  const tabId = state().activeTabId;
  let path = state().filePath;
  if (asNew || !path) {
    const picked = await pickPath('saveProject', path ?? `${t('doc.untitled')}.tlaku`);
    if (!picked) return false;
    path = withExtension(picked, 'tlaku');
  }
  // Lo que se guarda es esta versión; si se edita durante el guardado, el documento sigue sucio.
  const saved = tabDoc(state(), tabId);
  if (!saved) return false; // la pestaña se cerró mientras tanto
  return writeTab(tabId, path, saved);
}

/** Escribe `doc` en `path` y, si la pestaña `tabId` sigue abierta, la marca guardada. */
async function writeTab(tabId: string, path: string, saved: Document): Promise<boolean> {
  // Historial de versiones: conserva lo que tiene ahora el archivo. No puede fallar ni retrasar el guardado (avisa y sigue).
  await captureBeforeSave(path);
  try {
    await getApi().saveProject({ path, doc: saved });
  } catch (e) {
    state().notify('error', t('history.notice.saveFailed', { name: baseName(path), error: errorMessage(e) }));
    return false;
  }
  state().runInTab(tabId, () => state().markSaved(path, saved));
  useRecents.getState().add(path);
  state().notify('success', t('notice.saved', { path }));
  return true;
}

/**
 * Guarda la pestaña `tabId` (activa o no). Una pestaña de fondo que ya tiene archivo se guarda sin moverse;
 * si hace falta elegir archivo se activa primero, para que se vea qué se guarda.
 */
export async function saveTab(tabId: string, asNew = false): Promise<boolean> {
  if (tabId === state().activeTabId) return saveProject(asNew);
  const view = tabViews(state()).find((v) => v.id === tabId);
  const doc = tabDoc(state(), tabId);
  if (!view || !doc) return false;
  if (asNew || !view.filePath) {
    state().activateTab(tabId);
    return saveProject(asNew);
  }
  return writeTab(tabId, view.filePath, doc);
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
  // El destino es la pestaña activa AL EMPEZAR: cada archivo se aplica ahí aunque el usuario cambie de pestaña mientras se lee.
  const tabId = state().activeTabId;
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
      const placed = state().runInTab(tabId, () => state().applyImport(result));
      if (!placed) {
        state().notify('warning', t('tabs.targetClosed', { file }));
        break;
      }
      ids.push(...placed);
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
    state().runInTab(tabId, () => {
      state().select(ids);
      state().fitContent();
    });
  }
  return ids.length;
}

/** Diálogo de importación (permite varios archivos). */
export async function importDesign(): Promise<void> {
  const paths = await pickImportPaths();
  if (paths.length > 0) await importPaths(paths);
}

export async function exportGcodeFile(): Promise<boolean> {
  // Se exporta la pestaña activa al pedirlo, no la que esté activa cuando se cierre el selector de archivo.
  const args = buildJobArgs(state());
  const base = state().filePath ? baseName(state().filePath!).replace(/\.[^.]+$/, '') : t('doc.untitled');
  const picked = await pickPath('exportGcode', `${base}.gcode`);
  if (!picked) return false;
  const path = withExtension(picked, 'gcode');
  try {
    const r = await getApi().exportGcode({ ...(await resolveJobArgs(args)), path });
    const text = t('notice.exported', { lines: r.lines, time: fmtClock(r.estimate_s), path });
    if (r.warnings.length > 0) state().notify('warning', text, r.warnings);
    else state().notify('success', text);
    return true;
  } catch (e) {
    state().notify('error', t('history.notice.exportFailed', { error: errorMessage(e) }));
    return false;
  }
}
