import { getApi, getApiKind, type LaserApi } from '@/api';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { EMPTY_CSV, useVariablesUi, type CsvState } from './context';
import { variableTextOf } from './defaults';

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

const extraSeq = new Map<string, number>();

/**
 * Lee el CSV de la fuente secundaria `name` (en minúsculas) y deja su estado en la interfaz.
 * `null` borra el estado de esa fuente.
 */
export async function loadExtraCsv(name: string, path: string | null, api: () => Pick<LaserApi, 'readCsv'> = getApi): Promise<void> {
  const mine = (extraSeq.get(name) ?? 0) + 1;
  extraSeq.set(name, mine);
  const put = (state: CsvState | null) => {
    const { extra, setExtra } = useVariablesUi.getState();
    const next = { ...extra };
    if (state === null) delete next[name];
    else next[name] = state;
    setExtra(next);
  };
  if (path === null) return put(null);
  const prev = useVariablesUi.getState().extra[name];
  put({ path, table: prev?.path === path ? prev.table : null, error: null, loading: true });
  try {
    const table = await api().readCsv({ path });
    if (extraSeq.get(name) !== mine) return;
    put({ path, table, error: null, loading: false });
  } catch (e) {
    if (extraSeq.get(name) !== mine) return;
    put({ path, table: null, error: String(e instanceof Error ? e.message : e), loading: false });
  }
}

/** Vuelve a leer todas las fuentes del documento (botón «Recargar»). */
export function reloadAllCsv(): void {
  const { sources } = variableTextOf(useStore.getState().doc);
  void loadCsv(sources[0]?.path ?? null);
  sources.slice(1).forEach((s) => void loadExtraCsv(s.name.toLowerCase(), s.path));
}

/** Clave que cambia cuando cambia cualquier ruta de las fuentes (o sus nombres). */
function sourcesKey(): string {
  return variableTextOf(useStore.getState().doc)
    .sources.map((s) => `${s.name.toLowerCase()}\u0000${s.path}`)
    .join('\u0001');
}

/** Mantiene las fuentes cargadas al día con `variable_text.sources` del documento. Devuelve cómo parar. */
export function startCsvSync(): () => void {
  let prevMain: string | null | undefined;
  let prevKey: string | undefined;
  const check = () => {
    const { sources } = variableTextOf(useStore.getState().doc);
    const main = sources[0]?.path ?? null;
    if (main !== prevMain) {
      prevMain = main;
      void loadCsv(main);
    }
    const key = sourcesKey();
    if (key === prevKey) return;
    prevKey = key;
    const wanted = new Map(sources.slice(1).map((s) => [s.name.toLowerCase(), s.path]));
    const have = useVariablesUi.getState().extra;
    for (const name of Object.keys(have)) if (!wanted.has(name)) void loadExtraCsv(name, null);
    for (const [name, path] of wanted) if (have[name]?.path !== path) void loadExtraCsv(name, path);
  };
  check();
  const unsubscribe = useStore.subscribe(check);
  return unsubscribe;
}
