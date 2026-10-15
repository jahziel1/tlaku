/**
 * Estado por pestaña que vive FUERA del store principal (p. ej. las tablas CSV ya leídas de los
 * datos variables, que están en `useVariablesUi`). Cada función se registra aquí con una clave y
 * `tabs.ts` lo guarda en la foto de la pestaña al salir (`TabData.extras`) y lo restaura al volver,
 * igual que el documento. Es un módulo sin dependencias para que cualquier función pueda
 * registrarse sin crear ciclos con el store.
 */
export interface TabExtra<T = unknown> {
  /** Foto del estado vivo (la pestaña activa). */
  capture: () => T;
  /** Pone el estado de una pestaña; `undefined` = pestaña sin foto aún (estado inicial). */
  restore: (value: T | undefined) => void;
}

const registry = new Map<string, TabExtra>();

/** Registra (o reemplaza) el estado por pestaña `key`. Devuelve cómo quitarlo. */
export function registerTabExtra<T>(key: string, extra: TabExtra<T>): () => void {
  registry.set(key, extra as TabExtra);
  return () => {
    if (registry.get(key) === (extra as TabExtra)) registry.delete(key);
  };
}

/** Foto de todo lo registrado, o `undefined` si no hay nada. */
export function captureExtras(): Record<string, unknown> | undefined {
  if (registry.size === 0) return undefined;
  const out: Record<string, unknown> = {};
  for (const [key, extra] of registry) out[key] = extra.capture();
  return out;
}

/** Restaura lo registrado desde una foto (`undefined`: estado inicial de cada uno). */
export function restoreExtras(snapshot: Record<string, unknown> | undefined): void {
  for (const [key, extra] of registry) extra.restore(snapshot?.[key]);
}
