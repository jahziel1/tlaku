/**
 * Curva de tono de un material (vista previa «por tonos», mejora 8). Lógica pura, sin interfaz.
 *
 * `toneDarkness` y `toneMix` son copia de `ToneCurve::darkness`/`mix` de `crates/tlaku-doc/src/tonal.rs`
 * (el motor es quien dibuja la imagen; aquí se usan para la leyenda, la calibración y el modo
 * simulado); `curve.test.ts` repite los vectores de la prueba de Rust. Los valores de partida de
 * cada material salen del mismo archivo de datos que usa el motor.
 */
import type { ToneCalibration } from '@/bindings/ToneCalibration';
import type { ToneCurve } from '@/bindings/ToneCurve';
import type { ToneSample } from '@/bindings/ToneSample';
import presetData from '../../../../../crates/tlaku-doc/data/tone-presets.json';

export type { ToneCalibration, ToneCurve, ToneSample };

interface PresetRow {
  id: string;
  base: number[];
  burn: number[];
  /** Dosis característica por watt (W·s/mm²): `dose_char = dose_char_w / watts`. */
  dose_char_w: number;
  gamma: number;
  /** `medido` (en una máquina real) o `estimado`. */
  fuente: string;
}

const ROWS = presetData.presets as PresetRow[];

/** Watts que se suponen cuando el perfil no dice los del láser (`watts_por_defecto` del archivo de datos). */
export const DEFAULT_TONE_WATTS: number = presetData.watts_por_defecto;

/** Si el valor de partida de un material fue medido en una máquina real o es una estimación. */
export function presetSource(id: string): 'medido' | 'estimado' {
  return ROWS.find((r) => r.id === id)?.fuente === 'medido' ? 'medido' : 'estimado';
}

/** 6 cifras significativas, igual que `tone_preset_for` del motor (0,0924 y no 0,09240000000000001). */
const roundSig = (v: number) => Number(v.toPrecision(6));

/** Identificadores de los materiales de partida; el último, `custom`, es el personalizado. */
export const TONE_MATERIALS: readonly string[] = ROWS.map((r) => r.id);
export const CUSTOM_TONE_MATERIAL = 'custom';
export const DEFAULT_TONE_MATERIAL = 'light_wood';

/** Ancho del punto del láser que se supone si el perfil no lo dice (mm); los perfiles no lo guardan. */
export const DEFAULT_SPOT_MM = 0.1;
export const SPOT_RANGE = { min: 0.01, max: 1, step: 0.01 } as const;

const rgb = (v: number[]): [number, number, number] => [v[0] ?? 0, v[1] ?? 0, v[2] ?? 0];

/**
 * Curva de partida de un material (la del personalizado si no se conoce) para un láser de `watts`.
 * Los valores van por watt: un láser de 22 W llega al mismo tono con menos de la mitad de dosis que
 * uno de 10 W. Sin watts conocidos (o inválidos) se suponen `DEFAULT_TONE_WATTS`. Igual que
 * `tone_preset_for` del motor.
 */
export function presetCurve(id: string, watts?: number | null): ToneCurve {
  const row = ROWS.find((r) => r.id === id) ?? ROWS.find((r) => r.id === CUSTOM_TONE_MATERIAL) ?? ROWS[0]!;
  const w = watts != null && Number.isFinite(watts) && watts > 0 ? watts : DEFAULT_TONE_WATTS;
  return { base: rgb(row.base), burn: rgb(row.burn), dose_char: roundSig(row.dose_char_w / w), gamma: row.gamma };
}

/**
 * La curva calibrada del material si existe (ya es de la máquina y el material del usuario: no se
 * convierte con los watts) y, si no, la de partida para el láser de `watts`.
 */
export function curveFor(material: string, tones: readonly ToneCalibration[] | undefined, watts?: number | null): ToneCurve {
  return tones?.find((c) => c.material === material)?.curve ?? presetCurve(material, watts);
}

/** Oscuridad (0 = material sin tocar, 1 = tono máximo) para una dosis; monótona, acotada y sin NaN. */
export function toneDarkness(curve: ToneCurve, dose: number): number {
  if (Number.isNaN(dose) || dose <= 0) return 0;
  if (!Number.isFinite(dose)) return 1;
  const x = (dose / curve.dose_char) ** curve.gamma;
  return Math.min(1, Math.max(0, 1 - Math.exp(-x)));
}

