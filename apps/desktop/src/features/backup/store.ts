import { create } from 'zustand';
import type { BackupConfig, BackupEntry, BackupList } from '@/api';

interface BackupState {
  /** Configuración leída del motor; `null` hasta que se lee. */
  config: BackupConfig | null;
  /** Respaldos de la carpeta; `null` hasta que se lee. */
  list: BackupList | null;
  /** Hay un respaldo en curso. */
  busy: boolean;
  /** Respaldo más nuevo de otra computadora que se ofrece importar; `null` si no hay (o ya se vio). */
  newer: BackupEntry | null;
  /** La pregunta «hay un respaldo más nuevo» está en pantalla. */
  promptOpen: boolean;
}

/** Estado del respaldo a carpeta (no es parte del documento ni se guarda: el motor lo guarda en `backup.json`). */
export const useBackup = create<BackupState>()(() => ({ config: null, list: null, busy: false, newer: null, promptOpen: false }));

/** Para las pruebas. */
export const resetBackupStore = (): void => useBackup.setState({ config: null, list: null, busy: false, newer: null, promptOpen: false });
