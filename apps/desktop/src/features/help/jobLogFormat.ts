import type { JobLogEntry } from './jobLog';

/** Fecha y hora local corta, en el idioma de la interfaz. */
export function fmtWhen(ms: number): string {
  return new Date(ms).toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' });
}

/** Cuánto más (o menos) tardó el trabajo que lo estimado, para un vistazo rápido: «+12 %». */
export function deltaLabel(e: Pick<JobLogEntry, 'estimatedS' | 'actualS'>): string | null {
  if (e.estimatedS === null || e.estimatedS <= 0) return null;
  const pct = Math.round(((e.actualS - e.estimatedS) / e.estimatedS) * 100);
  return `${pct > 0 ? '+' : ''}${pct} %`;
}

/** El SVG de la miniatura como URL de una imagen: dentro de un `<img>` no corre ningún script. */
export function thumbUrl(svg: string): string {
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}
