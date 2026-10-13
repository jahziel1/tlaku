import { getApi } from '@/api';
import type { JobOptions, JobPreview } from '@/api/types';
import { errorMessage } from '@/lib/errors';
import { buildJobArgs } from './selectors';
import type { SliceCreator } from './types';

export type PreviewStatus = 'idle' | 'computing' | 'ready' | 'error';

export interface PreviewSlice {
  preview: JobPreview | null;
  previewStatus: PreviewStatus;
  previewError: string | null;
  /** La vista previa se dibuja encima del diseño. */
  previewVisible: boolean;
  /** 0..1: hasta dónde se reproduce el recorrido. */
  scrub: number;
  jobOptions: JobOptions;

  computePreview: () => Promise<void>;
  setPreviewVisible: (v: boolean) => void;
  togglePreview: () => void;
  setScrub: (v: number) => void;
  setJobOptions: (patch: Partial<JobOptions>) => void;
}

export const DEFAULT_JOB_OPTIONS: JobOptions = {
  start_from: 'absolute',
  job_origin: 6, // abajo a la izquierda (fila por fila desde arriba a la izquierda)
  user_origin: null,
  selection: null,
  layer_order: 'auto',
};

let seq = 0;

export const createPreviewSlice: SliceCreator<PreviewSlice> = (set, get) => ({
  preview: null,
  previewStatus: 'idle',
  previewError: null,
  previewVisible: false,
  scrub: 1,
  jobOptions: DEFAULT_JOB_OPTIONS,

  computePreview: async () => {
    const mine = ++seq;
    set({ previewStatus: 'computing', previewError: null });
    try {
      const preview = await getApi().computeJob(buildJobArgs(get()));
      if (mine !== seq) return; // llegó una petición más nueva
      set({ preview, previewStatus: 'ready' });
    } catch (e) {
      if (mine !== seq) return;
      set({ previewStatus: 'error', previewError: errorMessage(e), preview: null });
    }
  },

  setPreviewVisible: (previewVisible) => set({ previewVisible }),
  togglePreview: () => set((s) => ({ previewVisible: !s.previewVisible })),
  setScrub: (scrub) => set({ scrub: Math.min(1, Math.max(0, scrub)) }),
  setJobOptions: (patch) => set((s) => ({ jobOptions: { ...s.jobOptions, ...patch } })),
});
