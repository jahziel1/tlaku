import type { DeviceProfile, LaserSource } from '@/api/types';
import type { I18nKey } from '@/i18n';

/** Mismos tope y piso que el motor (`calib.rs`): fuera de ellos la corrección deja de ser monótona. */
export const MAX_NONLINEAR = 0.3;
export const MIN_SCALE = 0.1;
export const MAX_SCALE = 10;
/** Topes del motor (`safety.rs`): desfase del puntero rojo y overscan del perfil, en mm. */
export const MAX_POINTER_OFFSET_MM = 50;
export const MAX_OVERSCAN_MM = 20;
/** Tope de pasos por vuelta del rotativo del galvo (solo para cazar un cero de más). */
export const MAX_ROTARY_STEPS = 10_000_000;
/** Tope de repeticiones del marcado repetido (el motor: `MAX_REPEATS`). */
export const MAX_REPEATS = 100_000;

/** Fuentes en el orden en que se ofrecen; el texto sale de `galvo.source.<id>`. */
export const GALVO_SOURCES: readonly LaserSource[] = ['fiber_q', 'fiber_mopa', 'uv', 'co2'];

/** Lentes F-theta habituales (lado del campo, mm). */
export const COMMON_FIELDS_MM = [70, 110, 150, 175, 200, 300] as const;

/** ¿Esta fuente lleva ancho de pulso por comando propio? (fibra MOPA y UV). */
export function usesPulseWidth(source: LaserSource): boolean {
  return source === 'fiber_mopa' || source === 'uv';
}

/** Una fuente MOPA exige el ancho de pulso; en UV es opcional. */
export function requiresPulseWidth(source: LaserSource): boolean {
  return source === 'fiber_mopa';
}

/** `4, 6, 9` -> `[4, 6, 9]`; `null` si algo no es un entero positivo. Admite comas, espacios y puntos y coma. */
export function parsePulseWidths(text: string): number[] | null {
  const parts = text.split(/[\s,;]+/).filter((p) => p !== '');
  const out = parts.map(Number);
  return out.every((n) => Number.isInteger(n) && n > 0 && n <= 100_000) ? [...new Set(out)].sort((a, b) => a - b) : null;
}

export function formatPulseWidths(list: readonly number[]): string {
  return list.join(', ');
}

export type GalvoField =
  | 'origin'
  | 'field_mm'
  | 'watts'
  | 'freq'
  | 'pulse_widths'
  | 'max_power_pct'
  | 'light_speed'
  | 'timing'
  | 'jump'
  | 'scale'
  | 'nonlinear'
  | 'rotation'
  | 'cylinder_diameter'
  | 'cylinder_distance'
  | 'taper'
  | 'piece_mm'
  | 'overlap_mm'
  | 'pause_ms'
  | 'repeat_count'
  | 'increment'
  | 'overscan'
  | 'pointer_offset'
  | 'interlock'
  | 'rotary_diameter'
  | 'rotary_steps';

export type GalvoIssues = Partial<Record<GalvoField, I18nKey>>;

const finite = (n: number) => Number.isFinite(n);
const pos = (n: number) => finite(n) && n > 0;

/**
 * Errores del perfil galvo por campo (vacío si es válido). Refleja las comprobaciones del motor
 * (`SafetyLimits::validate`, `calib.rs`, `cylinder.rs`, `split.rs`): lo que él rechazaría al conectar o
 * calcular, la interfaz lo marca antes de guardar.
 */