/** Color RGB a una oscuridad (0..1). */
export function toneMix(curve: ToneCurve, darkness: number): [number, number, number] {
  const d = Number.isNaN(darkness) ? 0 : Math.min(1, Math.max(0, darkness));
  const ch = (i: number) => Math.min(255, Math.max(0, Math.round(curve.base[i]! + (curve.burn[i]! - curve.base[i]!) * d)));
  return [ch(0), ch(1), ch(2)];
}

export const toneColor = (curve: ToneCurve, dose: number) => toneMix(curve, toneDarkness(curve, dose));

/** `#rrggbb` de un color RGB. */
export function hexOf(c: readonly number[]): string {
  return `#${c.map((v) => Math.min(255, Math.max(0, Math.round(v))).toString(16).padStart(2, '0')).join('')}`;
}

/** Color RGB de un `#rrggbb`; `null` si no lo es. */
export function rgbOfHex(hex: string): [number, number, number] | null {
  const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex.trim());
  return m ? [parseInt(m[1]!, 16), parseInt(m[2]!, 16), parseInt(m[3]!, 16)] : null;
}

/**
 * Dosis de un trazo: pasadas x (potencia/100) / (velocidad en mm/s x ancho en mm). El ancho es el del
 * punto en una línea y el intervalo en un relleno o una imagen. Igual que `tone_dose` del motor.
 */
export function toneDose(powerPct: number, speedMmMin: number, passes: number, widthMm: number): number {
  const v = speedMmMin / 60;
  if (!(powerPct > 0 && v > 0 && widthMm > 0)) return 0;
  return (passes * (powerPct / 100)) / (v * widthMm);
}

// ---------------------------------------------------------------------------
// Ajuste con las observaciones del usuario
// ---------------------------------------------------------------------------

const MIN_DARK = 0.02;
const MAX_DARK = 0.98;
const GAMMA_RANGE = [0.5, 3] as const;
const DOSE_RANGE = [1e-4, 1e4] as const;

const round4 = (v: number) => Number(v.toPrecision(4));

/**
 * Ajusta la curva a las observaciones (`darkness` = a qué punto de la curva se parece la celda de
 * esa dosis). Con una sola, o si todas tienen la misma dosis, solo se mueve `dose_char` (la gamma se
 * conserva); con dos o más dosis distintas se ajustan `dose_char` y `gamma` por mínimos cuadrados
 * de `ln(-ln(1 - d)) = gamma (ln D - ln dose_char)`. Los colores no cambian. Sin observaciones
 * válidas devuelve la misma curva.
 */
export function fitCurve(curve: ToneCurve, samples: readonly ToneSample[]): ToneCurve {
  const pts = samples
    .filter((s) => Number.isFinite(s.dose) && s.dose > 0 && Number.isFinite(s.darkness))
    .map((s) => ({ t: Math.log(s.dose), y: Math.log(-Math.log(1 - Math.min(MAX_DARK, Math.max(MIN_DARK, s.darkness)))) }));
  if (pts.length === 0) return curve;
  const n = pts.length;
  const mt = pts.reduce((a, p) => a + p.t, 0) / n;
  const my = pts.reduce((a, p) => a + p.y, 0) / n;
  const varT = pts.reduce((a, p) => a + (p.t - mt) ** 2, 0);
  let gamma = curve.gamma;
  if (n >= 2 && varT > 1e-6) {
    const cov = pts.reduce((a, p) => a + (p.t - mt) * (p.y - my), 0);
    gamma = Math.min(GAMMA_RANGE[1], Math.max(GAMMA_RANGE[0], cov / varT));
  }
  // y = gamma (t - ln Dc): con la gamma elegida, ln Dc es el promedio de t - y / gamma.
  const lnDc = pts.reduce((a, p) => a + (p.t - p.y / gamma), 0) / n;
  const dose = Math.min(DOSE_RANGE[1], Math.max(DOSE_RANGE[0], Math.exp(lnDc)));
  return { ...curve, dose_char: round4(dose), gamma: round4(gamma) };
}

/** Lista de calibraciones con la de `material` reemplazada (o quitada si `next` es `null`). */
export function withCalibration(tones: readonly ToneCalibration[] | undefined, material: string, next: ToneCalibration | null): ToneCalibration[] {
  const rest = (tones ?? []).filter((c) => c.material !== material);
  return next ? [...rest, next] : rest;
}
