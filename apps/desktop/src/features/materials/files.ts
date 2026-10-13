import { getApiKind } from '@/api';
import { t } from '@/i18n';

const FILTERS = [{ name: 'Biblioteca de materiales Tlaku', extensions: ['tlakumat'] }];

/** Descarga en el navegador (modo mock): no hay diálogo nativo. */
export function downloadText(name: string, text: string): string {
  const url = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  return name;
}

/** Nombre de archivo propuesto al exportar. */
export const DEFAULT_EXPORT_NAME = 'materiales.tlakumat';

/** Diálogo para elegir el `.tlakumat` a importar. En modo mock devuelve una ruta de ejemplo. */
export async function pickLibraryToOpen(): Promise<string | null> {
  if (getApiKind() !== 'tauri') return 'ejemplos/materiales.tlakumat';
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.open({ multiple: false, directory: false, filters: FILTERS, title: t('materials.file.importTitle') });
  return typeof picked === 'string' ? picked : null;
}

/** Diálogo para elegir dónde guardar el `.tlakumat`. Devuelve `null` si se cancela. */
export async function pickLibraryToSave(): Promise<string | null> {
  if (getApiKind() !== 'tauri') return DEFAULT_EXPORT_NAME;
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.save({ filters: FILTERS, defaultPath: DEFAULT_EXPORT_NAME, title: t('materials.file.exportTitle') });
  return picked ?? null;
}
