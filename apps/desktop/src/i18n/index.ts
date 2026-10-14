import { en } from './en';
import { es } from './es';

export type I18nKey = keyof typeof es;
export type Language = 'es' | 'en';

type Params = Record<string, string | number>;

let current: Language = 'es';

/** Idioma de la interfaz. Lo fija la preferencia `language`; la interfaz se vuelve a pintar aparte. */
export function setLanguage(lang: Language): void {
  current = lang;
}

export function getLanguage(): Language {
  return current;
}

/** Idioma del sistema: inglés si `navigator.language` empieza por «en»; en cualquier otro caso, español. */
export function detectLanguage(): Language {
  if (typeof window === 'undefined' || typeof navigator === 'undefined') return 'es';
  return /^en\b/i.test(navigator.language || '') ? 'en' : 'es';
}

/** Código regional para `Intl` y `localeCompare` según el idioma de la interfaz. */
export function locale(): string {
  return current === 'en' ? 'en-US' : 'es';
}

/**
 * Traduce una clave del diccionario. Los marcadores `{nombre}` se reemplazan con `params`.
 * Todo texto visible de la interfaz sale de aquí. En inglés, una clave sin traducir se muestra
 * en español en lugar de romper la interfaz.
 */
export function t(key: I18nKey, params?: Params): string {
  const text: string = (current === 'en' ? en[key] : undefined) ?? es[key];
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (_, name: string) => String(params[name] ?? `{${name}}`));
}

/** Claves que existen en pareja `<base>.one` / `<base>.other` (singular y plural). */
export type PluralBase = {
  [K in I18nKey]: K extends `${infer B}.one` ? (`${B}.other` extends I18nKey ? B : never) : never;
}[I18nKey];

/**
 * Traduce una clave con plural: elige `<base>.one` si `n` es 1 y `<base>.other` en cualquier otro caso
 * (el cero va en plural, en español y en inglés). `n` también llena el marcador `{n}`.
 */
export function tn(base: PluralBase, n: number, params?: Params): string {
  return t(`${base}.${n === 1 ? 'one' : 'other'}` as I18nKey, { ...params, n });
}

export { en, es };
