import { getApi, getApiKind } from '@/api';
import { baseName, withExtension } from '@/features/history/paths';
import { t } from '@/i18n';
import { BACKUP_EXTENSION } from './ruidaSettings';

/** Archivos `.tlakuruida` (respaldo manual de los ajustes de Ruida): diálogos y lectura/escritura. */

const FILTERS = () => [{ name: t('ruida.set.backup.fileFilter'), extensions: [BACKUP_EXTENSION] }];
const MOCK_NAME = `ajustes-ruida.${BACKUP_EXTENSION}`;
let lastMockPath = MOCK_NAME;

async function pickToSave(): Promise<string | null> {
  if (getApiKind() !== 'tauri') return MOCK_NAME;
  const dialog = await import('@tauri-apps/plugin-dialog');
  return (await dialog.save({ filters: FILTERS(), defaultPath: MOCK_NAME, title: t('ruida.set.backup.saveTitle') })) ?? null;
}

async function pickToOpen(): Promise<string | null> {
  if (getApiKind() !== 'tauri') return lastMockPath;
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.open({ multiple: false, directory: false, filters: FILTERS(), title: t('ruida.set.backup.openTitle') });
  return typeof picked === 'string' ? picked : null;
}

/** Guarda `text` donde elija la persona (siempre con la extensión `.tlakuruida`). Devuelve la ruta o `null` si canceló. */
export async function saveRuidaBackup(text: string): Promise<string | null> {
  const picked = await pickToSave();
  if (!picked) return null;
  const path = withExtension(picked, BACKUP_EXTENSION);
  await getApi().writeSettingsBackup({ path, text });
  lastMockPath = path;
  return path;
}

/** Lee el respaldo que elija la persona. `null` si canceló. */
export async function openRuidaBackup(): Promise<{ path: string; name: string; text: string } | null> {
  const path = await pickToOpen();
  if (!path) return null;
  const text = await getApi().readSettingsBackup({ path });
  return { path, name: baseName(path), text };
}
