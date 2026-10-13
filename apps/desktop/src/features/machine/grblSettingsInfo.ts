import type { I18nKey } from '@/i18n';
import type { FirmwareFamily } from './grblCodes';

/**
 * Metadatos de los ajustes `$n` de GRBL 1.1 y de los que añade grblHAL. Solo datos: el nombre y la
 * descripción de cada ajuste viven en el diccionario (`mset.info.<n>.name` y `.desc`).
 *
 * Son datos de conocimiento público del protocolo (GRBL 1.1, grblHAL); lo que no está aquí se
 * muestra como «Ajuste sin descripción» y se puede editar solo con una validación mínima.
 */

/** Cómo se interpreta el valor: casilla 0/1, máscara de bits, entero o decimal. */
export type SettingKind = 'bool' | 'bits' | 'int' | 'float';

/** Unidad que se muestra junto al valor. `steps/mm` y `mask` se traducen; el resto son símbolos. */
export type SettingUnit = 'µs' | 'ms' | 'mm' | 'mm/min' | 'mm/s²' | 'steps/mm' | 'S' | 'Hz' | '%' | 'mask' | 'bool';

export interface GrblSettingInfo {
  id: number;
  kind: SettingKind;
  unit?: SettingUnit;
  /** Límites razonables en GRBL 1.1 (inclusivos). Sin límite = solo se pide un número finito. */
  min?: number;
  max?: number;
  /** En grblHAL el ajuste admite hasta este valor (por ejemplo, una casilla que allí es máscara). */
  halMax?: number;
  /** Con un valor equivocado es peligroso: mueve mal la máquina, la desprotege o apaga el modo láser. */
  critical?: boolean;
  /** Existe solo en grblHAL. */
  halOnly?: boolean;
}

const POS = 0.001;

const grbl = (id: number, kind: SettingKind, rest: Partial<GrblSettingInfo> = {}): GrblSettingInfo => ({ id, kind, ...rest });
const hal = (id: number, kind: SettingKind, rest: Partial<GrblSettingInfo> = {}): GrblSettingInfo => ({ id, kind, halOnly: true, ...rest });

