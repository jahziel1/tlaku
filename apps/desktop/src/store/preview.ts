import { getApi } from '@/api';
import type { JobOptions, JobPreview } from '@/api/types';
import { DEFAULT_OPTIMIZATION, readOptimization, sanitizeOptimization, sameOptimization, writeOptimization, type Optimization } from '@/features/preview/optimization';
import { getModel, PLAY_SPEEDS, type PlaySpeed, type PreviewViewOptions } from '@/features/preview/model';
import { errorMessage } from '@/lib/errors';
import { readPref, writePref } from '@/lib/platform';
import { hasTemplates, resolveJobArgs } from '@/features/variables/jobResolve';
import { buildJobArgs } from './selectors';
import type { SliceCreator } from './types';

export type PreviewStatus = 'idle' | 'computing' | 'ready' | 'error';

export interface PreviewSlice {
  preview: JobPreview | null;
  previewStatus: PreviewStatus;
  previewError: string | null;
  /** El documento, las capas o el dispositivo cambiaron desde el último cálculo que se aplicó. */
  previewStale: boolean;
  /** La vista previa se dibuja encima del diseño. */
  previewVisible: boolean;
  /** 0..1: instante de la reproducción como fracción del tiempo total. */
  scrub: number;
  playing: boolean;
  playSpeed: PlaySpeed;
  /** Opciones visuales del recorrido (el lienzo observa `previewView` y se redibuja al cambiarlas). */
  previewView: PreviewViewOptions;
  jobOptions: JobOptions;
  /**
   * "Solo la selección": el trabajo (vista previa, encuadre, envío y exportación) lleva solo lo
   * seleccionado. Sin selección no tiene efecto y la interfaz lo deshabilita con el motivo; los
   * ids viajan en `JobOptions.selection` (ver `selectJobOptions`), no se guardan aquí.
   */
  cutSelectionOnly: boolean;

  computePreview: () => Promise<void>;
  /** Descarta el cálculo en vuelo (su respuesta ya no vale) y marca la vista previa como vencida. */
  invalidatePreview: () => void;
  setPreviewVisible: (v: boolean) => void;
  togglePreview: () => void;
  setScrub: (v: number) => void;
  setPlaying: (v: boolean) => void;
  togglePlaying: () => void;
  setPlaySpeed: (v: PlaySpeed) => void;
  stepPlaySpeed: (dir: 1 | -1) => void;
  setPreviewView: (patch: Partial<PreviewViewOptions>) => void;
  setJobOptions: (patch: Partial<JobOptions>) => void;
  setCutSelectionOnly: (v: boolean) => void;
  /** "Usar el origen de la selección": el origen del trabajo es la caja de la selección. */
  setUseSelectionOrigin: (v: boolean) => void;
  /**
   * Cambia opciones de optimización (orden de corte y recorridos) y las recuerda para la próxima
   * sesión. Se valida: la tolerancia se acota y lo que no sea del tipo correcto se ignora.
   */
  setOptimization: (patch: Partial<Optimization>) => void;
  /** Devuelve la optimización a sus valores de fábrica. */
  resetOptimization: () => void;
}

export const DEFAULT_JOB_OPTIONS: JobOptions = {
  start_from: 'absolute',
  job_origin: 6, // abajo a la izquierda (fila por fila desde arriba a la izquierda)
  user_origin: null,
  selection: null,
  layer_order: 'auto',
  optimization: DEFAULT_OPTIMIZATION,
  use_selection_origin: false,
  transform: null,
  tile: null,
};

const PREF_TRAVEL = 'preview.showTravel';
const PREF_OVERSCAN = 'preview.showOverscan';

