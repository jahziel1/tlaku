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

export { en, es };
