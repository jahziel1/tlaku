import type { FirmwareFamily } from './grblCodes';
import { effectiveRule, settingInfo } from './grblSettingsInfo';

/**
 * Lógica pura de la ventana «Ajustes de la máquina»: leer la respuesta de `$$` conservando el texto
 * de cada valor, validar lo que se escribe, armar y leer el respaldo, y comparar un respaldo con lo
 * que hay en la máquina. Nada aquí toca la máquina ni la interfaz.
 */

// ---------------------------------------------------------------------------
// Lectura de `$$`
// ---------------------------------------------------------------------------

/** Un ajuste tal como lo informó la controladora. */
export interface RawSetting {
  id: number;
  /** El valor exacto de la línea (`400.000`, `1`, o texto en ajustes que no son números). */
  raw: string;
  /** El valor numérico; `null` si el texto no es un número (por ejemplo un nombre de red). */
  value: number | null;
}

export interface ParsedSettings {
  settings: RawSetting[];
  /** Líneas `$nombre=valor` cuyo identificador no es un número (la vista de FluidNC): no se editan aquí. */
  namedLines: number;
}

const NUMBER = /^[+-]?\d+(?:\.\d+)?$/;

/** Lee las líneas `$n=valor` de la respuesta de `$$`; ignora `ok`, avisos y comentarios. Si un número se repite, vale el último. */
export function parseRawSettings(lines: readonly string[]): ParsedSettings {
  const byId = new Map<number, RawSetting>();
  let namedLines = 0;
  for (const line of lines) {
    const numbered = /^\s*\$(\d+)\s*=\s*(.*?)\s*$/.exec(line);
    if (numbered) {
      const raw = numbered[2]!;
      byId.set(Number(numbered[1]), { id: Number(numbered[1]), raw, value: NUMBER.test(raw) ? Number(raw) : null });
    } else if (/^\s*\$[^\s=]+\s*=/.test(line)) {
      namedLines++;
    }
  }
  return { settings: [...byId.values()].sort((a, b) => a.id - b.id), namedLines };
}

/** Tiene homing (`$22`) pero no límites por software (`$20=0`): se recomienda activarlos (nunca se escribe solo). */
export function needsSoftLimits(settings: readonly RawSetting[]): boolean {
  const value = (id: number) => settings.find((s) => s.id === id)?.value ?? null;
  const homing = value(22);
  return homing !== null && homing !== 0 && value(20) === 0;
}

// ---------------------------------------------------------------------------
// Validación de un valor escrito por la persona
// ---------------------------------------------------------------------------

export type ValidationError =
  | { key: 'mset.err.empty' }
  | { key: 'mset.err.notNumber' }
  | { key: 'mset.err.bool' }
  | { key: 'mset.err.integer' }
  | { key: 'mset.err.range'; min: number; max: number }
  | { key: 'mset.err.min'; min: number }
  | { key: 'mset.err.max'; max: number };

export type Validation = { ok: true; value: number; text: string } | { ok: false; error: ValidationError };

/** Texto de un número sin notación científica (con a lo más 6 decimales). */
function plain(value: number): string {
  const s = String(value);
  return /e/i.test(s) ? value.toFixed(6).replace(/\.?0+$/, '') : s;
}

/**
 * Valida el texto de un valor para `$id`: debe ser un número finito (se admite la coma decimal) y,
 * donde se conoce el ajuste, cumplir su tipo (0/1, entero, máscara) y su rango razonable. Devuelve
 * el texto normalizado que se enviaría, sin exponentes ni ceros de más.
 */
export function validateSettingValue(id: number, input: string, family: FirmwareFamily = 'unknown'): Validation {
  const trimmed = input.trim().replace(',', '.');
  if (trimmed === '') return { ok: false, error: { key: 'mset.err.empty' } };
  if (!NUMBER.test(trimmed)) return { ok: false, error: { key: 'mset.err.notNumber' } };
  const value = Number(trimmed);
  if (!Number.isFinite(value)) return { ok: false, error: { key: 'mset.err.notNumber' } };
  const info = settingInfo(id);
  if (info) {
    const rule = effectiveRule(info, family);
    if (rule.kind === 'bool' && value !== 0 && value !== 1) return { ok: false, error: { key: 'mset.err.bool' } };
    if (rule.kind !== 'float' && rule.kind !== 'bool' && !Number.isInteger(value)) return { ok: false, error: { key: 'mset.err.integer' } };
    const { min, max } = rule;
    if (min !== undefined && max !== undefined && (value < min || value > max)) return { ok: false, error: { key: 'mset.err.range', min, max } };
    if (min !== undefined && value < min) return { ok: false, error: { key: 'mset.err.min', min } };
    if (max !== undefined && value > max) return { ok: false, error: { key: 'mset.err.max', max } };
  } else if (Math.abs(value) > 1e9) {
    return { ok: false, error: { key: 'mset.err.range', min: -1e9, max: 1e9 } };
  }
  return { ok: true, value, text: plain(value) };
}

