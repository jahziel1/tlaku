import { locale } from '@/i18n';

/**
 * Unidades de longitud. El documento, el motor y todas las cuentas internas trabajan siempre en
 * milímetros: las pulgadas son solo una forma de mostrar y de leer lo que escribe el usuario.
 * Todo texto con una longitud debería pasar por `formatLength` / `parseLength`.
 */

export type LengthUnit = 'mm' | 'in';

export const LENGTH_UNITS: readonly LengthUnit[] = ['mm', 'in'];
export const MM_PER_INCH = 25.4;

/** Símbolo para mostrar junto al número. */
export function unitSymbol(unit: LengthUnit): string {
  return unit;
}

/** Milímetros -> número en la unidad de visualización. */
export function toDisplay(mm: number, unit: LengthUnit): number {
  return unit === 'in' ? mm / MM_PER_INCH : mm;
}

/** Número en la unidad de visualización -> milímetros. */
export function fromDisplay(value: number, unit: LengthUnit): number {
  return unit === 'in' ? value * MM_PER_INCH : value;
}

/** Decimales por omisión: 2 en mm (centésimas) y 3 en pulgadas (milésimas). */
export function defaultDigits(unit: LengthUnit): number {
  return unit === 'in' ? 3 : 2;
}

export interface FormatOptions {
  /** Decimales; por omisión según la unidad. */
  digits?: number;
  /** Agrega el símbolo (`12,5 mm`). Por omisión, sí. */
  symbol?: boolean;
  /** Quita los ceros sobrantes (`12,5` en vez de `12,50`). Por omisión, no. */
  trim?: boolean;
}

/**
 * Texto de una longitud dada en mm, en la unidad pedida y con coma decimal (configuración regional
 * española, igual que `fmt`). `-0` se muestra como `0`.
 */
export function formatLength(mm: number, unit: LengthUnit = 'mm', opts: FormatOptions = {}): string {
  const { digits = defaultDigits(unit), symbol = true, trim = false } = opts;
  if (!Number.isFinite(mm)) return '—';
  const value = toDisplay(mm, unit);
  const text = new Intl.NumberFormat(locale(), {
    minimumFractionDigits: trim ? 0 : digits,
    maximumFractionDigits: digits,
    useGrouping: false,
  }).format(Math.abs(value) < 0.5 * 10 ** -digits ? 0 : value);
  return symbol ? `${text} ${unitSymbol(unit)}` : text;
}

const UNIT_WORDS: Record<string, LengthUnit | 'cm'> = {
  mm: 'mm',
  cm: 'cm',
  in: 'in',
  pulg: 'in',
  '"': 'in',
  '”': 'in',
};

/**
 * Lee lo que escribe el usuario y devuelve milímetros, o `null` si no se entiende.
 *
 * - Acepta coma o punto decimal: `12,5`, `12.5`.
 * - Un sufijo manda sobre `defaultUnit`: `2 in`, `2"`, `5 cm`, `30mm`.
 * - En pulgadas acepta fracciones: `1/2`, `1 1/2 in`, `3/8"`.
 */
export function parseLength(text: string, defaultUnit: LengthUnit = 'mm'): number | null {
  const m = /^\s*([-+]?)\s*(.*?)\s*(mm|cm|in|pulg|"|”)?\s*$/i.exec(text);
  if (!m) return null;
  const [, sign, body, word] = m;
  const unit = word ? UNIT_WORDS[word.toLowerCase()]! : defaultUnit;
  const number = parseNumber(body!, unit !== 'mm' && unit !== 'cm');
  if (number === null || !Number.isFinite(number)) return null;
  const value = sign === '-' ? -number : number;
  return unit === 'cm' ? value * 10 : fromDisplay(value, unit);
}

/** Decimal (`12,5`) o, si `fractions`, también `1/2` y `1 1/2`. */
function parseNumber(body: string, fractions: boolean): number | null {
  const s = body.trim();
  if (s === '') return null;
  if (fractions) {
    const mixed = /^(\d+)\s+(\d+)\s*\/\s*(\d+)$/.exec(s);
    if (mixed) return Number(mixed[1]) + safeDiv(Number(mixed[2]), Number(mixed[3]));
    const frac = /^(\d+)\s*\/\s*(\d+)$/.exec(s);
    if (frac) return safeDiv(Number(frac[1]), Number(frac[2]));
  }
  if (!/^(\d+([.,]\d*)?|[.,]\d+)$/.test(s)) return null;
  const n = Number(s.replace(',', '.'));
  return Number.isFinite(n) ? n : null;
}

function safeDiv(a: number, b: number): number {
  return b === 0 ? NaN : a / b;
}
