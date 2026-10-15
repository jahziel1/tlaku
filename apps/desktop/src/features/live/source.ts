import type { Language } from '@/i18n';
import type { LiveFrame } from './follow';
import type { RouteShape } from './route';
import type { LiveInfo } from './service';

/** Fase del trabajo, tal como la ve la vista en vivo. */
export type JobPhase = 'idle' | 'running' | 'paused' | 'finished' | 'cancelled' | 'error';

/** Tiempos del trabajo (ms del reloj de la PC) para el transcurrido y el restante. */
export interface LiveTiming {
  jobStartedAt: number | null;
  jobPausedAt: number | null;
  jobPausedMs: number;
  jobEndedAt: number | null;
}

/**
 * Todo lo de BAJA frecuencia que necesita la vista en vivo para pintarse (porcentaje, estado, tiempos,
 * tamaño de la cama, tema e idioma). Son datos simples: la ventana principal los saca de sus
 * stores y la ventana aparte los recibe tal cual (`wire.ts`).
 */
export interface LiveSnapshot extends Pick<LiveInfo, 'jobSeq' | 'route' | 'source' | 'fraction' | 'emitting' | 'ended'> {
  job: JobPhase;
  timing: LiveTiming;
  /** Duración estimada de la ruta (s), si ya se conoce. */
  total: number | null;
  /** Tamaño de la cama (mm). */
  bed: [number, number];
  showTravel: boolean;
  theme: 'dark' | 'light';
  lang: Language;
}

export const EMPTY_SNAPSHOT: LiveSnapshot = {
  jobSeq: 0,
  route: 'none',
  source: null,
  fraction: 0,
  emitting: false,
  ended: null,
  job: 'idle',
  timing: { jobStartedAt: null, jobPausedAt: null, jobPausedMs: 0, jobEndedAt: null },
  total: null,
  bed: [300, 200],
  showTravel: false,
  theme: 'dark',
  lang: 'es',
};

/** ¿Dos fotos dicen lo mismo? (para no avisar de cambios que no lo son). */
export function sameSnapshot(a: LiveSnapshot, b: LiveSnapshot): boolean {
  return (
    a.jobSeq === b.jobSeq &&
    a.route === b.route &&
    a.source === b.source &&
    a.fraction === b.fraction &&
    a.emitting === b.emitting &&
    a.ended === b.ended &&
    a.job === b.job &&
    a.total === b.total &&
    a.bed[0] === b.bed[0] &&
    a.bed[1] === b.bed[1] &&
    a.showTravel === b.showTravel &&
    a.theme === b.theme &&
    a.lang === b.lang &&
    a.timing.jobStartedAt === b.timing.jobStartedAt &&
    a.timing.jobPausedAt === b.timing.jobPausedAt &&
    a.timing.jobPausedMs === b.timing.jobPausedMs &&
    a.timing.jobEndedAt === b.timing.jobEndedAt
  );
}

/**
 * De dónde saca la vista en vivo lo que dibuja. La ventana principal lo lee del servicio y de los
 * stores (`localSource.ts`); la ventana aparte, de lo que le manda la principal (`remoteSource.ts`).
 * El dibujo (`LivePanel`) es el mismo.
 */
export interface LiveSource {
  /** La misma foto mientras nada cambie (sirve a `useSyncExternalStore`). */
  snapshot(): LiveSnapshot;
  /** La ruta del trabajo, solo con `snapshot().route === 'ready'`. */
  route(): RouteShape | null;
  /** El cuadro a dibujar ahora; `null` si todavía no hay con qué. */
  frame(): LiveFrame | null;
  /** Avisa de cualquier cosa que pida otro cuadro (otra foto, un reporte nuevo). Devuelve cómo dejar de escuchar. */
  subscribe(fn: () => void): () => void;
}