/** Parámetros de `t()` de un error de validación (`{min}` y `{max}` cuando el mensaje los usa). */
export function validationParams(error: ValidationError): Record<string, number> {
  const out: Record<string, number> = {};
  if ('min' in error) out.min = error.min;
  if ('max' in error) out.max = error.max;
  return out;
}

/** La línea exacta que se enviará para escribir `$id`. */
export function settingLine(id: number, valueText: string): string {
  return `$${id}=${valueText}`;
}

/** ¿Dos valores son el mismo? Compara como números si lo son (`25.000` = `25`) y como texto si no. */
export function sameValue(a: string, b: string): boolean {
  const x = a.trim();
  const y = b.trim();
  return NUMBER.test(x) && NUMBER.test(y) ? Number(x) === Number(y) : x === y;
}

// ---------------------------------------------------------------------------
// Respaldo
// ---------------------------------------------------------------------------

/** Primera línea de todo respaldo de Tlaku. */
export const BACKUP_HEADER = '; Tlaku: respaldo de ajustes GRBL';

/** Extensión que se propone al guardar un respaldo. */
export const BACKUP_EXTENSION = 'tlakuset';

/** Texto de un respaldo: encabezado con fecha y firmware, y una línea `$n=valor` por ajuste. */
export function buildBackupText(settings: readonly RawSetting[], firmware: string | null, date: Date): string {
  const lines = [
    BACKUP_HEADER,
    `; fecha: ${date.toISOString()}`,
    `; firmware: ${(firmware ?? 'desconocido').replace(/[\r\n]+/g, ' ')}`,
    ...[...settings].sort((a, b) => a.id - b.id).map((s) => settingLine(s.id, s.raw)),
  ];
  return `${lines.join('\n')}\n`;
}

export interface BackupEntry {
  id: number;
  raw: string;
}

export interface ParsedBackup {
  entries: BackupEntry[];
  /** Firmware anotado en el encabezado, si lo hay. */
  firmware: string | null;
  /** Líneas que no son ajustes ni comentarios (se ignoran). */
  ignored: string[];
  /** Números que aparecían más de una vez (vale el último). */
  duplicates: number[];
}

/** Lee un respaldo: las líneas `$n=valor`; los comentarios (`;` o paréntesis) y las líneas vacías se saltan. */
export function parseBackupText(text: string): ParsedBackup {
  const byId = new Map<number, BackupEntry>();
  const ignored: string[] = [];
  const duplicates: number[] = [];
  let firmware: string | null = null;
  for (const rawLine of text.replace(/^\uFEFF/, '').split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line === '') continue;
    if (line.startsWith(';')) {
      const fw = /^;\s*firmware:\s*(.+)$/i.exec(line);
      if (fw) firmware = fw[1]!.trim();
      continue;
    }
    const m = /^\$(\d+)\s*=\s*([^;\s][^;]*?)\s*(?:;.*)?$/.exec(line);
    if (!m) {
      ignored.push(line);
      continue;
    }
    const id = Number(m[1]);
    if (byId.has(id) && !duplicates.includes(id)) duplicates.push(id);
    byId.set(id, { id, raw: m[2]! });
  }
  return { entries: [...byId.values()].sort((a, b) => a.id - b.id), firmware, ignored, duplicates };
}

// ---------------------------------------------------------------------------
// Vista previa de diferencias
// ---------------------------------------------------------------------------

export interface DiffRow {
  id: number;
  /** Valor que hay ahora en la máquina. */
  current: string;
  /** Valor del respaldo, normalizado. */
  next: string;
  /** Línea exacta que se enviaría. */
  line: string;
}

export interface BackupDiff {
  /** Lo que cambiaría, por número ascendente. */
  changes: DiffRow[];
  /** Cuántos ajustes del respaldo ya valen lo mismo (se ignoran). */
  same: number;
  /** Ajustes del respaldo que esta máquina no informó: no se pueden escribir. */
  missing: number[];
  /** Ajustes del respaldo con un valor que no pasa la validación, con el error. */
  invalid: { id: number; raw: string; error: ValidationError }[];
}

/** Compara un respaldo con los ajustes actuales de la máquina. Los iguales se descartan. */
export function diffBackup(current: readonly RawSetting[], backup: readonly BackupEntry[], family: FirmwareFamily = 'unknown'): BackupDiff {
  const now = new Map(current.map((s) => [s.id, s]));
  const diff: BackupDiff = { changes: [], same: 0, missing: [], invalid: [] };
  for (const entry of backup) {
    const have = now.get(entry.id);
    if (!have) {
      diff.missing.push(entry.id);
      continue;
    }
    if (sameValue(have.raw, entry.raw)) {
      diff.same++;
      continue;
    }
    const check = validateSettingValue(entry.id, entry.raw, family);
    if (!check.ok) {
      diff.invalid.push({ id: entry.id, raw: entry.raw, error: check.error });
      continue;
    }
    diff.changes.push({ id: entry.id, current: have.raw, next: check.text, line: settingLine(entry.id, check.text) });
  }
  diff.changes.sort((a, b) => a.id - b.id);
  return diff;
}
