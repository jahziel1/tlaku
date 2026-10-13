import { create } from 'zustand';
import { getApi, type LaserApi, type RecoveryInfo } from '@/api';
import { ensureSaved } from '@/features/history/files';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { emptyDocument } from '@/store/document';
import { useStore, type AppState } from '@/store';
import { activeAutosaver } from './autosave';

type RecoveryApi = Pick<LaserApi, 'autosaveList' | 'autosaveRead' | 'autosaveDiscard'>;

export interface RecoveryDeps {
  api: RecoveryApi;
  store: { getState: () => AppState; setState: (partial: Partial<AppState>) => void };
  /** Pregunta por los cambios sin guardar antes de reemplazar el documento. */
  ensureSaved: () => Promise<boolean>;
  /** Guarda de inmediato una copia del documento recién recuperado; `false` si no se pudo. */
  writeNow: () => Promise<boolean>;
}

interface RecoveryUi {
  open: boolean;
  items: RecoveryInfo[];
  /** Copia que se está recuperando (bloquea los botones). */
  busy: string | null;
  close: () => void;
}

/** Estado del diálogo de recuperación. */
export const useRecovery = create<RecoveryUi>()((set) => ({
  open: false,
  items: [],
  busy: null,
  close: () => set({ open: false }),
}));

function defaultDeps(): RecoveryDeps {
  return {
    api: getApi(),
    store: useStore,
    ensureSaved: () => ensureSaved('open'),
    writeNow: async () => (await activeAutosaver()?.tick(true)) === 'saved',
  };
}

/**
 * Busca copias pendientes (al abrir la app o una ventana) y abre el diálogo si hay. El motor solo
 * lista las huérfanas: las copias de otras ventanas que siguen abiertas no se ofrecen (ver
 * `LiveCopies` en `src-tauri/src/autosave.rs`).
 */
export async function checkRecovery(deps: Pick<RecoveryDeps, 'api' | 'store'> = defaultDeps()): Promise<RecoveryInfo[]> {
  try {
    const items = await deps.api.autosaveList();
    useRecovery.setState({ items, open: items.length > 0 });
    return items;
  } catch (e) {
    deps.store.getState().notify('warning', t('prefs.recovery.listFailed', { error: errorMessage(e) }));
    return [];
  }
}

function drop(id: string) {
  const items = useRecovery.getState().items.filter((i) => i.id !== id);
  useRecovery.setState({ items, open: items.length > 0 });
}

/**
 * Abre la copia `id` como el documento actual, marcado con cambios sin guardar. La copia solo se
 * borra cuando ya hay otra nueva del documento recuperado: así un cierre inesperado justo después
 * no pierde nada. Devuelve `true` si se recuperó.
 */
export async function recoverCopy(info: RecoveryInfo, deps: RecoveryDeps = defaultDeps()): Promise<boolean> {
  const state = () => deps.store.getState();
  if (useRecovery.getState().busy) return false;
  useRecovery.setState({ busy: info.id });
  try {
    if (!(await deps.ensureSaved())) return false;
    let doc;
    try {
      doc = await deps.api.autosaveRead({ id: info.id });
    } catch (e) {
      state().notify('error', t('prefs.recovery.recoverFailed', { name: info.name, error: errorMessage(e) }));
      return false;
    }
    state().replaceDocument(doc, info.path);
    // Lo recuperado nunca está guardado: se compara contra un documento vacío para que cuente como con cambios.
    deps.store.setState({ savedDoc: emptyDocument(), dirty: true });
    state().fitContent();
    void state().computePreview();
    state().notify('success', t('prefs.recovery.recovered', { name: info.name }));
    if (await deps.writeNow()) await discardCopy(info.id, deps);
    else drop(info.id);
    return true;
  } finally {
    useRecovery.setState({ busy: null });
  }
}

/** Borra una copia del disco y de la lista. */
export async function discardCopy(id: string, deps: Pick<RecoveryDeps, 'api' | 'store'> = defaultDeps()): Promise<void> {
  try {
    await deps.api.autosaveDiscard({ id });
  } catch (e) {
    deps.store.getState().notify('error', t('prefs.recovery.discardFailed', { error: errorMessage(e) }));
    return;
  }
  drop(id);
}

export async function discardAll(deps: Pick<RecoveryDeps, 'api' | 'store'> = defaultDeps()): Promise<void> {
  for (const item of [...useRecovery.getState().items]) await discardCopy(item.id, deps);
}
