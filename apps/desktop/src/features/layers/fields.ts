import type { I18nKey } from '@/i18n';

/**
 * Campos numéricos del editor de capas: unidad, rango y validación. Un valor inválido se
 * marca en la interfaz y NO se aplica al documento. Los rangos siguen los del motor
 * (`tlaku-pipeline`: `stroke.rs`, `layers.rs`, `tabs.rs`): lo que él rechaza, el editor no lo deja
 * escribir, y lo que acepta (dentro de un tope práctico), el editor lo acepta.
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
  | 'tab_power'
  | 'halftone_cell'
  | 'halftone_angle'
  | 'z_offset'
  | 'z_step'
  | 'u_offset'
  | 'perf_cut'
  | 'perf_skip'
  | 'lead_length'
  | 'lead_angle'
  | 'overcut'
  | 'delay'
  | 'cut_through_power'
  | 'cut_through_ms'
  | 'dot_dwell'
  | 'dot_spacing'
  | 'ramp_length'
  | 'ramp_start'
  | 'angle_increment'
  | 'enhance_radius'
  | 'enhance_amount'
  | 'denoise'
  | 'galvo_speed'
  | 'galvo_freq'
  | 'galvo_repeat'
  | 'galvo_pause'
  | 'galvo_ppi'
  | 'galvo_wobble_size'
  | 'galvo_wobble_step'
  | 'galvo_delay'
  | 'galvo_jump_speed'
  | 'galvo_jump_delay'
  | 'galvo_slices'
  | 'galvo_angle_step';

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
  // Potencia de los puentes: 0 = apagado (no marca); el motor la limita a la del corte.
  tab_power: { min: 0, max: 100, step: 1, decimals: 1, unit: '%' },
  halftone_cell: { min: 0.05, max: 10, step: 0.05, decimals: 3, unit: 'mm', positive: true },
  halftone_angle: { min: -360, max: 360, step: 1, decimals: 2, unit: '°' },
  z_offset: { min: -100, max: 100, step: 0.1, decimals: 3, unit: 'mm' },
  z_step: { min: -100, max: 100, step: 0.1, decimals: 3, unit: 'mm' },
  // Eje U (rodillo o segundo eje) por capa: relativo al U del inicio del trabajo.
  u_offset: { min: -1000, max: 1000, step: 1, decimals: 3, unit: 'mm' },
  // Ola 2. Un tramo de perforación, de entrada o de rampa de 0 no hace nada: debe ser mayor que 0.
  perf_cut: { min: 0.01, max: 100, step: 0.1, decimals: 3, unit: 'mm', positive: true },
  perf_skip: { min: 0.01, max: 100, step: 0.1, decimals: 3, unit: 'mm', positive: true },
  lead_length: { min: 0.01, max: 50, step: 0.5, decimals: 3, unit: 'mm', positive: true },
  // Igual que el motor (`stroke.rs`): mayor que 0 y hasta 90° en recta o 180° de barrido en arco.
  lead_angle: { min: 0, max: 180, step: 5, decimals: 1, unit: '°', positive: true },
  overcut: { min: 0, max: 50, step: 0.1, decimals: 3, unit: 'mm' },
  // Pausa con el láser encendido, en milisegundos (0 = sin pausa).
  delay: { min: 0, max: 10000, step: 10, decimals: 0, unit: 'ms' },
  cut_through_power: { min: 0, max: 100, step: 1, decimals: 1, unit: '%' },
  // El motor exige un tiempo mayor que 0 ms.
  cut_through_ms: { min: 0, max: 10000, step: 10, decimals: 0, unit: 'ms', positive: true },
  dot_dwell: { min: 0.01, max: 1000, step: 0.5, decimals: 2, unit: 'ms', positive: true },
  dot_spacing: { min: 0.01, max: 50, step: 0.1, decimals: 3, unit: 'mm', positive: true },
  ramp_length: { min: 0.01, max: 50, step: 0.5, decimals: 3, unit: 'mm', positive: true },
  ramp_start: { min: 0, max: 100, step: 5, decimals: 1, unit: '%' },
  angle_increment: { min: -360, max: 360, step: 1, decimals: 2, unit: '°' },
  // Realce de bordes: radio en píxeles de la imagen (0 = apagado) y fuerza de 0 a 1.
  enhance_radius: { min: 0, max: 20, step: 0.5, decimals: 2, unit: 'px' },
  enhance_amount: { min: 0, max: 1, step: 0.05, decimals: 2, unit: '' },
  denoise: { min: 0, max: 20, step: 1, decimals: 1, unit: 'px' },
  // Ola 5, galvo. La velocidad se muestra en mm/s (la capa guarda mm/min = mm/s × 60).
  galvo_speed: { min: 1, max: 30000, step: 50, decimals: 1, unit: 'mm/s' },
  galvo_freq: { min: 0.1, max: 4000, step: 1, decimals: 2, unit: 'kHz', positive: true },
  // El motor acepta de 1 a 100000 repeticiones por capa.
  galvo_repeat: { min: 1, max: 100000, step: 1, decimals: 0, unit: '', integer: true },
  galvo_pause: { min: 0, max: 60000, step: 10, decimals: 0, unit: 'ms' },
  // Pulsos por pulgada: 0 = apagado.
  galvo_ppi: { min: 0, max: 5000, step: 10, decimals: 1, unit: 'PPI' },
  // El bamboleo exige tamaño y paso mayores que 0.
  galvo_wobble_size: { min: 0.01, max: 10, step: 0.05, decimals: 3, unit: 'mm', positive: true },
  galvo_wobble_step: { min: 0.005, max: 5, step: 0.01, decimals: 3, unit: 'mm', positive: true },
  // Retardos de la tarjeta: pueden ser negativos.
  galvo_delay: { min: -10000, max: 10000, step: 10, decimals: 1, unit: 'µs' },
  galvo_jump_speed: { min: 1, max: 30000, step: 100, decimals: 0, unit: 'mm/s', positive: true },
  galvo_jump_delay: { min: 0, max: 10000, step: 10, decimals: 0, unit: 'µs' },
  // De 2 a 256 cortes (8 bits dan como mucho 256 niveles).
  galvo_slices: { min: 2, max: 256, step: 1, decimals: 0, unit: '', integer: true },
  galvo_angle_step: { min: -360, max: 360, step: 1, decimals: 2, unit: '°' },
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
  /** Tipo de la entrada o salida: en recta el ángulo llega a 90°, en arco el barrido a 180°. */
  leadKind?: 'straight' | 'arc';
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
  let max = field === 'order' && ctx.layerCount ? Math.max(spec.min, ctx.layerCount) : spec.max;
  if (field === 'lead_angle' && ctx.leadKind === 'straight') max = 90;
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
