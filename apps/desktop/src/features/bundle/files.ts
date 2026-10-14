import { getApi, getApiKind, type UserBundle } from '@/api';
import { loadUserArt } from '@/features/art/actions';
import { useArtPanel } from '@/features/art/artStore';
import { loadCameraSetup } from '@/features/camera/actions';
import { baseName, withExtension } from '@/features/history/paths';
import { useMaterials } from '@/features/materials/state';
import { loadMaterials } from '@/features/materials/actions';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';
import { buildUserBundle, DEFAULT_USER_BUNDLE_NAME, isUserBundle, USER_BUNDLE_EXTENSION, type SectionChoice } from './model';

const FILTERS = () => [{ name: t('bundle.file.filter'), extensions: [USER_BUNDLE_EXTENSION] }];

/** Última ruta usada en modo mock (no hay diálogo nativo): sirve para importar lo que se exportó. */
let lastMockPath = DEFAULT_USER_BUNDLE_NAME;

/** Diálogo para elegir dónde guardar el paquete. En modo mock devuelve un nombre fijo. */
export async function pickBundleToSave(): Promise<string | null> {
  if (getApiKind() !== 'tauri') return DEFAULT_USER_BUNDLE_NAME;
  const dialog = await import('@tauri-apps/plugin-dialog');
  return (await dialog.save({ filters: FILTERS(), defaultPath: DEFAULT_USER_BUNDLE_NAME, title: t('bundle.file.exportTitle') })) ?? null;
}

/** Diálogo para elegir el paquete a importar. En modo mock devuelve la ruta de la última exportación. */
export async function pickBundleToOpen(): Promise<string | null> {
  if (getApiKind() !== 'tauri') return lastMockPath;
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.open({ multiple: false, directory: false, filters: FILTERS(), title: t('bundle.file.importTitle') });
  return typeof picked === 'string' ? picked : null;
}

/** Exporta las secciones elegidas a un `.tlakubundle`. Devuelve si se escribió. */
export async function exportUserBundle(sections: SectionChoice): Promise<boolean> {
  const picked = await pickBundleToSave();
  if (!picked) return false;
  const path = withExtension(picked, USER_BUNDLE_EXTENSION);
  const s = useStore.getState();
  try {
    if (sections.materials) await loadMaterials();
    if (sections.cameras) await loadCameraSetup();
    if (sections.art) await loadUserArt();
    const app = await getApi().appVersion().catch(() => '');
    const bundle = buildUserBundle(
      {
        devices: s.devices,
        activeDeviceId: s.activeDeviceId,
        library: useMaterials.getState().library,
        prefs: s.prefs,
        shortcuts: s.shortcutOverrides,
        cameras: useStore.getState().cameraSetup,
        userArt: useArtPanel.getState().user,
      },
      { sections, app },
    );
    await getApi().exportUserBundle({ path, bundle });
  } catch (e) {
    s.notify('error', t('bundle.notice.exportFailed', { error: errorMessage(e) }));
    return false;
  }
  lastMockPath = path;
  s.notify('success', t('bundle.notice.exported', { path }));
  return true;
}

/** Lee un paquete del disco y comprueba su sobre. Lanza con un mensaje listo para mostrar. */
export async function readUserBundle(path: string): Promise<UserBundle> {
  const bundle = await getApi().importUserBundle({ path });
  if (!isUserBundle(bundle)) throw new Error(t('bundle.err.notBundle'));
  return bundle;
}

export { baseName };