const INFO_LIST: GrblSettingInfo[] = [
  // ---- GRBL 1.1 --------------------------------------------------------------------------
  grbl(0, 'float', { unit: 'µs', min: 1, max: 255 }),
  grbl(1, 'int', { unit: 'ms', min: 0, max: 255 }),
  grbl(2, 'bits', { unit: 'mask', min: 0, max: 7, halMax: 63 }),
  grbl(3, 'bits', { unit: 'mask', min: 0, max: 7, halMax: 63, critical: true }),
  grbl(4, 'bool', { unit: 'bool', halMax: 63 }),
  grbl(5, 'bool', { unit: 'bool', halMax: 63, critical: true }),
  grbl(6, 'bool', { unit: 'bool' }),
  grbl(10, 'bits', { unit: 'mask', min: 0, max: 3, halMax: 4095 }),
  grbl(11, 'float', { unit: 'mm', min: 0, max: 10 }),
  grbl(12, 'float', { unit: 'mm', min: 0.0001, max: 10 }),
  grbl(13, 'bool', { unit: 'bool' }),
  grbl(20, 'bool', { unit: 'bool', critical: true }),
  grbl(21, 'bool', { unit: 'bool', critical: true }),
  grbl(22, 'bool', { unit: 'bool', halMax: 255, critical: true }),
  grbl(23, 'bits', { unit: 'mask', min: 0, max: 7, halMax: 63, critical: true }),
  grbl(24, 'float', { unit: 'mm/min', min: 1, max: 100000 }),
  grbl(25, 'float', { unit: 'mm/min', min: 1, max: 100000 }),
  grbl(26, 'int', { unit: 'ms', min: 0, max: 1000 }),
  grbl(27, 'float', { unit: 'mm', min: 0, max: 100 }),
  grbl(30, 'float', { unit: 'S', min: 1, max: 1000000, critical: true }),
  grbl(31, 'float', { unit: 'S', min: 0, max: 1000000 }),
  grbl(32, 'bool', { unit: 'bool', halMax: 2, critical: true }),
  ...[100, 101, 102].map((id) => grbl(id, 'float', { unit: 'steps/mm', min: POS, max: 100000, critical: true })),
  ...[110, 111, 112].map((id) => grbl(id, 'float', { unit: 'mm/min', min: POS, max: 1000000, critical: true })),
  ...[120, 121, 122].map((id) => grbl(id, 'float', { unit: 'mm/s²', min: POS, max: 100000, critical: true })),
  ...[130, 131, 132].map((id) => grbl(id, 'float', { unit: 'mm', min: POS, max: 100000, critical: true })),

  // ---- añadidos de grblHAL (solo los que se conocen con certeza) -------------------------------
  hal(14, 'bits', { unit: 'mask', min: 0, max: 255 }),
  hal(15, 'bits', { unit: 'mask', min: 0, max: 7 }),
  hal(16, 'bits', { unit: 'mask', min: 0, max: 7 }),
  hal(17, 'bits', { unit: 'mask', min: 0, max: 255 }),
  hal(18, 'bits', { unit: 'mask', min: 0, max: 63 }),
  hal(19, 'bool', { unit: 'bool' }),
  hal(28, 'float', { unit: 'mm', min: 0, max: 100 }),
  hal(29, 'float', { unit: 'µs', min: 0, max: 1000 }),
  hal(33, 'float', { unit: 'Hz', min: 1, max: 1000000 }),
  hal(34, 'float', { unit: '%', min: 0, max: 100 }),
  hal(35, 'float', { unit: '%', min: 0, max: 100 }),
  hal(36, 'float', { unit: '%', min: 0, max: 100 }),
  hal(37, 'bits', { unit: 'mask', min: 0, max: 63 }),
  hal(38, 'int', { min: 0, max: 65535 }),
  hal(39, 'bool', { unit: 'bool' }),
  hal(40, 'bool', { unit: 'bool' }),
  hal(43, 'int', { min: 0, max: 16 }),
  hal(44, 'bits', { unit: 'mask', min: 0, max: 63 }),
  hal(45, 'bits', { unit: 'mask', min: 0, max: 63 }),
  hal(46, 'bits', { unit: 'mask', min: 0, max: 63 }),
  hal(60, 'bool', { unit: 'bool' }),
  hal(61, 'bool', { unit: 'bool' }),
  hal(62, 'bool', { unit: 'bool' }),
  hal(63, 'bool', { unit: 'bool' }),
  hal(64, 'bool', { unit: 'bool' }),
];

const BY_ID = new Map<number, GrblSettingInfo>(INFO_LIST.map((i) => [i.id, i]));

/** Todos los ajustes con metadatos, por número ascendente. */
export const GRBL_SETTINGS_INFO: readonly GrblSettingInfo[] = [...INFO_LIST].sort((a, b) => a.id - b.id);

/** Metadatos del ajuste `$id`, o `undefined` si no se conoce. */
export function settingInfo(id: number): GrblSettingInfo | undefined {
  return BY_ID.get(id);
}

/** Clave del nombre del ajuste (el diccionario trae `mset.info.<n>.name`); sin metadatos, un texto genérico. */
export function settingNameKey(id: number): I18nKey {
  return (BY_ID.has(id) ? `mset.info.${id}.name` : 'mset.unknown.name') as I18nKey;
}

/** Clave de la descripción corta del ajuste. */
export function settingDescKey(id: number): I18nKey {
  return (BY_ID.has(id) ? `mset.info.${id}.desc` : 'mset.unknown.desc') as I18nKey;
}

/** ¿Es un ajuste peligroso con un valor equivocado? */
export function isCriticalSetting(id: number): boolean {
  return BY_ID.get(id)?.critical === true;
}

/** Reglas efectivas de un ajuste según el firmware (en grblHAL algunas casillas son máscaras o modos). */
export function effectiveRule(info: GrblSettingInfo, family: FirmwareFamily): { kind: SettingKind; min: number | undefined; max: number | undefined } {
  if (family === 'grblhal' && info.halMax !== undefined) {
    return { kind: info.kind === 'float' ? 'float' : 'int', min: info.min ?? 0, max: info.halMax };
  }
  if (info.kind === 'bool') return { kind: 'bool', min: 0, max: 1 };
  return { kind: info.kind, min: info.min, max: info.max };
}
