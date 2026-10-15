import { create } from 'zustand';
import { getLanguage } from '@/i18n';
import type { MachineCost, MaterialCost, MaterialMode, SalesCost, TimeSource } from './costModel';
import {
  cleanProfileName,
  defaultStored,
  machineFor,
  readCosts,
  removeMaterial,
  sanitizeMachine,
  sanitizeMaterial,
  sanitizeSales,
  upsertMaterial,
  writeCosts,
  type CostStored,
} from './costProfiles';

const systemTag = () => (typeof navigator === 'undefined' ? null : navigator.language);

interface CostUi extends CostStored {
  open: boolean;
  /** Los datos del disco ya se leyeron (se hace al abrir, cuando el idioma ya está fijado). */
  loaded: boolean;
  openDialog: () => void;
  closeDialog: () => void;
  /** Cambia los valores de la máquina `deviceId` y los recuerda. */
  setMachine: (deviceId: string, patch: Partial<MachineCost>) => void;
  setSales: (patch: Partial<SalesCost>) => void;
  setMaterial: (patch: Partial<MaterialCost>) => void;
  setMaterialMode: (mode: MaterialMode) => void;
  setTimeSource: (source: TimeSource) => void;
  setQuantity: (n: number) => void;
  setPiecesInDesign: (n: number) => void;
  /** Guarda los valores de material de ahora como perfil `name` (reemplaza uno con ese nombre). */
  saveMaterialProfile: (name: string) => boolean;
  loadMaterialProfile: (name: string) => void;
  deleteMaterialProfile: (name: string) => void;
}

function snapshot(s: CostUi): CostStored {
  return {
    machines: s.machines,
    sales: s.sales,
    material: s.material,
    materialName: s.materialName,
    materials: s.materials,
    materialMode: s.materialMode,
    timeSource: s.timeSource,
    quantity: s.quantity,
    piecesInDesign: s.piecesInDesign,
  };
}

/**
 * Estado de la calculadora «Costos y precio». Cada cambio se recuerda en `localStorage`
 * (`costProfiles.ts`); no es parte del documento ni sale de la computadora.
 */
export const useCostStore = create<CostUi>()((set, get) => {
  /** Aplica un cambio, lo sanea y lo guarda. */
  const commit = (patch: Partial<CostStored>) => {
    set(patch);
    writeCosts(snapshot(get()));
  };
  return {
    ...defaultStored(getLanguage(), systemTag()),
    open: false,
    loaded: false,
    openDialog: () => {
      if (!get().loaded) set({ ...readCosts(getLanguage(), systemTag()), loaded: true });
      set({ open: true });
    },
    closeDialog: () => set({ open: false }),
    setMachine: (deviceId, patch) => {
      const s = get();
      commit({ machines: { ...s.machines, [deviceId]: sanitizeMachine({ ...machineFor(s, deviceId), ...patch }) } });
    },
    setSales: (patch) => {
      const s = get();
      commit({ sales: sanitizeSales({ ...s.sales, ...patch }, getLanguage(), systemTag()) });
    },
    setMaterial: (patch) => {
      const s = get();
      commit({ material: sanitizeMaterial({ ...s.material, ...patch }) });
    },
    setMaterialMode: (materialMode) => commit({ materialMode }),
    setTimeSource: (timeSource) => commit({ timeSource }),
    setQuantity: (n) => commit({ quantity: Math.max(1, Math.floor(Number.isFinite(n) ? n : 1)) }),
    setPiecesInDesign: (n) => commit({ piecesInDesign: Math.max(1, Math.floor(Number.isFinite(n) ? n : 1)) }),
    saveMaterialProfile: (name) => {
      const clean = cleanProfileName(name);
      if (!clean) return false;
      const s = get();
      const materials = upsertMaterial(s.materials, { name: clean, material: s.material, mode: s.materialMode });
      if (!materials.some((p) => p.name === clean)) return false;
      commit({ materials, materialName: clean });
      return true;
    },
    loadMaterialProfile: (name) => {
      const p = get().materials.find((x) => x.name === name);
      if (p) commit({ material: { ...p.material }, materialMode: p.mode, materialName: p.name });
    },
    deleteMaterialProfile: (name) => {
      const s = get();
      commit({ materials: removeMaterial(s.materials, name), materialName: s.materialName === name ? null : s.materialName });
    },
  };
});

/** Para las pruebas: vuelve al estado de fábrica y a «no leído». */
export function resetCostStore(): void {
  useCostStore.setState({ ...defaultStored(getLanguage(), systemTag()), open: false, loaded: false });
}
