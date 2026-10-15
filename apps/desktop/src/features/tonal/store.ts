import { create } from 'zustand';
import { readPref, writePref } from '@/lib/platform';
import { DEFAULT_SPOT_MM, DEFAULT_TONE_MATERIAL, SPOT_RANGE, TONE_MATERIALS } from './curve';

export type TonalMode = 'normal' | 'tonal';
export type TonalStatus = 'idle' | 'busy' | 'ready' | 'error';

const PREF_MATERIAL = 'preview.tonal.material';
const PREF_SPOT = 'preview.tonal.spot';

function readMaterial(): string {
  const v = readPref(PREF_MATERIAL);
  return v && TONE_MATERIALS.includes(v) ? v : DEFAULT_TONE_MATERIAL;
}

function readSpot(): number {
  const raw = readPref(PREF_SPOT);
  const v = raw === null ? NaN : Number(raw);
  return Number.isFinite(v) && v >= SPOT_RANGE.min && v <= SPOT_RANGE.max ? v : DEFAULT_SPOT_MM;
}

/** Estado de interfaz de la vista previa «por tonos» (no es parte del documento). */
interface TonalState {
  /** Cómo se ve la vista previa; siempre arranca en normal. */
  mode: TonalMode;
  /** Material de la simulación (de `TONE_MATERIALS`). */
  material: string;
  /** Ancho del punto del láser, mm (los perfiles no lo guardan). */
  spotMm: number;
  status: TonalStatus;
  error: string | null;
  /** Diálogo de calibración con la prueba de materiales. */
  calibrateOpen: boolean;

  setMode: (mode: TonalMode) => void;
  setMaterial: (material: string) => void;
  setSpot: (spotMm: number) => void;
  setStatus: (status: TonalStatus, error?: string | null) => void;
  setCalibrateOpen: (open: boolean) => void;
}

export const useTonal = create<TonalState>()((set) => ({
  mode: 'normal',
  material: readMaterial(),
  spotMm: readSpot(),
  status: 'idle',
  error: null,
  calibrateOpen: false,

  setMode: (mode) => set({ mode }),
  setMaterial: (material) => {
    if (!TONE_MATERIALS.includes(material)) return;
    writePref(PREF_MATERIAL, material);
    set({ material });
  },
  setSpot: (spotMm) => {
    if (!Number.isFinite(spotMm)) return;
    const v = Math.min(SPOT_RANGE.max, Math.max(SPOT_RANGE.min, spotMm));
    writePref(PREF_SPOT, String(v));
    set({ spotMm: v });
  },
  setStatus: (status, error = null) => set({ status, error }),
  setCalibrateOpen: (calibrateOpen) => set({ calibrateOpen }),
}));
