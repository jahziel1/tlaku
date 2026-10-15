import { useEffect, useState } from 'react';
import { getApi } from '@/api';
import type { FontCapabilities, TextObject } from '@/api/types';

/** Espera tras el último cambio de ejes antes de pedir las vistas previas al motor. */
export const CAPABILITIES_DEBOUNCE_MS = 200;
const CACHE_LIMIT = 24;

const cache = new Map<string, Promise<FontCapabilities>>();

/** Lo que cambia las funciones y vistas previas: la cara (familia, negrita, cursiva) y la posición de los ejes. */
export function capabilitiesKey(kind: TextObject, family: string): string {
  return JSON.stringify([family.trim().toLowerCase(), kind.bold, kind.italic, (kind.variations ?? []).map((a) => [a.tag, a.value])]);
}

/** Solo la cara (sin la posición de los ejes): lo que decide qué ejes y funciones existen. */
export function faceKey(kind: TextObject, family: string): string {
  return JSON.stringify([family.trim().toLowerCase(), kind.bold, kind.italic]);
}

/** Pide (con memoria) las funciones y ejes de la fuente del texto. */
export function loadCapabilities(kind: TextObject, family: string, api: () => Pick<ReturnType<typeof getApi>, 'fontCapabilities'> = getApi): Promise<FontCapabilities> {
  const key = capabilitiesKey(kind, family);
  const hit = cache.get(key);
  if (hit) return hit;
  // Solo la cara y los ejes importan: el contenido, las funciones y el contorno no se mandan.
  const text: TextObject = { ...kind, text: '', font_family: family, features: undefined, outline: [], on_path: null };
  const pending = api().fontCapabilities({ text });
  cache.set(key, pending);
  if (cache.size > CACHE_LIMIT) cache.delete(cache.keys().next().value as string);
  // Un fallo no se recuerda: la próxima vez se vuelve a intentar.
  pending.catch(() => cache.delete(key));
  return pending;
}

/** Vacía la memoria (pruebas y recarga de fuentes). */
export function clearCapabilitiesCache(): void {
  cache.clear();
}

export interface CapabilitiesState {
  caps: FontCapabilities | null;
  loading: boolean;
  error: string | null;
}

/**
 * Funciones y ejes de la fuente de un texto. Mientras llega la respuesta de otra posición de ejes
 * se conserva la anterior, para que los controles no parpadeen al mover un deslizador.
 */
export function useFontCapabilities(kind: TextObject, family: string, enabled: boolean): CapabilitiesState {
  const key = capabilitiesKey(kind, family);
  const face = faceKey(kind, family);
  const [state, setState] = useState<CapabilitiesState & { key: string; face: string }>({ caps: null, loading: false, error: null, key: '', face: '' });

  useEffect(() => {
    if (!enabled) return;
    let alive = true;
    const timer = setTimeout(() => {
      setState((s) => ({ ...s, loading: true, error: null }));
      loadCapabilities(kind, family).then(
        (caps) => alive && setState({ caps, loading: false, error: null, key, face }),
        (e: unknown) => alive && setState({ caps: null, loading: false, error: String(e instanceof Error ? e.message : e), key, face }),
      );
    }, CAPABILITIES_DEBOUNCE_MS);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
    // `kind` cambia con cada tecla; la clave resume lo que de verdad importa.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, enabled]);

  if (!enabled) return { caps: null, loading: false, error: null };
  // La respuesta anterior se conserva solo si es de la misma cara: al pasar a otro texto (otra pestaña, otra
  // selección) los ejes y funciones de la fuente anterior no se ofrecen para esta.
  const sameFace = state.face === face;
  return { caps: sameFace ? state.caps : null, loading: state.loading || state.key !== key, error: sameFace ? state.error : null };
}
