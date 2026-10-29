import type { ArtBundle, ComponentsBundle } from '@/api';
import { validComponents } from '@/features/art/components';
import type { UserArt, UserComponent } from '@/features/art/types';

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
  /** Componentes guardados (issue #22) que entran; igual que los dibujos, nunca pisan los locales. */
  components: UserComponent[];
  /** De los `components`, los que entrarán con otro nombre por chocar con uno distinto. */
  componentsRenamed: number;
  /** Componentes idénticos a uno que ya estaba. */
  componentsSkipped: number;
  /** Componentes que no eran válidos. */
  componentsInvalid: number;
}

const isObject = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);

export function buildArtSection(user: readonly UserArt[]): ArtBundle {
  return { items: user.map((a) => ({ name: a.name, svg: a.svg })) };
}

/** Sección `components` del paquete: los componentes guardados (viajan con la casilla de la biblioteca de arte). */
export function buildComponentsSection(components: readonly UserComponent[]): ComponentsBundle {
  return { items: components.map((c) => ({ name: c.name, data: structuredClone(c.data) })) };
}

/** Qué componentes entrarían: los válidos que no están ya (mismo nombre y mismo contenido). */
export function planComponents(raw: unknown, current: readonly UserComponent[]): Pick<ArtPlan, 'components' | 'componentsRenamed' | 'componentsSkipped' | 'componentsInvalid'> {
  const { items, invalid } = validComponents(isObject(raw) ? raw.items : []);
  const byName = new Map(current.map((c) => [c.name.toLowerCase(), c]));
  const seen = new Set<string>();
  const components: UserComponent[] = [];
  let renamed = 0;
  let skipped = 0;
  for (const c of items) {
    const key = c.name.toLowerCase();
    const existing = byName.get(key);
    const same = (x: UserComponent) => JSON.stringify(x.data) === JSON.stringify(c.data);
    if ((existing && same(existing)) || components.some((x) => x.name.toLowerCase() === key && same(x))) {
      skipped++;
      continue;
    }
    if (existing || seen.has(key)) renamed++;
    seen.add(key);
    components.push(c);
  }
  return { components, componentsRenamed: renamed, componentsSkipped: skipped, componentsInvalid: invalid };
}

/** ¿Parece un SVG? (el motor lo vuelve a probar con el importador al guardar) */
const looksSvg = (text: string) => /<svg[\s>]/i.test(text);

/** Qué entraría de la sección sobre la biblioteca actual; `null` si no es una sección. */
export function planArt(raw: unknown, current: readonly UserArt[], rawComponents?: unknown, currentComponents: readonly UserComponent[] = []): ArtPlan | null {
  if (!isObject(raw) && !isObject(rawComponents)) return null;
  const comp = planComponents(rawComponents, currentComponents);
  const list = isObject(raw) && Array.isArray(raw.items) ? (raw.items as unknown[]) : [];
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
  return { items, renamed, skipped, invalid, ...comp };
}
