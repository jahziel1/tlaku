import { getApi, getApiKind } from '@/api';
import { commands } from '@/commands';
import { baseName, withExtension } from '@/features/history/paths';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';
import { applyBundle, buildBundle, BUNDLE_EXTENSION, DEFAULT_BUNDLE_NAME, DEFAULT_SHORTCUTS_NAME, isBundle, type BundleScope } from './bundle';

const FILTERS = [{ name: 'Preferencias de Tlaku', extensions: [BUNDLE_EXTENSION] }];
const state = () => useStore.getState();

/** Diálogo para elegir dónde guardar el paquete. En modo mock devuelve un nombre fijo. */
async function pickToSave(scope: BundleScope): Promise<string | null> {
  const name = scope === 'all' ? DEFAULT_BUNDLE_NAME : DEFAULT_SHORTCUTS_NAME;
  if (getApiKind() !== 'tauri') return name;
  const dialog = await import('@tauri-apps/plugin-dialog');
  return (await dialog.save({ filters: FILTERS, defaultPath: name, title: t('prefs.file.exportTitle') })) ?? null;
}

/** Diálogo para elegir el paquete a importar. En modo mock devuelve la ruta que usó la última exportación. */
async function pickToOpen(): Promise<string | null> {
  if (getApiKind() !== 'tauri') return lastMockPath;
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.open({ multiple: false, directory: false, filters: FILTERS, title: t('prefs.file.importTitle') });
  return typeof picked === 'string' ? picked : null;
}

let lastMockPath = DEFAULT_BUNDLE_NAME;

/** Exporta preferencias y atajos (`all`) o solo los atajos a un `.tlakuprefs`. `true` si se escribió. */
export async function exportPrefs(scope: BundleScope = 'all'): Promise<boolean> {
  const picked = await pickToSave(scope);
  if (!picked) return false;
  const path = withExtension(picked, BUNDLE_EXTENSION);
  const s = state();
  try {
    await getApi().exportPreferences({ path, bundle: buildBundle(s.prefs, s.shortcutOverrides, scope) });
  } catch (e) {
    s.notify('error', t('prefs.notice.exportFailed', { error: errorMessage(e) }));
    return false;
  }
  lastMockPath = path;
  s.notify('success', t('prefs.notice.exported', { path }));
  return true;
}

/**
 * Importa un `.tlakuprefs`. Con `shortcuts` solo toma los atajos. Lo que el paquete no trae se deja
 * como está; lo que no se entiende se descarta y se avisa. `true` si se aplicó.
 */
export async function importPrefs(scope: BundleScope = 'all'): Promise<boolean> {
  const path = await pickToOpen();
  if (!path) return false;
  const s = state();
  try {
    const bundle = await getApi().importPreferences({ path });
    if (!isBundle(bundle)) throw new Error(t('prefs.err.notBundle'));
    const known = new Set(commands.list().map((c) => c.id));
    const change = applyBundle(bundle, { prefs: s.prefs, shortcuts: s.shortcutOverrides }, known, scope);
    s.replacePrefs({ prefs: change.prefs, shortcuts: change.shortcuts }, false);
    const text = t('prefs.notice.imported', { name: baseName(path), prefs: change.appliedPrefs, shortcuts: change.appliedShortcuts });
    const problems = [
      change.ignoredPrefs > 0 && t('prefs.notice.ignoredPrefs', { n: change.ignoredPrefs }),
      change.unknownCommands > 0 && t('prefs.notice.unknownCommands', { n: change.unknownCommands }),
    ].filter((x): x is string => typeof x === 'string');
    s.notify(problems.length > 0 ? 'warning' : 'success', text, problems.length > 0 ? problems : undefined);
    return true;
  } catch (e) {
    s.notify('error', t('prefs.notice.importFailed', { name: baseName(path), error: errorMessage(e) }));
    return false;
  }
}
