import { getApi } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { normalizeLayers } from '@/lib/layers';
import { baseName } from '@/features/history/paths';
import { pickPath } from '@/features/history/dialogs';
import { askChoice } from '@/features/history/prompt';
import { findTabByPath, tabDoc, useStore } from '@/store';
import { formatCopyDate } from '@/features/recovery/format';
import { useVersionsUi } from './versionsUi';
import type { VersionInfo } from './types';

const state = () => useStore.getState();

/** Para avisar una sola vez por racha de fallos (si el disco falla en cada guardado no se llena la pantalla de avisos). */
let captureFailing = false;

/** Para las pruebas: vuelve a avisar en el próximo fallo. */
export function resetCaptureWarning(): void {
  captureFailing = false;
}

/**
 * Justo antes de sobrescribir `path`: el motor conserva lo que tiene ahora el archivo como versión. NUNCA lanza ni
 * detiene el guardado: si el disco falla se avisa y el proyecto se guarda igual.
 */
export async function captureBeforeSave(path: string): Promise<void> {
  try {
    await getApi().versionsCapture({ path });
    captureFailing = false;
  } catch (e) {
    if (!captureFailing) state().notify('warning', t('versions.notice.captureFailed', { error: errorMessage(e) }));
    captureFailing = true;
  }
}

/** Abre el historial del proyecto de la pestaña activa (o, si no tiene archivo, solo la lista de otros proyectos). */
export function openHistory(path?: string | null): void {
  useVersionsUi.getState().openFor(path === undefined ? state().filePath : path);
}

/** Pide un archivo `.tlaku` y abre su historial, esté o no abierto. */
export async function openHistoryOfFile(): Promise<void> {
  const picked = await pickPath('openProject');
  if (picked) useVersionsUi.getState().openFor(picked);
}

/** «Guardar versión…»: pide el nombre. Un documento sin archivo no tiene historial (lo cubre la recuperación). */
export function openSaveVersion(): void {
  if (!state().filePath) {
    state().notifyKey('info', 'versions.notice.needFile');
    return;
  }
  useVersionsUi.getState().openName();
}

/** Guarda la pestaña activa como versión con nombre. `true` si quedó guardada. */
export async function saveNamedVersion(name: string): Promise<boolean> {
  const tabId = state().activeTabId;
  const path = state().filePath;
  const doc = tabDoc(state(), tabId);
  if (!path || !doc) return false;
  try {
    await getApi().versionsSnapshot({ path, doc, kind: 'manual', label: name });
  } catch (e) {
    state().notify('error', t('versions.notice.saveFailed', { error: errorMessage(e) }));
    return false;
  }
  useVersionsUi.getState().bump();
  state().notify('success', t('versions.notice.saved', { name: name.trim() }));
  return true;
}

/** Nombre corto de una versión para los avisos: su nombre o su fecha. */
export function versionTitle(v: Pick<VersionInfo, 'label' | 'created_ms'>): string {
  return v.label ?? formatCopyDate(v.created_ms);
}

/** Abre la versión en una pestaña nueva, sin archivo y con cambios sin guardar (guárdala con otro nombre). El documento actual no se toca. */
export async function restoreAsNewTab(path: string, version: VersionInfo): Promise<boolean> {
  try {
    const doc = await getApi().versionsRead({ path, id: version.id });
    state().openInTab(doc, null, { unsaved: true });
    state().fitContent();
    void state().computePreview();
    state().notify('success', t('versions.notice.openedCopy', { when: versionTitle(version), name: baseName(path) }));
    return true;
  } catch (e) {
    state().notify('error', t('versions.notice.restoreFailed', { error: errorMessage(e) }));
    return false;
  }
}

/**
 * Reemplaza el documento de la pestaña que tiene `path` por la versión, tras confirmar. Antes guarda el estado
 * actual como versión («antes de restaurar») y el reemplazo es UN paso de deshacer. `true` si se restauró.
 */
export async function restoreInPlace(path: string, version: VersionInfo): Promise<boolean> {
  const tabId = findTabByPath(state(), path);
  if (!tabId) {
    state().notifyKey('warning', 'versions.notice.notOpen');
    return false;
  }
  const choice = await askChoice({
    title: t('versions.replace.title'),
    message: t('versions.replace.message', { when: versionTitle(version), name: baseName(path) }),
    choices: [
      { id: 'restore', label: t('versions.replace.confirm'), tone: 'accent' },
      { id: 'cancel', label: t('versions.replace.cancel') },
    ],
    cancelId: 'cancel',
  });
  if (choice !== 'restore') return false;
  try {
    const restored = normalizeLayers(await getApi().versionsRead({ path, id: version.id }));
    // La pestaña pudo cerrarse o cambiar mientras se pedía la confirmación y se leía la versión.
    const current = tabDoc(state(), tabId);
    if (!current) {
      state().notifyKey('warning', 'tabs.targetClosedGeneric');
      return false;
    }
    // Si no se puede guardar el estado actual, no se reemplaza nada: sería perder trabajo sin vuelta atrás.
    await getApi().versionsSnapshot({ path, doc: current, kind: 'restore' });
    const done = state().runInTab(tabId, () => {
      state().applyDocChange('versions.history.restore', () => restored);
      state().clearSelection();
      void state().computePreview();
      return true;
    });
    if (!done) {
      state().notifyKey('warning', 'tabs.targetClosedGeneric');
      return false;
    }
    state().notify('success', t('versions.notice.restored', { when: versionTitle(version) }));
    return true;
  } catch (e) {
    state().notify('error', t('versions.notice.restoreFailed', { error: errorMessage(e) }));
    return false;
  }
}

/** Selecciona en el lienzo de la pestaña de `path` los objetos indicados (los que difieren de la versión). */
export function highlightInCanvas(path: string, ids: string[]): boolean {
  const tabId = findTabByPath(state(), path);
  if (!tabId || ids.length === 0) return false;
  state().activateTab(tabId);
  state().select(ids);
  return true;
}
