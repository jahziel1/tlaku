import { useEffect, useState } from 'react';
import { getApi } from '@/api';
import type { FontFamily } from '@/api/types';

/**
 * Familias que se prefieren como fuente por defecto, en orden: sans-serif comunes y legibles de
 * cada sistema. Si no hay ninguna instalada se usa la primera de la lista.
 */
export const DEFAULT_FONT_PREFERENCE = [
  'arial',
  'helvetica',
  'helvetica neue',
  'segoe ui',
  'roboto',
  'noto sans',
  'dejavu sans',
  'liberation sans',
  'open sans',
  'inter',
  'ubuntu',
  'cantarell',
] as const;

const norm = (s: string) => s.trim().toLowerCase();

/** Familia por nombre, sin distinguir mayúsculas. */
export function matchFamily(families: readonly FontFamily[], wanted: string): FontFamily | undefined {
  const w = norm(wanted);
  return w ? families.find((f) => norm(f.family) === w) : undefined;
}

/** Fuente por defecto de las instaladas (cadena vacía si no hay ninguna). */
export function pickDefaultFont(families: readonly FontFamily[]): string {
  for (const name of DEFAULT_FONT_PREFERENCE) {
    const hit = matchFamily(families, name);
    if (hit) return hit.family;
  }
  return families[0]?.family ?? '';
}

export interface EffectiveFont {
  /** Familia que se manda al motor. */
  family: string;
  /** Se pidió una familia que no está instalada y se sustituyó por la de por defecto. */
  fellBack: boolean;
}

/**
 * Familia con la que se genera el contorno: la pedida si está instalada; si no, la de por
 * defecto (y `fellBack` avisa, salvo que ni siquiera se hubiera elegido una). Sin lista de
 * fuentes no se puede juzgar y se respeta lo pedido.
 */
export function effectiveFont(families: readonly FontFamily[], wanted: string): EffectiveFont {
  if (families.length === 0) return { family: wanted, fellBack: false };
  const hit = matchFamily(families, wanted);
  if (hit) return { family: hit.family, fellBack: false };
  return { family: pickDefaultFont(families), fellBack: wanted.trim() !== '' };
}

/** Familias que contienen `query` (sin distinguir mayúsculas); las que empiezan por él van primero. */
export function filterFonts(families: readonly FontFamily[], query: string): FontFamily[] {
  const q = norm(query);
  if (!q) return [...families];
  const starts: FontFamily[] = [];
  const contains: FontFamily[] = [];
  for (const f of families) {
    const n = norm(f.family);
    if (n.startsWith(q)) starts.push(f);
    else if (n.includes(q)) contains.push(f);
  }
  return [...starts, ...contains];
}

let cache: Promise<FontFamily[]> | null = null;

/** Lista de fuentes del motor; se pide una sola vez (si falla, la siguiente llamada reintenta). */
export function loadFonts(): Promise<FontFamily[]> {
  if (!cache) {
    const pending = getApi().listFonts();
    cache = pending;
    pending.catch(() => {
      if (cache === pending) cache = null;
    });
  }
  return cache;
}

/** Para pruebas. */
export function resetFontsCache(): void {
  cache = null;
}

export interface FontsState {
  fonts: FontFamily[];
  loading: boolean;
  failed: boolean;
}

/** Fuentes disponibles para la interfaz (se cargan al primer uso). */
export function useFonts(): FontsState {
  const [state, setState] = useState<FontsState>({ fonts: [], loading: true, failed: false });
  useEffect(() => {
    let alive = true;
    loadFonts().then(
      (fonts) => alive && setState({ fonts, loading: false, failed: false }),
      () => alive && setState({ fonts: [], loading: false, failed: true }),
    );
    return () => {
      alive = false;
    };
  }, []);
  return state;
}
