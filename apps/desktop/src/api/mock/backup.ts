import type { BackupConfig, BackupEntry, BackupList, BackupSettings, LaserApi, UserBundle } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

export const MOCK_BACKUP_FOLDER = '/mock/Respaldos';

type BackupApi = Pick<LaserApi, 'backupConfigGet' | 'backupConfigSet' | 'backupRun' | 'backupSafety' | 'backupList' | 'backupCheckNewer' | 'backupMarkSeen'>;

/** Control del respaldo simulado, para pruebas y capturas: la unidad y «otra computadora». */
export interface MockBackupSim {
  /** `false` simula una unidad desconectada o una nube sin sincronizar. */
  folderAvailable: boolean;
  /** Deja en la carpeta un respaldo hecho por otra instalación (lo hace parecer de otra computadora). */
  addForeign(bundle: UserBundle, createdMs?: number): BackupEntry;
  /** Cuántos respaldos locales previos a importar hay. */
  safetyCount(): number;
  /** Nombres de los respaldos que hay en la carpeta, más nuevo primero. */
  names(): string[];
}

const SECRET = /(token|secret|password|passwd|passphrase|apikey|api_key|private_key|authorization|cookie|credentials?)$/i;

function hasSecret(value: unknown, free = false, top = true): boolean {
  if (Array.isArray(value)) return value.some((v) => hasSecret(v, free, false));
  if (value && typeof value === 'object') {
    return Object.entries(value).some(([k, v]) => (SECRET.test(k) && v !== null) || hasSecret(v, free || (top && (k === 'templates' || k === 'history')), false));
  }
  return typeof value === 'string' && !free && /^[a-z][a-z0-9+.-]*:\/\/[^/?#]*@/i.test(value);
}

const pad = (n: number) => String(n).padStart(2, '0');
function stamp(prefix: string, ms: number): string {
  const d = new Date(ms);
  return `${prefix}-${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())}_${pad(d.getUTCHours())}-${pad(d.getUTCMinutes())}-${pad(d.getUTCSeconds())}`;
}

/**
 * Respaldo a carpeta en memoria (modo mock): se comporta como `src-tauri/src/backup/mod.rs`: nombres
 * fechados, rotación, rechazo de secretos, carpeta ausente sin crearla, respaldo previo a importar y
 * detección de un respaldo más nuevo de otra instalación. Los archivos se guardan en `store` (el mismo
 * de `mockBundle`), así `importUserBundle` los lee por su ruta.
 */
export function mockBackup(lag: Lag, store: Map<string, UserBundle>, now: () => number = Date.now): { api: BackupApi; sim: MockBackupSim } {
  const config: BackupConfig = { version: 1, folder: null, on_close: false, every_days: 0, keep: 10, include_history: false, last_backup_ms: 0, last_seen_ms: 0, install_id: 'mock-instalacion' };
  const files = new Map<string, { entry: BackupEntry; owner: string }>();
  const safety: string[] = [];

  const sorted = () => [...files.values()].sort((a, b) => b.entry.created_ms - a.entry.created_ms || b.entry.name.localeCompare(a.entry.name));

  function put(bundle: UserBundle, prefix: string, ms: number, owner: string): BackupEntry {
    let name = `${stamp(prefix, ms)}.tlakubundle`;
    for (let n = 2; files.has(name); n++) name = `${stamp(prefix, ms)}_${n}.tlakubundle`;
    const path = `${MOCK_BACKUP_FOLDER}/${name}`;
    const copy = structuredClone(bundle);
    copy.install_id = owner;
    store.set(path, copy);
    const entry: BackupEntry = { name, path, size_bytes: JSON.stringify(copy).length, created_ms: Math.floor(ms / 1000) * 1000 };
    files.set(name, { entry, owner });
    return entry;
  }

  const sim: MockBackupSim = {
    folderAvailable: true,
    addForeign: (bundle, createdMs = now()) => put(bundle, 'tlaku-respaldo', createdMs, 'otra-computadora'),
    safetyCount: () => safety.length,
    names: () => sorted().map((f) => f.entry.name),
  };

  const check = (bundle: UserBundle): void => {
    if (bundle?.format !== 'tlaku-bundle') throw new Error('El archivo no es un paquete de Tlaku (.tlakubundle).');
    if (bundle.version !== 1 && bundle.version !== 2) throw new Error(`Versión de paquete no soportada: ${String(bundle.version)} (esta aplicación entiende la 2).`);
    if (hasSecret(bundle)) throw new Error('El respaldo se rechazó porque contiene un dato que parece un secreto. Los respaldos nunca llevan llaves, contraseñas ni tokens.');
  };

  const list = (): BackupList => {
    if (!config.folder) return { configured: false, available: false, entries: [] };
    if (!sim.folderAvailable) return { configured: true, available: false, entries: [] };
    return { configured: true, available: true, entries: sorted().map((f) => ({ ...f.entry })) };
  };

  const api: BackupApi = {
    backupConfigGet: () => lag(() => ({ ...config })),
    backupConfigSet: ({ settings }: { settings: BackupSettings }) =>
      lag(() => {
        const folder = settings.folder?.trim() || null;
        Object.assign(config, { ...settings, folder, keep: Math.min(100, Math.max(1, Math.round(settings.keep))), every_days: Math.min(365, Math.max(0, Math.round(settings.every_days))) });
        return { ...config };
      }),
    backupRun: ({ bundle }) =>
      lag(() => {
        if (!config.folder) throw new Error('Elige primero la carpeta donde guardar los respaldos.');
        if (!sim.folderAvailable) throw new Error(`La carpeta de respaldo no está disponible: ${config.folder}. Revisa que la unidad esté conectada o que la nube haya terminado de sincronizar.`);
        check(bundle);
        const entry = put(bundle, 'tlaku-respaldo', now(), config.install_id);
        const removed: string[] = [];
        for (const f of sorted().slice(config.keep)) {
          files.delete(f.entry.name);
          store.delete(f.entry.path);
          removed.push(f.entry.name);
        }
        config.last_backup_ms = entry.created_ms;
        return { name: entry.name, path: entry.path, created_ms: entry.created_ms, removed };
      }),
    backupSafety: ({ bundle }) =>
      lag(() => {
        check(bundle);
        const name = `${stamp('tlaku-antes-de-importar', now())}.tlakubundle`;
        safety.unshift(name);
        safety.length = Math.min(safety.length, 5);
        return { name, path: `/mock/respaldos-locales/${name}`, created_ms: now(), removed: [] };
      }),
    backupList: () => lag(list),
    backupCheckNewer: () =>
      lag(() => {
        const newest = list().entries[0];
        if (!newest || newest.created_ms <= Math.max(config.last_backup_ms, config.last_seen_ms)) return null;
        return files.get(newest.name)?.owner === config.install_id ? null : newest;
      }),
    backupMarkSeen: ({ created_ms }) =>
      lag(() => {
        config.last_seen_ms = Math.max(config.last_seen_ms, created_ms);
      }),
  };
  return { api, sim };
}