export const createPreviewSlice: SliceCreator<PreviewSlice> = (set, get) => {
  /**
   * Número del último cálculo pedido o invalidado. Una respuesta solo se aplica si su número
   * sigue siendo el vigente: así una respuesta lenta no pisa a una más nueva, ni a un cambio
   * posterior del documento. El contrato no permite abortar la petición, solo ignorar su resultado.
   */
  let seq = 0;

  return {
    preview: null,
    previewStatus: 'idle',
    previewError: null,
    previewStale: true,
    previewVisible: false,
    scrub: 1,
    playing: false,
    playSpeed: 1,
    previewView: {
      showTravel: readPref(PREF_TRAVEL) !== '0',
      showOverscan: readPref(PREF_OVERSCAN) !== '0',
      highlightLayer: null,
    },
    // La optimización se recuerda entre sesiones (ver `features/preview/optimization.ts`).
    jobOptions: { ...DEFAULT_JOB_OPTIONS, optimization: readOptimization() },
    cutSelectionOnly: false,

    computePreview: async () => {
      const mine = ++seq;
      set({ previewStatus: 'computing', previewError: null, previewStale: false });
      try {
        // Los textos con variables se resuelven con los valores de ahora antes de calcular.
        const base = buildJobArgs(get());
        const args = hasTemplates(base.doc) ? await resolveJobArgs(base) : base;
        const preview = await getApi().computeJob(args);
        if (mine !== seq) return; // llegó una petición más nueva o el documento cambió
        getModel(preview, args.device.rapid_rate); // arma el modelo ya, fuera del camino de dibujo
        set((s) => ({
          preview,
          previewStatus: 'ready',
          previewView: s.previewView.highlightLayer === null ? s.previewView : { ...s.previewView, highlightLayer: null },
        }));
      } catch (e) {
        if (mine !== seq) return;
        set({ previewStatus: 'error', previewError: errorMessage(e), preview: null, playing: false });
      }
    },

    invalidatePreview: () => {
      seq++;
      if (!get().previewStale) set({ previewStale: true });
    },

    setPreviewVisible: (previewVisible) => set(previewVisible ? { previewVisible } : { previewVisible, playing: false }),
    togglePreview: () => get().setPreviewVisible(!get().previewVisible),
    setScrub: (scrub) => set({ scrub: Math.min(1, Math.max(0, Number.isFinite(scrub) ? scrub : 0)) }),

    setPlaying: (playing) => {
      const s = get();
      if (!playing) return void set({ playing: false });
      if (!s.preview || s.preview.moves.length === 0) return;
      // Reproducir al final reinicia desde el principio.
      set({ playing: true, scrub: s.scrub >= 1 ? 0 : s.scrub });
    },
    togglePlaying: () => get().setPlaying(!get().playing),
    setPlaySpeed: (playSpeed) => set({ playSpeed }),
    stepPlaySpeed: (dir) => {
      const i = PLAY_SPEEDS.indexOf(get().playSpeed);
      const next = PLAY_SPEEDS[Math.min(PLAY_SPEEDS.length - 1, Math.max(0, i + dir))]!;
      set({ playSpeed: next });
    },

    setPreviewView: (patch) => {
      const s = get();
      const previewView = { ...s.previewView, ...patch };
      if (patch.showTravel !== undefined) writePref(PREF_TRAVEL, patch.showTravel ? '1' : '0');
      if (patch.showOverscan !== undefined) writePref(PREF_OVERSCAN, patch.showOverscan ? '1' : '0');
      set({ previewView });
    },

    setJobOptions: (patch) => set((s) => ({ jobOptions: { ...s.jobOptions, ...patch } })),
    setCutSelectionOnly: (cutSelectionOnly) => set({ cutSelectionOnly }),
    setUseSelectionOrigin: (use_selection_origin) => set((s) => ({ jobOptions: { ...s.jobOptions, use_selection_origin } })),
    setOptimization: (patch) => {
      const current = get().jobOptions.optimization;
      const optimization = sanitizeOptimization({ ...current, ...patch });
      if (sameOptimization(current, optimization)) return;
      writeOptimization(optimization);
      set((s) => ({ jobOptions: { ...s.jobOptions, optimization } }));
    },
    resetOptimization: () => get().setOptimization(DEFAULT_OPTIMIZATION),
  };
};
