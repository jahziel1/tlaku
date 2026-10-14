import type { ArtBundle } from '@/api';
import type { UserArt } from '@/features/art/types';

/**
 * Sección `art` del paquete `.tlakubundle` (M4): la biblioteca de arte propia (los SVG que el
 * usuario agregó). Lógica pura: armar la sección, leerla con cuidado y calcular qué entraría. La
 * biblioteca incluida en la app no viaja (ya está en cada instalación). Los dibujos se mezclan: uno
 * con el mismo nombre y distinto contenido entra con otro nombre; nunca se pisa nada local.
 */

/** Tamaño máximo de un SVG de la biblioteca (el mismo que el motor). */
export const MAX_ART_BYTES = 2 * 1024 * 1024;
/** Tope de dibujos de un paquete. */
export const MAX_ART_ITEMS = 500;

export interface ArtPlan {
  /** Dibujos que entran (con su nombre original; si está ocupado, el motor les da otro). */
  items: UserArt[];
  /** De los `items`, los que chocan de nombre con otro distinto y entrarán con nombre nuevo. */
  renamed: number;
  /** Idénticos a uno que ya estaba. */
  skipped: number;
  /** No eran válidos (sin nombre, sin SVG o demasiado pesados) o sobraron del tope. */
  invalid: number;
}

const isObject = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);

export function buildArtSection(user: readonly UserArt[]): ArtBundle {
  return { items: user.map((a) => ({ name: a.name, svg: a.svg })) };
}

/** ¿Parece un SVG? (el motor lo vuelve a probar con el importador al guardar) */
const looksSvg = (text: string) => /<svg[\s>]/i.test(text);

/** Qué entraría de la sección sobre la biblioteca actual; `null` si no es una sección. */
export function planArt(raw: unknown, current: readonly UserArt[]): ArtPlan | null {
  if (!isObject(raw)) return null;
  const list = Array.isArray(raw.items) ? (raw.items as unknown[]) : [];
  const byName = new Map(current.map((a) => [a.name.toLowerCase(), a]));
  const seen = new Set<string>();
  const items: UserArt[] = [];
  let renamed = 0;
  let skipped = 0;
  let invalid = 0;
  for (const it of list) {
    if (items.length >= MAX_ART_ITEMS) {
      invalid++;
      continue;
    }
    if (!isObject(it) || typeof it.name !== 'string' || it.name.trim() === '' || typeof it.svg !== 'string' || !looksSvg(it.svg) || it.svg.length > MAX_ART_BYTES) {
      invalid++;
      continue;
    }
    const name = it.name.trim();
    const svg = it.svg;
    const existing = byName.get(name.toLowerCase());
    const dup = items.find((x) => x.name.toLowerCase() === name.toLowerCase() && x.svg === svg);
    if ((existing && existing.svg === svg) || dup) {
      skipped++;
      continue;
    }
    if (existing || seen.has(name.toLowerCase())) renamed++;
    seen.add(name.toLowerCase());
    items.push({ name, svg });
  }
  return { items, renamed, skipped, invalid };
}
