import type { Document, JobPreview } from '@/api/types';

export interface JobLayerLine {
  color: string;
  name: string;
  mode: string;
  /** Potencia de la capa, 0-100 %. */
  power: number;
  speed: number;
  passes: number;
  seconds: number;
}

export interface JobSummary {
  totalSeconds: number;
  layers: JobLayerLine[];
  /** Mayor potencia de las capas que se envían, 0-100 %. */
  maxPower: number;
  warnings: string[];
}

/** Resumen para el diálogo de inicio: tiempo, capas que salen y potencia máxima. */
export function summarizeJob(doc: Pick<Document, 'layers'>, preview: JobPreview): JobSummary {
  const byColor = new Map(doc.layers.map((l) => [l.color.toLowerCase(), l]));
  const merged = new Map<string, JobLayerLine>();
  for (const pl of preview.layers) {
    const key = pl.color.toLowerCase();
    const prev = merged.get(key);
    if (prev) {
      prev.seconds += pl.seconds; // una capa de imagen con varias imágenes da una entrada por imagen
      continue;
    }
    const layer = byColor.get(key);
    merged.set(key, {
      color: pl.color,
      name: layer?.name ?? pl.color,
      mode: layer?.mode ?? pl.mode,
      power: layer?.power ?? 0,
      speed: layer?.speed ?? 0,
      passes: layer?.passes ?? 1,
      seconds: pl.seconds,
    });
  }
  const layers = [...merged.values()];
  return {
    totalSeconds: preview.total_s,
    layers,
    maxPower: layers.reduce((m, l) => Math.max(m, l.power), 0),
    warnings: preview.warnings,
  };
}

/**
 * Segundos que faltan. Al principio manda la estimación del motor; cuando ya hay avance
 * suficiente se usa el ritmo real (tiempo transcurrido sin contar pausas).
 */
export function remainingSeconds(pct: number, estimateS: number | null, elapsedS: number): number | null {
  if (pct >= 1) return 0;
  const fromRate = pct >= 0.03 && elapsedS >= 5 ? (elapsedS * (1 - pct)) / pct : null;
  if (fromRate !== null && estimateS !== null) return 0.7 * fromRate + 0.3 * estimateS * (1 - pct);
  if (fromRate !== null) return fromRate;
  return estimateS === null ? null : estimateS * (1 - pct);
}
