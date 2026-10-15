import { create } from 'zustand';
import type { GeneratorInfo } from '@/bindings/GeneratorInfo';
import type { GeneratorRecord } from '@/bindings/GeneratorRecord';

/** Valor de la lista de capas de rótulos que significa «no insertar rótulos». */
export const NO_ENGRAVE = 'none';
/** Valor que significa «crear una capa nueva de poca potencia». */
export const NEW_ENGRAVE = 'new';

export type FormValues = Record<string, unknown>;

interface PartsUi {
  open: boolean;
  /** Los generadores ya se pidieron al motor. */
  loaded: boolean;
  generators: GeneratorInfo[];
  generator: string;
  /** Valores del formulario por generador (se recuerdan mientras la ventana esté abierta). */
  values: Record<string, FormValues>;
  /** Capa de corte (`#rrggbb`) o `null` = la activa o la primera de líneas. */
  cutLayer: string | null;
  /** Capa de rótulos: un color, `NO_ENGRAVE` o `NEW_ENGRAVE`. */
  engraveLayer: string;
  /** Grupo que se reemplaza al «Regenerar»; `null` al insertar. */
  regenGroup: string | null;
  /** Pestaña en la que se abrió el diálogo. */
  tabId: string | null;
  busy: boolean;

  setGenerators: (list: GeneratorInfo[]) => void;
  /** Abre el diálogo para insertar. */
  openDialog: (tabId: string, generator?: string) => void;
  /** Abre el diálogo con los parámetros de un grupo ya generado. */
  openRegenerate: (tabId: string, record: GeneratorRecord) => void;
  closeDialog: () => void;
  setGenerator: (id: string) => void;
  setValue: (generator: string, key: string, value: unknown) => void;
  resetValues: (generator: string) => void;
  setCutLayer: (color: string | null) => void;
  setEngraveLayer: (value: string) => void;
  setBusy: (busy: boolean) => void;
}

/** Valores de fábrica de un generador (copia). */
export function defaultsOf(list: readonly GeneratorInfo[], id: string): FormValues {
  return { ...(list.find((g) => g.id === id)?.defaults ?? {}) };
}

export const usePartsUi = create<PartsUi>()((set) => ({
  open: false,
  loaded: false,
  generators: [],
  generator: 'box',
  values: {},
  cutLayer: null,
  engraveLayer: NEW_ENGRAVE,
  regenGroup: null,
  tabId: null,
  busy: false,

  setGenerators: (generators) => set({ generators, loaded: true }),
  openDialog: (tabId, generator) =>
    set((s) => ({ open: true, tabId, regenGroup: null, busy: false, generator: generator ?? s.generator })),
  openRegenerate: (tabId, record) =>
    set((s) => ({
      open: true,
      tabId,
      busy: false,
      regenGroup: record.group,
      generator: record.generator,
      // Los campos que el guardado no trae (versión anterior) toman el valor de fábrica.
      values: { ...s.values, [record.generator]: { ...defaultsOf(s.generators, record.generator), ...record.params } },
      cutLayer: record.cut_layer,
      engraveLayer: record.engrave_layer ?? NO_ENGRAVE,
    })),
  closeDialog: () => set({ open: false, regenGroup: null, busy: false }),
  setGenerator: (generator) => set({ generator }),
  setValue: (generator, key, value) =>
    set((s) => ({
      values: { ...s.values, [generator]: { ...(s.values[generator] ?? defaultsOf(s.generators, generator)), [key]: value } },
    })),
  resetValues: (generator) => set((s) => ({ values: { ...s.values, [generator]: defaultsOf(s.generators, generator) } })),
  setCutLayer: (cutLayer) => set({ cutLayer }),
  setEngraveLayer: (engraveLayer) => set({ engraveLayer }),
  setBusy: (busy) => set({ busy }),
}));

/** Valores que se muestran para un generador: los editados o los de fábrica. */
export function valuesFor(s: Pick<PartsUi, 'values' | 'generators'>, id: string): FormValues {
  return s.values[id] ?? defaultsOf(s.generators, id);
}

/** Vuelve el estado de la ventana al de fábrica (pruebas). */
export function resetPartsUi(): void {
  usePartsUi.setState({
    open: false,
    loaded: false,
    generators: [],
    generator: 'box',
    values: {},
    cutLayer: null,
    engraveLayer: NEW_ENGRAVE,
    regenGroup: null,
    tabId: null,
    busy: false,
  });
}
