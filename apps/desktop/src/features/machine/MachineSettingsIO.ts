import { getApi, getApiKind } from '@/api';
import { baseName, withExtension } from '@/features/history/paths';
import { t } from '@/i18n';
import { parseErrorLine } from './grblCodes';
import { BACKUP_EXTENSION, parseRawSettings, type ParsedSettings } from './MachineSettingsLogic';

/** Lectura de `$$` y archivos de respaldo: lo único de esta ventana que habla con el API. */

/** Lee `$$` y conserva el texto de cada valor. Lanza con un texto legible si la máquina no contestó ajustes. */
export async function readMachineSettings(): Promise<ParsedSettings> {
  const reply = await getApi().machineCommand({ line: '$$' });
  const parsed = parseRawSettings(reply);
  if (parsed.settings.length === 0) {
    const bad = reply.find((l) => parseErrorLine(l) !== null);
    throw new Error(bad ?? t('machine.settings.empty'));
  }
  return parsed;
}

const FILTERS = () => [{ name: t('mset.file.filter'), extensions: [BACKUP_EXTENSION, 'txt'] }];
const MOCK_NAME = `ajustes-grbl.${BACKUP_EXTENSION}`;
let lastMockPath = MOCK_NAME;

/** Diálogo para elegir dónde guardar el respaldo. En modo mock devuelve un nombre fijo. */
export async function pickBackupToSave(): Promise<string | null> {
  if (getApiKind() !== 'tauri') return MOCK_NAME;
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.save({ filters: FILTERS(), defaultPath: MOCK_NAME, title: t('mset.file.saveTitle') });
  return picked ?? null;
}

/** Diálogo para elegir el respaldo a restaurar. En modo mock devuelve la ruta del último guardado. */
export async function pickBackupToOpen(): Promise<string | null> {
  if (getApiKind() !== 'tauri') return lastMockPath;
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.open({ multiple: false, directory: false, filters: FILTERS(), title: t('mset.file.openTitle') });
  return typeof picked === 'string' ? picked : null;
}

/** Guarda `text` donde elija la persona. Devuelve la ruta, o `null` si canceló. */
export async function saveBackupFile(text: string): Promise<string | null> {
  const picked = await pickBackupToSave();
  if (!picked) return null;
  // Si se escribió una extensión propia (p. ej. `.txt`) se respeta; el motor solo acepta `.tlakuset` y `.txt`.
  const path = withExtension(picked, BACKUP_EXTENSION);
  await getApi().writeSettingsBackup({ path, text });
  lastMockPath = path;
  return path;
}

/** Lee un respaldo elegido por la persona. Devuelve su ruta, su nombre y el texto, o `null` si canceló. */
export async function openBackupFile(): Promise<{ path: string; name: string; text: string } | null> {
  const path = await pickBackupToOpen();
  if (!path) return null;
  const text = await getApi().readSettingsBackup({ path });
  return { path, name: baseName(path), text };
}
