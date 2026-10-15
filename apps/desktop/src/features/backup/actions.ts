import { getApi, getApiKind, type BackupConfig, type BackupEntry, type BackupList, type BackupSettings } from '@/api';
import { isMainWindow } from '@/features/help/updater';
import { allSections } from '@/features/bundle/model';
import { buildCurrentBundle } from '@/features/bundle/sources';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';
import { useBackup } from './store';

/** Carpeta que devuelve el selector en modo mock (no hay diálogo nativo). */
export const MOCK_PICKED_FOLDER = '/mock/Respaldos';

const DAY_MS = 24 * 60 * 60 * 1000;
/** Tiempo máximo que se espera el respaldo al cerrar: una nube lenta no debe dejar la ventana abierta. */
export const CLOSE_BACKUP_TIMEOUT_MS = 8000;

export type BackupReason = 'manual' | 'schedule' | 'close';

export async function loadBackupConfig(): Promise<BackupConfig> {
  const config = await getApi().backupConfigGet();
  useBackup.setState({ config });
  return config;
}

export async function refreshBackupList(): Promise<BackupList> {
  const list = await getApi().backupList();
  useBackup.setState({ list });
  return list;
}

/** Guarda lo elegido en el diálogo (el motor lo acota) y devuelve la configuración resultante. */
export async function saveBackupSettings(patch: Partial<BackupSettings>): Promise<BackupConfig> {
  const current = useBackup.getState().config ?? (await loadBackupConfig());
  const settings: BackupSettings = {
    folder: patch.folder === undefined ? current.folder : patch.folder,
    on_close: patch.on_close ?? current.on_close,
    every_days: patch.every_days ?? current.every_days,
    keep: patch.keep ?? current.keep,
    include_history: patch.include_history ?? current.include_history,
  };
  const config = await getApi().backupConfigSet({ settings });
  useBackup.setState({ config });
  if (patch.folder !== undefined) await refreshBackupList();
  return config;
}

/** Diálogo para elegir la carpeta de respaldo. En modo mock devuelve una carpeta fija. */
export async function pickBackupFolder(): Promise<string | null> {
  if (getApiKind() !== 'tauri') return MOCK_PICKED_FOLDER;
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.open({ directory: true, multiple: false, title: t('bundle.backup.pickTitle') });
  return typeof picked === 'string' ? picked : null;
}

/** ¿Toca un respaldo programado? (cada N días desde el último; nunca si N es 0 o no hay carpeta). */
export function isBackupDue(config: Pick<BackupConfig, 'folder' | 'every_days' | 'last_backup_ms'>, now: number): boolean {
  if (!config.folder || config.every_days <= 0) return false;
  return config.last_backup_ms === 0 || now - config.last_backup_ms >= config.every_days * DAY_MS;
}

let warned = false;
/** Para las pruebas: vuelve a avisar la próxima vez. */
export const resetBackupWarning = (): void => {
  warned = false;
};

/** Avisa una sola vez por sesión que la carpeta no está disponible (o que el respaldo falló) y sigue. */
function warnOnce(config: BackupConfig | null, error: string | null): void {
  if (warned) return;
  warned = true;
  const text = error ? t('bundle.backup.failedWarn', { error }) : t('bundle.backup.unavailableWarn', { folder: config?.folder ?? '' });
  useStore.getState().notify('warning', text);
}

/** Secciones del respaldo: todo, y el historial de trabajos solo si la persona lo pidió. */
export const backupSections = (config: Pick<BackupConfig, 'include_history'>) => ({ ...allSections(), history: config.include_history });

/**
 * Hace un respaldo a la carpeta elegida. Los automáticos (`schedule`, `close`) no hacen ruido: si falla
 * avisan una vez y siguen; el manual muestra el resultado o el error. Mientras hay un respaldo más
 * nuevo de otra computadora sin revisar, los automáticos no corren (no taparían su detección).
 */
export async function runBackup(reason: BackupReason): Promise<boolean> {
  const ui = useBackup.getState();
  const notify = useStore.getState().notify;
  const config = ui.config ?? (await loadBackupConfig());
  if (!config.folder) {
    if (reason === 'manual') notify('error', t('bundle.backup.needFolder'));
    return false;
  }
  if (reason !== 'manual' && ui.newer) return false;
  if (ui.busy) return false;
  useBackup.setState({ busy: true });
  try {
    const bundle = await buildCurrentBundle(backupSections(config));
    const written = await getApi().backupRun({ bundle });
    warned = false;
    await loadBackupConfig();
    await refreshBackupList();
    if (reason === 'manual') notify('success', t('bundle.backup.done', { name: written.name }));
    return true;
  } catch (e) {
    const error = errorMessage(e);
    if (reason === 'manual') notify('error', t('bundle.backup.failed', { error }));
    else warnOnce(config, error);
    return false;
  } finally {
    useBackup.setState({ busy: false });
  }
}

/** ¿Hay que respaldar al cerrar? (decisión inmediata, con la configuración ya leída). */
export function shouldBackupOnClose(): boolean {
  const s = useBackup.getState();
  return !!s.config?.on_close && !!s.config.folder && !s.newer;
}

/** Respaldo al cerrar la ventana principal, con tiempo máximo. Nunca lanza. */
export async function backupOnClose(): Promise<void> {
  try {
    if (!shouldBackupOnClose() || !(await isMainWindow())) return;
    await Promise.race([runBackup('close'), new Promise<void>((resolve) => setTimeout(resolve, CLOSE_BACKUP_TIMEOUT_MS))]);
  } catch {
    // Cerrar la aplicación no puede depender del respaldo.
  }
}

/**
 * Al abrir: lee la configuración y, con carpeta, revisa primero si hay un respaldo más nuevo de otra
 * computadora (se ofrece, nunca se aplica solo) y solo si no lo hay hace el respaldo programado.
 */
export async function runBackupStartup(now: number = Date.now()): Promise<void> {
  if (!(await isMainWindow())) return;
  const config = await loadBackupConfig();
  if (!config.folder) return;
  const list = await refreshBackupList();
  if (!list.available) {
    warnOnce(config, null);
    return;
  }
  const newer = await getApi().backupCheckNewer();
  if (newer) {
    useBackup.setState({ newer });
    return;
  }
  if (isBackupDue(config, now)) await runBackup('schedule');
}

/** Anota un respaldo como visto (importado o descartado) y deja de ofrecerlo. */
export async function markBackupSeen(entry: Pick<BackupEntry, 'created_ms'>): Promise<void> {
  await getApi().backupMarkSeen({ created_ms: entry.created_ms });
  if (useBackup.getState().newer?.created_ms === entry.created_ms) useBackup.setState({ newer: null });
  await loadBackupConfig();
}
