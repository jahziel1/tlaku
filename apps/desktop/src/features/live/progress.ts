import { selectActiveDevice, selectJobPreview } from '@/store/selectors';
import { jobElapsedSeconds } from '@/store/machine/reducer';
import type { AppState } from '@/store/types';

/**
 * Fracción 0..1 del trabajo según el progreso que ya existe: líneas con `ok` del streamer en las
 * máquinas que transmiten línea a línea y tiempo transcurrido sobre el estimado en las que no
 * (Ruida, K40 y galvo suben el archivo: su «progreso» es la subida, no el trabajo).
 */
export function jobFraction(s: AppState, now: number): number {
  if (s.jobState === 'finished') return 1;
  if (s.jobState !== 'running' && s.jobState !== 'paused') return 0;
  const total = selectJobPreview(s)?.total_s ?? 0;
  const byTime = total > 0 ? Math.min(0.99, jobElapsedSeconds(s, now) / total) : null;
  const byLines = s.progress && s.progress.total > 0 ? Math.min(1, s.progress.acked / s.progress.total) : null;
  const kind = selectActiveDevice(s).kind;
  const streams = kind !== 'ruida' && kind !== 'lhy_micro' && kind !== 'galvo';
  return (streams ? (byLines ?? byTime) : (byTime ?? byLines)) ?? 0;
}
