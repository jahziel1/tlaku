import { getApi, getApiKind, type LaserApi } from '@/api';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { EMPTY_CSV, useVariablesUi } from './context';

const FILTERS = [{ name: 'CSV', extensions: ['csv', 'CSV', 'txt', 'tsv'] }];

/** Diálogo para elegir el CSV. En modo mock devuelve una ruta de ejemplo; `null` si se cancela. */
export async function pickCsvFile(): Promise<string | null> {
  if (getApiKind() !== 'tauri') return 'ejemplos/etiquetas.csv';
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.open({ multiple: false, directory: false, filters: FILTERS, title: t('variables.csv.pickTitle') });
  return typeof picked === 'string' ? picked : null;
}

let seq = 0;

/** Lee el CSV de `path` y lo deja en el estado de la interfaz (o el error). `null` lo descarta. */
export async function loadCsv(path: string | null, api: () => Pick<LaserApi, 'readCsv'> = getApi): Promise<void> {
  const mine = ++seq;
  const ui = useVariablesUi.getState();
  if (path === null) return ui.setCsv(EMPTY_CSV);
  // Se conserva la tabla anterior mientras se lee la nueva, para que el lienzo no parpadee.
  ui.setCsv({ path, table: ui.csv.path === path ? ui.csv.table : null, error: null, loading: true });
  try {
    const table = await api().readCsv({ path });
    if (mine !== seq) return;
    useVariablesUi.getState().setCsv({ path, table, error: null, loading: false });
  } catch (e) {
    if (mine !== seq) return;
    useVariablesUi.getState().setCsv({ path, table: null, error: String(e instanceof Error ? e.message : e), loading: false });
  }
}

/** Mantiene el CSV cargado al día con `variable_text.csv_path` del documento. Devuelve cómo parar. */
export function startCsvSync(): () => void {
  let prev: string | null | undefined;
  const check = () => {
    const path = useStore.getState().doc.variable_text?.csv_path ?? null;
    if (path === prev) return;
    prev = path;
    void loadCsv(path);
  };
  check();
  const unsubscribe = useStore.subscribe(check);
  return unsubscribe;
}
