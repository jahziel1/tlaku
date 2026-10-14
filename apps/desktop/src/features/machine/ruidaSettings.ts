import type { RuidaSetting, RuidaSettingChange } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';

/**
 * Ajustes de la controladora Ruida (EXPERIMENTAL). El motor decide qué se puede escribir y en qué
 * rango (lista cerrada `tlaku_ruida::settings::WRITABLE`); aquí solo se convierten unidades para
 * mostrarlas y se revisan los números antes de pedirle al motor que los valide de nuevo.
 *
 * Unidades en la memoria de la controladora: velocidades en µm/s, aceleraciones en µm/s² y
 * longitudes en µm. Se muestran en mm/s, mm/s² y mm; el paso de un eje se deja en µm y el
 * rotativo sin unidad (ninguna fuente pública la dice).
 */
export type SettingKind = RuidaSetting['kind'];

/** Cuántas unidades de la controladora hay en una unidad mostrada. */
export function factor(kind: SettingKind): number {
  return kind === 'speed' || kind === 'accel' || kind === 'length' ? 1000 : 1;
}

export function unitKey(kind: SettingKind): I18nKey {
  return `ruida.set.unit.${kind}` as I18nKey;
}

/** Valor de la controladora a la unidad mostrada. */
export function toDisplay(kind: SettingKind, raw: number): number {
  return raw / factor(kind);
}

/** Cifras decimales al mostrar. */
function digits(kind: SettingKind): number {
  return factor(kind) === 1000 ? 3 : 0;
}

/** Número tal como se escribe en el campo de edición (sin separador de miles). */
export function editText(kind: SettingKind, raw: number): string {
  const v = toDisplay(kind, raw);
  return Number.isInteger(v) ? String(v) : String(Number(v.toFixed(3)));
}

/** `400,000` a `400` y `12,500` a `12,5`: sin ceros sobrantes tras el separador decimal (solo si se pidieron decimales). */
function trimZeros(text: string, decimals: number): string {
  return decimals > 0 && /[.,]/.test(text) ? text.replace(/0+$/, '').replace(/[.,]$/, '') : text;
}

/** Valor con su unidad, para tablas (`400 mm/s`). El rotativo va sin unidad. */
export function formatValue(kind: SettingKind, raw: number, unit: string): string {
  const text = trimZeros(fmt(toDisplay(kind, raw), digits(kind)), digits(kind));
  return kind === 'raw' ? text : `${text} ${unit}`;
}

export type EditError =
  | { key: 'ruida.set.bad.number' }
  | { key: 'ruida.set.bad.integer'; unit: string }
  | { key: 'ruida.set.bad.range'; min: string; max: string; unit: string };

/**
 * Texto del campo a valor de la controladora (entero). Acepta coma o punto decimal. Devuelve el
 * error si no es un número, si tiene más cifras de las que admite la unidad o si sale del rango
 * que informó el motor.
 */
export function parseEdit(s: Pick<RuidaSetting, 'kind' | 'min' | 'max'>, text: string, unit: string): { raw: number } | { error: EditError } {
  const clean = text.trim().replace(',', '.');
  if (clean === '' || !/^[+-]?\d*\.?\d+$|^[+-]?\d+\.$/.test(clean)) return { error: { key: 'ruida.set.bad.number' } };
  const shown = Number(clean);
  if (!Number.isFinite(shown)) return { error: { key: 'ruida.set.bad.number' } };
  const scaled = shown * factor(s.kind);
  const raw = Math.round(scaled);
  if (Math.abs(scaled - raw) > 1e-6) return { error: { key: 'ruida.set.bad.integer', unit } };
  if (raw < s.min || raw > s.max) {
    return { error: { key: 'ruida.set.bad.range', min: trimZeros(fmt(toDisplay(s.kind, s.min), digits(s.kind)), digits(s.kind)), max: trimZeros(fmt(toDisplay(s.kind, s.max), digits(s.kind)), digits(s.kind)), unit } };
  }
  return { raw };
}

/** Rango permitido como texto (`1 a 600 mm/s`). */
export function rangeText(s: Pick<RuidaSetting, 'kind' | 'min' | 'max'>, unit: string): string {
  const a = trimZeros(fmt(toDisplay(s.kind, s.min), digits(s.kind)), digits(s.kind));
  const b = trimZeros(fmt(toDisplay(s.kind, s.max), digits(s.kind)), digits(s.kind));
  return s.kind === 'raw' ? `${a} – ${b}` : `${a} – ${b} ${unit}`;
}

/**
 * Cambios pendientes: de los textos editados (por clave), los que son un número válido y distinto
 * del valor actual, más los errores de los que no lo son. Un campo vacío no es un cambio.
 */
export function pendingChanges(
  settings: readonly RuidaSetting[],
  edits: Readonly<Record<string, string>>,
  unitOf: (kind: SettingKind) => string,
): { changes: RuidaSettingChange[]; errors: Record<string, EditError> } {
  const changes: RuidaSettingChange[] = [];
  const errors: Record<string, EditError> = {};
  for (const s of settings) {
    const text = edits[s.key];
    if (text === undefined || text.trim() === '') continue;
    const r = parseEdit(s, text, unitOf(s.kind));
    if ('error' in r) errors[s.key] = r.error;
    else if (r.raw !== s.value) changes.push({ key: s.key, value: r.raw });
  }
  return { changes, errors };
}

const GROUP_ORDER = ['general', 'axis_x', 'axis_y', 'axis_z', 'axis_u', 'rotary'] as const;

/** Ajustes agrupados en el orden de la interfaz; los grupos que el motor no declara van al final. */
export function groupSettings(settings: readonly RuidaSetting[]): [string, RuidaSetting[]][] {
  const by = new Map<string, RuidaSetting[]>();
  for (const s of settings) by.set(s.group, [...(by.get(s.group) ?? []), s]);
  const known = GROUP_ORDER.filter((g) => by.has(g)).map((g) => [g, by.get(g)!] as [string, RuidaSetting[]]);
  const rest = [...by.entries()].filter(([g]) => !(GROUP_ORDER as readonly string[]).includes(g));
  return [...known, ...rest];
}

/** Los ejes cuyo paso se puede calibrar, en el orden en que se ofrecen. */
export const CALIBRATABLE = ['x_step', 'y_step', 'z_step', 'u_step'] as const;

/** Nombre del archivo de respaldo manual. */
export const BACKUP_EXTENSION = 'tlakuruida';
