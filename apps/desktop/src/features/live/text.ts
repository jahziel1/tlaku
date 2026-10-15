import { t, type I18nKey } from '@/i18n';
import type { LiveInfo } from './service';

type JobPhase = 'idle' | 'running' | 'paused' | 'finished' | 'cancelled' | 'error';

/** Texto del estado del trabajo para la alternativa textual del lienzo. */
export function stateText(info: Pick<LiveInfo, 'ended' | 'route'>, job: JobPhase, hasFrame: boolean): string {
  if (info.ended) return t(`live.state.${info.ended}` as I18nKey);
  if (job === 'paused') return t('live.state.paused');
  if (job === 'running') return hasFrame ? t('live.state.running') : t('live.state.waiting');
  return t('live.state.waiting');
}

/** Alternativa textual del lienzo: porcentaje, estado y de dónde sale el avance. */
export function liveSummary(info: Pick<LiveInfo, 'fraction' | 'source' | 'ended' | 'route'>, job: JobPhase, hasFrame: boolean): string {
  const percent = Math.round(info.fraction * 100);
  return t('live.canvas.aria', {
    percent,
    state: stateText(info, job, hasFrame),
    source: info.source ? t(`live.source.${info.source}` as I18nKey) : '',
  }).trim();
}

/** Mensaje cuando no hay nada que dibujar todavía. */
export function routeNote(route: LiveInfo['route']): string | null {
  if (route === 'computing') return t('live.route.computing');
  if (route === 'unavailable') return t('live.route.unavailable');
  if (route === 'none') return t('live.route.none');
  return null;
}
