import type { DeviceProfile } from '@/api/types';

export type FeederSettings = DeviceProfile['feeder'];

/** Número para el G-code: sin ceros sobrantes y con hasta 3 decimales (`299.5`, `300`). */
export function formatMm(mm: number): string {
  return String(Number(mm.toFixed(3)));
}

/**
 * Líneas de G-code del avance: `feed_gcode` con `{mm}` sustituido, sin líneas vacías ni
 * comentarios completos (`;` o `(...)`). Los comentarios al final de línea se quitan.
 */
export function feedLines(feeder: FeederSettings, mm: number): string[] {
  const value = formatMm(mm);
  return feeder.feed_gcode
    .replaceAll('{mm}', value)
    .split(/\r?\n/)
    .map((l) => l.replace(/;.*$/, '').replace(/\(.*?\)/g, '').trim())
    .filter((l) => l !== '');
}

export type FeederIssue = 'gcodeEmpty' | 'gcodeNoMm' | 'gcodePlaceholder' | 'gcodeLong' | 'mmPositive' | 'mmTooBig';

/** Los avances más largos que esto casi seguro son un error de unidades. */
export const MAX_FEED_MM = 5000;
/** Cota de una línea de G-code (GRBL admite unas 80 con su buffer de línea). */
export const MAX_FEED_LINE = 120;

/**
 * Errores del alimentador por campo; vacío si es válido. Un alimentador apagado no se valida (así
 * un perfil con el G-code a medias se puede guardar mientras no se use).
 */
export function validateFeeder(feeder: FeederSettings): Partial<Record<'feed_gcode' | 'feed_mm', FeederIssue>> {
  const out: Partial<Record<'feed_gcode' | 'feed_mm', FeederIssue>> = {};
  if (!feeder.enabled) return out;
  const code = feeder.feed_gcode;
  const unknown = [...code.matchAll(/\{(\w*)\}/g)].some((m) => m[1] !== 'mm');
  if (code.trim() === '') out.feed_gcode = 'gcodeEmpty';
  else if (unknown) out.feed_gcode = 'gcodePlaceholder';
  else if (!code.includes('{mm}')) out.feed_gcode = 'gcodeNoMm';
  else if (feedLines(feeder, 9999.999).some((l) => l.length > MAX_FEED_LINE)) out.feed_gcode = 'gcodeLong';
  if (!Number.isFinite(feeder.feed_mm) || feeder.feed_mm <= 0) out.feed_mm = 'mmPositive';
  else if (feeder.feed_mm > MAX_FEED_MM) out.feed_mm = 'mmTooBig';
  return out;
}