export function validateGalvoProfile(d: Pick<DeviceProfile, 'galvo' | 'origin'> & Partial<Pick<DeviceProfile, 'pointer_offset'>>): GalvoIssues {
  const g = d.galvo;
  const out: GalvoIssues = {};
  // El galvo coloca el campo con las coordenadas del documento: solo vale el origen abajo a la izquierda.
  if (d.origin !== 'bottom_left') out.origin = 'galvo.err.origin';
  if (!pos(g.field_mm) || g.field_mm > 2000) out.field_mm = 'galvo.err.field';
  if (!pos(g.watts)) out.watts = 'galvo.err.positive';
  if (!pos(g.freq_min_khz) || !finite(g.freq_max_khz) || g.freq_min_khz > g.freq_max_khz) out.freq = 'galvo.err.freq';
  if (requiresPulseWidth(g.source) && g.pulse_widths_ns.length === 0) out.pulse_widths = 'galvo.err.pulseRequired';
  else if (g.pulse_widths_ns.some((n) => !Number.isInteger(n) || n <= 0)) out.pulse_widths = 'galvo.err.pulseList';
  if (!pos(g.max_power_pct) || g.max_power_pct > 100) out.max_power_pct = 'galvo.err.maxPower';
  if (!pos(g.light_speed)) out.light_speed = 'galvo.err.positive';
  const t = g.timing;
  if (![t.laser_on_us, t.laser_off_us, t.end_us, t.polygon_us].every(finite)) out.timing = 'galvo.err.number';
  const j = g.jump;
  if (!pos(j.speed)) out.jump = 'galvo.err.positive';
  else if (![j.delay_min_us, j.delay_max_us, j.distance_limit].every((n) => finite(n) && n >= 0)) out.jump = 'galvo.err.nonNegative';
  else if (j.delay_min_us > j.delay_max_us) out.jump = 'galvo.err.jumpOrder';
  const c = g.correction;
  if (!c.scale.every((n) => finite(n) && n >= MIN_SCALE && n <= MAX_SCALE)) out.scale = 'galvo.err.scale';
  if (![...c.bulge, ...c.trapezoid, ...c.skew].every((n) => finite(n) && Math.abs(n) <= MAX_NONLINEAR)) out.nonlinear = 'galvo.err.nonlinear';
  if (![...c.offset, c.rotation_deg].every(finite)) out.rotation = 'galvo.err.number';
  const cy = g.cylinder;
  if (cy.enabled) {
    if (!pos(cy.diameter_mm)) out.cylinder_diameter = 'galvo.err.positive';
    if (!pos(cy.mirror_distance_mm)) out.cylinder_distance = 'galvo.err.positive';
  }
  const tp = g.taper;
  if (tp.enabled && !(pos(tp.top_diameter_mm) && pos(tp.bottom_diameter_mm) && pos(tp.length_mm))) out.taper = 'galvo.err.positive';
  else if (tp.enabled && cy.enabled && tp.axis !== cy.axis) out.taper = 'galvo.err.axis';
  const sp = g.split;
  if (!finite(sp.piece_mm) || sp.piece_mm < 0) out.piece_mm = 'galvo.err.nonNegative';
  const side = sp.piece_mm > 0 ? Math.min(sp.piece_mm, g.field_mm) : g.field_mm;
  if (!finite(sp.overlap_mm) || sp.overlap_mm < 0) out.overlap_mm = 'galvo.err.nonNegative';
  else if (sp.enabled && sp.overlap_mm >= side) out.overlap_mm = 'galvo.err.overlap';
  if (!finite(sp.pause_after_move_ms) || sp.pause_after_move_ms < 0 || !finite(g.repeat.pause_ms) || g.repeat.pause_ms < 0) out.pause_ms = 'galvo.err.nonNegative';
  const r = g.repeat;
  if (!Number.isInteger(r.count) || r.count < 1 || r.count > MAX_REPEATS) out.repeat_count = 'galvo.err.repeat';
  if (!r.increment_mm.every(finite)) out.increment = 'galvo.err.number';
  if (!finite(g.overscan_mm) || g.overscan_mm < 0 || g.overscan_mm > MAX_OVERSCAN_MM) out.overscan = 'galvo.err.overscan';
  if (d.pointer_offset && !d.pointer_offset.every((n) => finite(n) && Math.abs(n) <= MAX_POINTER_OFFSET_MM)) out.pointer_offset = 'galvo.err.pointerOffset';
  const ro = g.rotary;
  if (ro.enabled && !pos(ro.diameter_mm)) out.rotary_diameter = 'galvo.err.rotaryDiameter';
  if (ro.enabled && (!Number.isInteger(ro.steps_per_rev) || ro.steps_per_rev < 0 || ro.steps_per_rev > MAX_ROTARY_STEPS)) out.rotary_steps = 'galvo.err.rotarySteps';
  if (g.require_input && (g.require_input.value & ~g.require_input.mask) !== 0) out.interlock = 'galvo.err.interlock';
  return out;
}
