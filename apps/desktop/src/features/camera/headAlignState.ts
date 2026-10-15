/** Estado de interfaz de «alinear con el cabezal» (cámara fija, sin grabar). */
import { create } from 'zustand';
import type { HeadAlignFrame, HeadAlignReport, HeadMethod } from '@/api/types';
import type { Vec2 } from './geometry';

/** Ajustes del recorrido. Se recuerdan entre usos (preferencia de este equipo). */
export interface HeadAlignSettings {
  /** Puntos por lado de la rejilla (3 a 5). */
  grid: number;
  /** Margen libre a cada lado de la cama (mm). */
  marginMm: number;
  method: HeadMethod;
  /** Alturas sobre el material para corregir el paralaje; `null` = sin corrección. */
  cameraHeightMm: number | null;
  featureHeightMm: number | null;
  /** Desfase fijo (mm) entre el punto detectado y el punto del láser. */
  offset: Vec2;
}

export const DEFAULT_HEAD_ALIGN: HeadAlignSettings = {
  grid: 3,
  marginMm: 20,
  method: 'auto',
  cameraHeightMm: null,
  featureHeightMm: null,
  offset: [0, 0],
};

export const HEAD_ALIGN_MIN_GRID = 3;
export const HEAD_ALIGN_MAX_GRID = 5;
const STORAGE_KEY = 'tlaku.headAlign.settings';

/** Ajustes guardados; cualquier dato dañado vuelve al valor por defecto. Nunca lanza. */
export function loadHeadAlignSettings(): HeadAlignSettings {
  try {
    const raw = typeof localStorage === 'undefined' ? null : localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_HEAD_ALIGN;
    return sanitizeSettings(JSON.parse(raw) as Partial<HeadAlignSettings>);
  } catch {
    return DEFAULT_HEAD_ALIGN;
  }
}

function saveHeadAlignSettings(s: HeadAlignSettings): void {
  try {
    if (typeof localStorage !== 'undefined') localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
  } catch {
    // Sin almacenamiento (ventana privada): los ajustes valen para esta sesión.
  }
}

const num = (v: unknown): number | null => (typeof v === 'number' && Number.isFinite(v) ? v : null);

/** Fuerza cada ajuste a un valor que el motor acepta. */
export function sanitizeSettings(p: Partial<HeadAlignSettings>): HeadAlignSettings {
  const d = DEFAULT_HEAD_ALIGN;
  const grid = Math.round(num(p.grid) ?? d.grid);
  const cam = num(p.cameraHeightMm);
  const feat = num(p.featureHeightMm);
  const off = Array.isArray(p.offset) ? p.offset : d.offset;
  const o = (v: unknown) => Math.max(-100, Math.min(100, num(v) ?? 0));
  return {
    grid: Math.min(HEAD_ALIGN_MAX_GRID, Math.max(HEAD_ALIGN_MIN_GRID, grid)),
    marginMm: Math.max(0, Math.min(150, num(p.marginMm) ?? d.marginMm)),
    method: p.method === 'led' || p.method === 'diff' ? p.method : 'auto',
    cameraHeightMm: cam !== null && cam > 0 ? Math.min(2000, cam) : null,
    featureHeightMm: feat !== null && feat >= 0 ? Math.min(500, feat) : null,
    offset: [o(off[0]), o(off[1])],
  };
}

export type HeadAlignPhase = 'config' | 'running' | 'result';

/** Estado de una posición del recorrido. */
export type ShotState = 'pending' | 'moving' | 'shot';

export interface ShotInfo {
  /** Identificador estable (ronda y posición). */
  key: string;
  mm: Vec2;
  round: number;
  state: ShotState;
  /** Miniatura de la foto (data URL) o `null` si no se pudo hacer. */
  thumb: string | null;
}

interface HeadAlignUi {
  open: boolean;
  phase: HeadAlignPhase;
  settings: HeadAlignSettings;
  shots: ShotInfo[];
  round: number;
  report: HeadAlignReport | null;
  /** Área segura que recorrió la rejilla `[x0, y0, x1, y1]` (mm). */
  area: [number, number, number, number] | null;
  /** Las fotos de la última corrida (para volver a resolver con otro desfase). */
  frames: HeadAlignFrame[];
  error: string | null;
  cancelRequested: boolean;
  confirmed: boolean;
  /** Hubo una alineación guardada desde este resultado. */
  saved: boolean;
  openDialog: () => void;
  closeDialog: () => void;
  setSettings: (patch: Partial<HeadAlignSettings>) => void;
  setPhase: (p: HeadAlignPhase) => void;
  setConfirmed: (v: boolean) => void;
  setShots: (s: ShotInfo[]) => void;
  patchShot: (key: string, patch: Partial<ShotInfo>) => void;
  setRound: (n: number) => void;
  setResult: (report: HeadAlignReport, frames: HeadAlignFrame[], area: [number, number, number, number] | null) => void;
  setError: (e: string | null) => void;
  requestCancel: (v: boolean) => void;
  markSaved: () => void;
  backToConfig: () => void;
}

export const useHeadAlign = create<HeadAlignUi>()((set, get) => ({
  open: false,
  phase: 'config',
  settings: loadHeadAlignSettings(),
  shots: [],
  round: 0,
  report: null,
  area: null,
  frames: [],
  error: null,
  cancelRequested: false,
  confirmed: false,
  saved: false,
  // Un cierre a mitad del recorrido lo detiene: el diálogo no deja nada moviéndose por su cuenta.
  openDialog: () => set({ open: true, phase: 'config', shots: [], report: null, frames: [], error: null, confirmed: false, saved: false, cancelRequested: false }),
  closeDialog: () => set({ open: false, cancelRequested: get().phase === 'running' }),
  setSettings: (patch) => {
    const settings = sanitizeSettings({ ...get().settings, ...patch });
    saveHeadAlignSettings(settings);
    set({ settings });
  },
  setPhase: (phase) => set({ phase }),
  setConfirmed: (confirmed) => set({ confirmed }),
  setShots: (shots) => set({ shots }),
  patchShot: (key, patch) => set((s) => ({ shots: s.shots.map((x) => (x.key === key ? { ...x, ...patch } : x)) })),
  setRound: (round) => set({ round }),
  setResult: (report, frames, area) => set({ report, frames, area, phase: 'result', saved: false }),
  setError: (error) => set({ error }),
  requestCancel: (cancelRequested) => set({ cancelRequested }),
  markSaved: () => set({ saved: true }),
  backToConfig: () => set({ phase: 'config', shots: [], report: null, frames: [], error: null, confirmed: false, saved: false, cancelRequested: false }),
}));

/** Para los comandos del menú y el panel: abre el asistente sin importar componentes. */
export function openHeadAlignDialog(): void {
  useHeadAlign.getState().openDialog();
}

/** Punto de referencia opcional: un disparo de prueba visible, marcado en la imagen, de posición conocida. */
interface HeadAlignRefState {
  /** Píxel marcado en la imagen corregida (origen en el borde del píxel). */
  px: Vec2 | null;
  /** Posición (mm de documento) a la que se disparó. */
  bed: Vec2;
  /** Los clics en la imagen del panel marcan el punto. */
  marking: boolean;
  setPx: (p: Vec2 | null) => void;
  setBed: (b: Vec2) => void;
  setMarking: (v: boolean) => void;
}

export const useHeadAlignRef = create<HeadAlignRefState>()((set) => ({
  px: null,
  bed: [0, 0],
  marking: false,
  setPx: (px) => set({ px }),
  setBed: (bed) => set({ bed }),
  setMarking: (marking) => set({ marking }),
}));
