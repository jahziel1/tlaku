import { getLanguage, type Language } from '@/i18n';
import { documentTitle, normalizeSearch, parseMarkdown, searchText, type Block } from './markdown';

/**
 * Páginas del manual. Los .md viven en docs/manual/ (se leen en GitHub igual que aquí) y entran en
 * el paquete de la app como texto con `?raw`. La traducción al inglés vive en docs/manual/en/ con los
 * mismos nombres de archivo; si falta un capítulo en inglés, se muestra el español.
 * `THIRD_PARTY.md` alimenta los créditos.
 */
const RAW_PAGES = import.meta.glob('../../../../../docs/manual/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

const RAW_PAGES_EN = import.meta.glob('../../../../../docs/manual/en/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

const RAW_THIRD_PARTY = import.meta.glob('../../../../../THIRD_PARTY.md', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

/** Capturas de apps/desktop/docs/screenshots/, por nombre de archivo. */
const SCREENSHOTS = import.meta.glob('../../../docs/screenshots/*.png', {
  query: '?url',
  import: 'default',
  eager: true,
}) as Record<string, string>;

export interface ManualPage {
  /** Nombre del archivo, por ejemplo `03-dibujar-y-editar.md`. */
  file: string;
  title: string;
  blocks: Block[];
  /** Texto en minúsculas y sin acentos, para buscar. */
  haystack: string;
}

const baseName = (path: string) => path.slice(path.lastIndexOf('/') + 1);

export function buildPages(raw: Record<string, string>): ManualPage[] {
  return Object.entries(raw)
    .map(([path, text]) => {
      const file = baseName(path);
      const blocks = parseMarkdown(text);
      return { file, title: documentTitle(blocks, file.replace(/\.md$/, '')), blocks, haystack: searchText(blocks) };
    })
    .sort((a, b) => a.file.localeCompare(b.file));
}

/**
 * Texto de cada capítulo en un idioma: en inglés, el capítulo traducido si existe y, si no, el español.
 * Las claves son rutas; solo importa el nombre del archivo, que es el mismo en los dos idiomas.
 */
export function rawPagesFor(lang: Language, es: Record<string, string>, en: Record<string, string>): Record<string, string> {
  if (lang !== 'en') return es;
  const translated = new Set(Object.keys(en).map(baseName));
  const missing = Object.fromEntries(Object.entries(es).filter(([path]) => !translated.has(baseName(path))));
  return { ...missing, ...en };
}

/** Nombres de los capítulos que ya tienen traducción al inglés. */
export function englishFiles(): string[] {
  return Object.keys(RAW_PAGES_EN).map(baseName).sort();
}

const cache: Partial<Record<Language, ManualPage[]>> = {};

/** Todas las páginas en el idioma de la interfaz, ordenadas por nombre de archivo (el prefijo numérico fija el orden). */
export function manualPages(): ManualPage[] {
  const lang = getLanguage();
  return (cache[lang] ??= buildPages(rawPagesFor(lang, RAW_PAGES, RAW_PAGES_EN)));
}

/** Páginas del manual que van en el índice lateral: sin "novedades", que tiene su propio menú. */
export const NEWS_FILE = 'novedades.md';
export const INDEX_FILE = '00-indice.md';

export function indexPages(): ManualPage[] {
  return manualPages().filter((p) => p.file !== NEWS_FILE);
}

export function findPage(file: string): ManualPage | undefined {
  return manualPages().find((p) => p.file === file);
}

export function searchPages(query: string): ManualPage[] {
  const q = normalizeSearch(query.trim());
  if (!q) return indexPages();
  const words = q.split(/\s+/);
  return indexPages().filter((p) => words.every((w) => p.haystack.includes(w) || normalizeSearch(p.title).includes(w)));
}

let thirdParty: Block[] | null = null;

/** Créditos: THIRD_PARTY.md del repositorio, interpretado con el mismo visor. */
export function thirdPartyBlocks(): Block[] {
  thirdParty ??= parseMarkdown(Object.values(RAW_THIRD_PARTY)[0] ?? '');
  return thirdParty;
}

/** URL empaquetada de una captura a partir de la ruta que usa el .md, o `null` si no existe. */
export function resolveImage(src: string): string | null {
  const name = baseName(src);
  const key = Object.keys(SCREENSHOTS).find((k) => baseName(k) === name);
  return key ? SCREENSHOTS[key] : null;
}
