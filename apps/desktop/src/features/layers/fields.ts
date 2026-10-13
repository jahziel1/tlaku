import type { I18nKey } from '@/i18n';

/**
 * Campos numéricos del editor de capas: unidad, rango y validación. Un valor inválido se
 * marca en la interfaz y NO se aplica al documento. Los rangos no son más estrictos que los
 * del motor (`tlaku-pipeline`): lo que él acepta, el editor lo acepta.
 */
export type NumericField =
  | 'speed'
  | 'power'
  | 'min_power'
  | 'passes'
  | 'interval'
  | 'dpi'
  | 'angle'
  | 'overscan'
  | 'kerf'
  | 'threshold'
  | 'brightness'
  | 'contrast'
  | 'gamma'
  | 'dot_width'
  | 'order'
  | 'tab_width'
  | 'tab_count'
  | 'tab_spacing'
  | 'halftone_cell'
  | 'halftone_angle'
  | 'z_offset'
  | 'z_step';

export interface FieldSpec {
  min: number;
  max: number;
  /** Paso de las flechas del teclado. */
  step: number;
  /** Decimales al mostrar el valor. */
  decimals: number;
  unit: string;
  integer?: boolean;
  /** El valor debe ser mayor que cero (el mínimo es solo un tope práctico): mensaje propio. */
  positive?: boolean;
}

export const FIELD_SPECS: Record<NumericField, FieldSpec> = {
  speed: { min: 1, max: 60000, step: 50, decimals: 0, unit: 'mm/min' },
  power: { min: 0, max: 100, step: 1, decimals: 1, unit: '%' },
  min_power: { min: 0, max: 100, step: 1, decimals: 1, unit: '%' },
  passes: { min: 1, max: 1000, step: 1, decimals: 0, unit: '', integer: true },
  interval: { min: 0.01, max: 5, step: 0.01, decimals: 4, unit: 'mm' },
  // 25,4 / intervalo: 0,01 mm = 2540 líneas/pulgada; 5 mm = 5,08.
  dpi: { min: 5.08, max: 2540, step: 10, decimals: 1, unit: 'DPI' },
  angle: { min: -360, max: 360, step: 1, decimals: 2, unit: '°' },
  overscan: { min: 0, max: 50, step: 0.5, decimals: 2, unit: 'mm' },
  kerf: { min: -5, max: 5, step: 0.01, decimals: 3, unit: 'mm' },
  threshold: { min: 0, max: 1, step: 0.01, decimals: 2, unit: '' },
  brightness: { min: -1, max: 1, step: 0.05, decimals: 2, unit: '' },
  contrast: { min: -1, max: 1, step: 0.05, decimals: 2, unit: '' },
  gamma: { min: 0.1, max: 5, step: 0.05, decimals: 2, unit: '' },
  dot_width: { min: 0, max: 1, step: 0.01, decimals: 3, unit: 'mm' },
  order: { min: 1, max: 999, step: 1, decimals: 0, unit: '', integer: true },
  tab_width: { min: 0.01, max: 50, step: 0.1, decimals: 3, unit: 'mm', positive: true },
  tab_count: { min: 1, max: 100, step: 1, decimals: 0, unit: '', integer: true },
  // 0 = repartir por cantidad.
  tab_spacing: { min: 0, max: 1000, step: 1, decimals: 2, unit: 'mm' },
  halftone_cell: { min: 0.05, max: 10, step: 0.05, decimals: 3, unit: 'mm', positive: true },
  halftone_angle: { min: -360, max: 360, step: 1, decimals: 2, unit: '°' },
  z_offset: { min: -100, max: 100, step: 0.1, decimals: 3, unit: 'mm' },
  z_step: { min: -100, max: 100, step: 0.1, decimals: 3, unit: 'mm' },
};

export interface FieldError {
  key: I18nKey;
  params?: Record<string, string | number>;
}

export type FieldResult = { ok: true; value: number } | { ok: false; error: FieldError };

/** Contexto para validaciones que dependen de otros campos de la capa. */
export interface FieldContext {
  /** Potencia máxima de la capa (para validar la mínima). */
  power?: number;
  /** Potencia mínima de la capa (para validar la máxima). */
  minPower?: number;
  /** Número de capas, tope de la posición fijada. */
  layerCount?: number;
  /** Largo de los puentes (la separación, si no es 0, debe ser mayor). */
  tabWidth?: number;
}

/** Texto numérico con coma o punto decimal; `null` si no es un número finito. */
export function parseDecimal(text: string): number | null {
  const s = text.trim().replace(',', '.');
  if (s === '') return null;
  const n = Number(s);
  return Number.isFinite(n) ? n : null;
}

/** Número para mostrar en un campo: sin ceros sobrantes. */
export function formatNumber(value: number, decimals: number): string {
  return String(Number(value.toFixed(decimals)));
}

/** Valida el texto de un campo. Devuelve el valor o el motivo del rechazo. */
export function validateField(field: NumericField, text: string, ctx: FieldContext = {}): FieldResult {
  const spec = FIELD_SPECS[field];
  const n = parseDecimal(text);
  if (n === null) return { ok: false, error: { key: 'layers.err.number' } };
  if (spec.integer && !Number.isInteger(n)) return { ok: false, error: { key: 'layers.err.integer' } };
  if (spec.positive && n <= 0) return { ok: false, error: { key: 'layers.err.positive' } };
  const max = field === 'order' && ctx.layerCount ? Math.max(spec.min, ctx.layerCount) : spec.max;
  if (n < spec.min || n > max) {
    const fmt = (v: number) => formatNumber(v, spec.decimals);
    return { ok: false, error: { key: 'layers.err.range', params: { min: fmt(spec.min), max: fmt(max), unit: spec.unit } } };
  }
  if (field === 'min_power' && ctx.power !== undefined && n > ctx.power) {
    return { ok: false, error: { key: 'layers.err.minAbovePower', params: { power: ctx.power } } };
  }
  if (field === 'power' && ctx.minPower !== undefined && n < ctx.minPower) {
    return { ok: false, error: { key: 'layers.err.powerBelowMin', params: { min: ctx.minPower } } };
  }
  if (field === 'tab_spacing' && n > 0 && ctx.tabWidth !== undefined && n <= ctx.tabWidth) {
    return { ok: false, error: { key: 'layers.err.spacingBelowWidth', params: { width: ctx.tabWidth } } };
  }
  return { ok: true, value: n };
}
