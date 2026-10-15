import { create } from 'zustand';
import { getApi } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';
import {
  emptyRemnants,
  freshRemnantId,
  MAX_REMNANT_NAME,
  MAX_REMNANTS,
  remnantFromLeftover,
  type LeftoverForm,
  type Remnant,
  type RemnantLibrary,
} from './remnants';

interface RemnantState {
  library: RemnantLibrary;
  /** El inventario ya se leyó del disco (o se confirmó que no hay). */
  loaded: boolean;
  /** Mensaje si la lectura falló: mientras tanto no se guarda nada para no pisar el archivo. */
  loadError: string | null;
  /** Diálogo del inventario abierto. */
  panelOpen: boolean;
  setPanelOpen: (open: boolean) => void;
}

/** Estado del inventario de retazos (no es parte del documento). */
export const useRemnants = create<RemnantState>()((set) => ({
  library: emptyRemnants(),
  loaded: false,
  loadError: null,
  panelOpen: false,
  setPanelOpen: (panelOpen) => set({ panelOpen }),
}));

export const setRemnantsPanel = (open: boolean): void => useRemnants.getState().setPanelOpen(open);

const ui = () => useRemnants.getState();
const notify = (level: 'success' | 'warning' | 'error', text: string) => useStore.getState().notify(level, text);

let loading: Promise<void> | null = null;

/** Lee el inventario (una vez). Si falla se avisa y no se guarda nada hasta reintentar. */
export function loadRemnants(force = false): Promise<void> {
  if (ui().loaded && !force) return Promise.resolve();
  loading ??= getApi()
    .loadRemnants()
    .then((library) => {
      useRemnants.setState({ library, loaded: true, loadError: null });
    })
    .catch((e: unknown) => {
      const error = errorMessage(e);
      useRemnants.setState({ loaded: false, loadError: error });
      notify('error', t('nest.remnants.err.load', { error }));
    })
    .finally(() => {
      loading = null;
    });
  return loading;
}

let saveQueue: Promise<void> = Promise.resolve();

/** Guarda el inventario actual. Las escrituras se encadenan para que la última gane. */
function persist(): Promise<void> {
  saveQueue = saveQueue.then(async () => {
    if (ui().loadError) return; // no se pisa un archivo que no se pudo leer
    try {
      await getApi().saveRemnants({ library: ui().library });
    } catch (e) {
      notify('error', t('nest.remnants.err.save', { error: errorMessage(e) }));
    }
  });
  return saveQueue;
}

function mutate(change: (remnants: Remnant[]) => Remnant[]): Promise<void> {
  const library = ui().library;
  useRemnants.setState({ library: { ...library, remnants: change(library.remnants) } });
  return persist();
}

/** Guarda un sobrante del motor como retazo. Devuelve el retazo, o `null` si no cabe en el inventario o no es válido. */
export function addLeftover(rings: [number, number][][], form: Omit<LeftoverForm, 'id' | 'now'>): Remnant | null {
  const list = ui().library.remnants;
  if (list.length >= MAX_REMNANTS) {
    notify('warning', t('nest.remnants.full', { max: MAX_REMNANTS }));
    return null;
  }
  const now = Date.now();
  const r = remnantFromLeftover(rings, { ...form, id: freshRemnantId(new Set(list.map((x) => x.id)), now), now });
  if (!r) return null;
  void mutate((all) => [...all, r]);
  return r;
}

export function removeRemnants(ids: readonly string[]): Promise<void> {
  const gone = new Set(ids);
  return mutate((all) => all.filter((r) => !gone.has(r.id)));
}

export function renameRemnant(id: string, name: string): Promise<void> {
  const clean = name.trim().slice(0, MAX_REMNANT_NAME);
  if (!clean) return Promise.resolve();
  return mutate((all) => all.map((r) => (r.id === id ? { ...r, name: clean } : r)));
}

/** Cambia el material y el grosor de un retazo. */
export function retagRemnant(id: string, material: string, thicknessMm: number): Promise<void> {
  const thickness = Number.isFinite(thicknessMm) ? Math.max(0, thicknessMm) : 0;
  return mutate((all) => all.map((r) => (r.id === id ? { ...r, material: material.trim(), thickness_mm: thickness } : r)));
}
