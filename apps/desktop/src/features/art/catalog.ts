import { getLanguage } from '@/i18n';
import catalog from './svg/catalog.json';

/** Categorías de la biblioteca incluida, en el orden en que se muestran. */
export const ART_CATEGORIES = ['flechas', 'marcos', 'estrellas', 'corazones', 'iconos', 'bordes', 'cajas'] as const;
export type ArtCategory = (typeof ART_CATEGORIES)[number];

export interface ArtItem {
  /** Identificador único: el nombre del archivo (incluidos) o `usuario:<nombre>` (del usuario). */
  id: string;
  category: ArtCategory | 'usuario';
  /** Nombre en cada idioma; los del usuario usan el nombre del archivo en ambos. */
  es: string;
  en: string;
  svg: string;
}

// Los SVG incluidos son diseños propios de Tlaku (licencia MIT): ver `svg/` y THIRD_PARTY.md.
const raw = import.meta.glob('./svg/*.svg', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

interface CatalogEntry {
  id: string;
  category: string;
  es: string;
  en: string;
}

/** La biblioteca incluida. Un SVG sin entrada en el catálogo (o al revés) no se muestra. */
export const BUILTIN_ART: ArtItem[] = (catalog as CatalogEntry[]).flatMap((e): ArtItem[] => {
  const svg = raw[`./svg/${e.id}.svg`];
  const category = ART_CATEGORIES.find((c) => c === e.category);
  return svg && category ? [{ id: e.id, category, es: e.es, en: e.en, svg }] : [];
});

/** Elemento de la biblioteca del usuario. */
export function userItem(name: string, svg: string): ArtItem {
  return { id: `usuario:${name}`, category: 'usuario', es: name, en: name, svg };
}

export function artName(item: ArtItem): string {
  return getLanguage() === 'en' ? item.en : item.es;
}

/** Filtra por categoría y por texto (sin distinguir mayúsculas ni acentos, en ambos idiomas). */
export function filterArt(items: readonly ArtItem[], category: ArtCategory | 'usuario' | 'todo', query: string): ArtItem[] {
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const q = norm(query.trim());
  return items.filter((i) => (category === 'todo' || i.category === category) && (q === '' || norm(i.es).includes(q) || norm(i.en).includes(q)));
}

/** URL de datos para mostrar un SVG en una miniatura. */
export function svgDataUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/**
 * El SVG con el trazo engrosado para verse bien en una miniatura. Los dibujos usan trazos de
 * 0,2 mm (para el láser): reducidos a 72 px quedan en una fracción de píxel y casi no se ven.
 * Solo cambia la vista; lo que se inserta en el diseño es el SVG original.
 */
export function thumbnailSvg(svg: string): string {
  const box = /viewBox="\s*[-\d.]+[\s,]+[-\d.]+[\s,]+([\d.]+)[\s,]+([\d.]+)\s*"/.exec(svg);
  if (!box) return svg;
  const stroke = Math.max(Number(box[1]), Number(box[2])) / 40;
  return stroke > 0 ? svg.replace(/stroke-width="[^"]*"/g, `stroke-width="${stroke.toFixed(2)}"`) : svg;
}
