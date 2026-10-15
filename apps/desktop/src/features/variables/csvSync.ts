import { getApi, getApiKind, type LaserApi } from '@/api';
import { t } from '@/i18n';
import { pinTab, useStore } from '@/store';
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

/**
 * Las tablas leídas son POR PESTAÑA (ver `context.ts`). Cada lectura fija la pestaña donde empezó
 * (`pinTab`) y su resultado se escribe en ella con `runInTab`, aunque el usuario ya esté en otra; si esa
 * pestaña se cerró, se descarta. El número de lectura también es por pestaña: una lectura nueva de la
 * misma fuente en la misma pestaña invalida la anterior, pero nunca la de otra pestaña.
 */
const seqs = new Map<string, number>();
const bump = (key: string): number => {
  const n = (seqs.get(key) ?? 0) + 1;
  seqs.set(key, n);
  return n;
};

const errorText = (e: unknown): string => String(e instanceof Error ? e.message : e);

/** Lee el CSV de `path` y lo deja en el estado de la interfaz de la pestaña activa (o el error). `null` lo descarta. */
export async function loadCsv(path: string | null, api: () => Pick<LaserApi, 'readCsv'> = getApi): Promise<void> {
  const s0 = useStore.getState();
  const inTab = pinTab(s0);
  const key = `${s0.activeTabId}\u0000main`;
  const mine = bump(key);
  const ui = useVariablesUi.getState();
  if (path === null) return ui.setCsv(EMPTY_CSV);
  // Se conserva la tabla anterior mientras se lee la nueva, para que el lienzo no parpadee.
  ui.setCsv({ path, table: ui.csv.path === path ? ui.csv.table : null, error: null, loading: true });
  const land = (state: CsvState) =>
    inTab(() => {
      // Si esa pestaña ya apunta a otro archivo, la respuesta no es suya.
      if (useVariablesUi.getState().csv.path === path) useVariablesUi.getState().setCsv(state);
    });
  try {
    const table = await api().readCsv({ path });
    if (seqs.get(key) !== mine) return;
    land({ path, table, error: null, loading: false });
  } catch (e) {
    if (seqs.get(key) !== mine) return;
    land({ path, table: null, error: errorText(e), loading: false });
  }
}

/**
 * Lee el CSV de la fuente secundaria `name` (en minúsculas) y deja su estado en la interfaz de la pestaña
 * activa. `null` borra el estado de esa fuente.
 */
export async function loadExtraCsv(name: string, path: string | null, api: () => Pick<LaserApi, 'readCsv'> = getApi): Promise<void> {
  const s0 = useStore.getState();
  const inTab = pinTab(s0);
  const key = `${s0.activeTabId}\u0000extra\u0000${name}`;
  const mine = bump(key);
  const put = (state: CsvState | null, onlyIfPath?: string) => {
    const { extra, setExtra } = useVariablesUi.getState();
    if (onlyIfPath !== undefined && extra[name]?.path !== onlyIfPath) return;
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
    if (seqs.get(key) !== mine) return;
    inTab(() => put({ path, table, error: null, loading: false }, path));
  } catch (e) {
    if (seqs.get(key) !== mine) return;
    inTab(() => put({ path, table: null, error: errorText(e), loading: false }, path));
  }
}

/** Vuelve a leer todas las fuentes del documento (botón «Recargar»). */
export function reloadAllCsv(): void {
  const { sources } = variableTextOf(useStore.getState().doc);
  void loadCsv(sources[0]?.path ?? null);
  sources.slice(1).forEach((s) => void loadExtraCsv(s.name.toLowerCase(), s.path));
}

/**
 * Mantiene las fuentes cargadas al día con `variable_text.sources` del documento de la pestaña ACTIVA:
 * lo que ya está cargado (que viaja con la pestaña) se compara con lo que el documento pide, y solo se
 * lee lo que falta. Devuelve cómo parar.
 */
export function startCsvSync(): () => void {
  const check = () => {
    const { sources } = variableTextOf(useStore.getState().doc);
    const ui = useVariablesUi.getState();
    const main = sources[0]?.path ?? null;
    if (main !== ui.csv.path) void loadCsv(main);
    const wanted = new Map(sources.slice(1).map((s) => [s.name.toLowerCase(), s.path]));
    for (const name of Object.keys(ui.extra)) if (!wanted.has(name)) void loadExtraCsv(name, null);
    for (const [name, path] of wanted) if (ui.extra[name]?.path !== path) void loadExtraCsv(name, path);
  };
  check();
  const unsubscribe = useStore.subscribe(check);
  return unsubscribe;
}
