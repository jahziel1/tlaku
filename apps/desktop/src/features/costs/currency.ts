import type { Language } from '@/i18n';

/** Monedas que ofrece la calculadora (códigos ISO 4217). Nada se consulta en internet: el tipo de cambio no existe aquí. */
export const CURRENCIES = ['MXN', 'USD', 'EUR', 'GBP', 'CAD', 'COP', 'ARS', 'CLP', 'PEN', 'BRL', 'UYU', 'CRC', 'GTQ', 'DOP', 'BOB', 'PYG'] as const;

const BY_REGION: Record<string, string> = {
  MX: 'MXN',
  US: 'USD',
  PR: 'USD',
  EC: 'USD',
  SV: 'USD',
  PA: 'USD',
  ES: 'EUR',
  DE: 'EUR',
  FR: 'EUR',
  IT: 'EUR',
  PT: 'EUR',
  GB: 'GBP',
  CA: 'CAD',
  CO: 'COP',
  AR: 'ARS',
  CL: 'CLP',
  PE: 'PEN',
  BR: 'BRL',
  UY: 'UYU',
  CR: 'CRC',
  GT: 'GTQ',
  DO: 'DOP',
  BO: 'BOB',
  PY: 'PYG',
};

/** `es-MX` -> `MX`; `es` -> `null`. */
export function regionOf(tag: string | undefined | null): string | null {
  const m = /^[a-z]{2,3}[-_]([A-Za-z]{2})\b/.exec(tag ?? '');
  return m ? m[1]!.toUpperCase() : null;
}

/**
 * Moneda por omisión: la de la región del sistema (`es-MX` -> MXN, `es-CO` -> COP); sin región
 * conocida, MXN en español y USD en inglés.
 */
export function defaultCurrency(language: Language, systemTag?: string | null): string {
  const region = regionOf(systemTag);
  const byRegion = region ? BY_REGION[region] : undefined;
  if (byRegion) return byRegion;
  return language === 'en' ? 'USD' : 'MXN';
}

/**
 * Etiqueta regional para dar formato a números y dinero: la del sistema si coincide con el idioma de
 * la interfaz (`es-MX` con la interfaz en español), y si no la general del idioma.
 */
export function formatTag(language: Language, systemTag?: string | null): string {
  const tag = (systemTag ?? '').replace('_', '-');
  if (tag && tag.toLowerCase().startsWith(language) && regionOf(tag)) return tag;
  return language === 'en' ? 'en-US' : 'es-MX';
}

/** Decimales de la moneda (2 en MXN o USD, 0 en CLP o PYG). */
export function currencyDigits(code: string): number {
  try {
    return new Intl.NumberFormat('en', { style: 'currency', currency: code }).resolvedOptions().maximumFractionDigits ?? 2;
  } catch {
    return 2;
  }
}

/** Importe con el símbolo y el formato de la región (`$1,234.50`, `1.234,50 €`). */
export function formatMoney(value: number, code: string, tag: string): string {
  try {
    return new Intl.NumberFormat(tag, { style: 'currency', currency: code }).format(value);
  } catch {
    return `${code} ${value.toFixed(2)}`;
  }
}

/** Solo el símbolo (`$`, `€`), para etiquetar los campos de importe. */
export function currencySymbol(code: string, tag: string): string {
  try {
    const part = new Intl.NumberFormat(tag, { style: 'currency', currency: code, currencyDisplay: 'narrowSymbol' })
      .formatToParts(0)
      .find((p) => p.type === 'currency');
    return part?.value ?? code;
  } catch {
    return code;
  }
}

/** Número con punto decimal y sin separador de miles, para el CSV y las hojas de cálculo. */
export function plainNumber(value: number, digits: number): string {
  return Number.isFinite(value) ? value.toFixed(digits) : '';
}
