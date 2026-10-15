import { selectActiveDevice, useStore } from '@/store';
import type { AppState } from '@/store/types';
import { getLiveService, onLiveReading, useLive } from './service';
import { EMPTY_SNAPSHOT, sameSnapshot, type LiveSnapshot, type LiveSource } from './source';

type StoreLike = Pick<typeof useStore, 'getState' | 'subscribe'>;

/** La foto de baja frecuencia sacada de los stores y del servicio de la ventana. */
export function readSnapshot(s: AppState): LiveSnapshot {
  const info = useLive.getState();
  const svc = getLiveService();
  const device = svc?.device ?? selectActiveDevice(s);
  return {
    jobSeq: info.jobSeq,
    route: info.route,
    source: info.source,
    fraction: info.fraction,
    emitting: info.emitting,
    ended: info.ended,
    job: s.jobState,
    timing: { jobStartedAt: s.jobStartedAt, jobPausedAt: s.jobPausedAt, jobPausedMs: s.jobPausedMs, jobEndedAt: s.jobEndedAt },
    total: svc?.route?.total ?? null,
    bed: [device.bed[0], device.bed[1]],
    showTravel: s.previewView.showTravel,
    theme: s.prefs.theme,
    lang: s.prefs.language,
  };
}

/**
 * Fuente de la ventana principal: lee el servicio (`getLiveService`) y el store. Es de solo
 * lectura. `subscribe` avisa de todo lo que pide otro cuadro: otra foto, un reporte nuevo de la máquina.
 */
export function createLocalSource(store: StoreLike = useStore): LiveSource {
  let last: LiveSnapshot = EMPTY_SNAPSHOT;
  return {
    snapshot() {
      const next = readSnapshot(store.getState());
      if (!sameSnapshot(last, next)) last = next;
      return last;
    },
    route() {
      return useLive.getState().route === 'ready' ? (getLiveService()?.route ?? null) : null;
    },
    frame() {
      return getLiveService()?.frame() ?? null;
    },
    subscribe(fn) {
      const offStore = store.subscribe((s, prev) => {
        if (s.status !== prev.status || s.jobState !== prev.jobState || s.previewView.showTravel !== prev.previewView.showTravel || s.prefs.theme !== prev.prefs.theme || s.prefs.language !== prev.prefs.language || s.devices !== prev.devices || s.activeDeviceId !== prev.activeDeviceId || s.jobStartedAt !== prev.jobStartedAt || s.jobPausedAt !== prev.jobPausedAt || s.jobEndedAt !== prev.jobEndedAt) fn();
      });
      const offLive = useLive.subscribe(fn);
      const offReading = onLiveReading(fn);
      return () => {
        offStore();
        offLive();
        offReading();
      };
    },
  };
}

let shared: LiveSource | null = null;
/** La fuente única de la ventana principal. */
export function localSource(): LiveSource {
  return (shared ??= createLocalSource());
}
