import { es } from './es';

export type I18nKey = keyof typeof es;

type Params = Record<string, string | number>;

/**
 * Traduce una clave del diccionario. Los marcadores `{nombre}` se reemplazan con `params`.
 * Todo texto visible de la interfaz sale de aquí (hoy solo español).
 */
export function t(key: I18nKey, params?: Params): string {
  const text: string = es[key];
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (_, name: string) => String(params[name] ?? `{${name}}`));
}

export { es };
