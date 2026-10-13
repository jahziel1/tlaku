import { getApi, getApiKind } from '@/api';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { refreshFonts } from './fonts';

const FILTERS = [{ name: 'SHX', extensions: ['shx', 'SHX'] }];

/** Diálogo para elegir fuentes SHX. En modo mock devuelve una ruta de ejemplo; `null` si se cancela. */
export async function pickShxFiles(): Promise<string[] | null> {
  if (getApiKind() !== 'tauri') return ['ejemplos/romans.shx'];
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.open({ multiple: true, directory: false, filters: FILTERS, title: t('text.shx.pickTitle') });
  if (picked === null) return null;
  return Array.isArray(picked) ? picked : [picked];
}

/**
 * Importa fuentes SHX de AutoCAD: las elige el usuario, el motor las guarda y la lista de fuentes
 * se refresca. Devuelve los nombres añadidos (`[]` si se cancela o falla; el error se avisa).
 */
export async function importShxFonts(): Promise<string[]> {
  const notify = useStore.getState().notify;
  try {
    const paths = await pickShxFiles();
    if (!paths || paths.length === 0) return [];
    const names = await getApi().importShxFonts({ paths });
    await refreshFonts();
    notify('success', t('text.shx.imported', { names: names.join(', ') }));
    return names;
  } catch (e) {
    notify('error', t('text.shx.failed', { message: String(e instanceof Error ? e.message : e) }));
    return [];
  }
}
