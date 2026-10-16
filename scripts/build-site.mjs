#!/usr/bin/env node
// Construye el sitio estático de `site/` y reemplaza los marcadores con los valores de
// `site/config.json`.
//
//   node scripts/build-site.mjs [--config site/config.json] [--out site-dist]
//   node scripts/build-site.mjs --readme            # también llena la sección de apoyo de README.md y README.es.md
//
// Marcadores de la configuración: {{REPO_URL}}, {{SPONSORS_URL}}, {{KOFI_URL}}.
// Marcadores calculados por página: {{ROOT}} (ruta relativa a la raíz del sitio), {{HOME}} (a la portada
// del idioma de la página), {{ALT_URL}} (a la misma página en el otro idioma) y {{VERSION}} (la de
// apps/desktop/src-tauri/tauri.conf.json).
//
// Sitio: copia `site/` a la carpeta de salida (sin config.json, pages.json ni _partials), copia a `img/`
// las capturas de `apps/desktop/docs/screenshots/` que las páginas usan y reemplaza los marcadores en los
// .html. Un valor vacío quita el enlace entero (p. ej. el botón de GitHub Sponsors mientras no exista) y,
// si un grupo `.optgroup` se queda sin enlaces, el CSS muestra su aviso de "aún no hay...".
//
// Parciales: `<!--@include head-->`, `sprite`, `header` y `footer` se sustituyen por
// `site/_partials/<nombre>.html` o, si existe, `site/_partials/<idioma>/<nombre>.html` (el idioma sale de
// `<html lang>`). Así el menú y el pie son los mismos en todas las páginas.
//
// SEO (`site/pages.json`): `<!--@seo-->` en el <head> se sustituye por canonical, hreflang (es, en y
// x-default), Open Graph, Twitter Card y JSON-LD (SoftwareApplication en la portada, FAQPage en las
// preguntas frecuentes, BreadcrumbList en el resto). Además genera `sitemap.xml` y `robots.txt` con todas
// las páginas registradas. Revisa el resultado con `node scripts/check-site.mjs <carpeta>`.
//
// README (`--readme`): regenera el bloque entre `<!-- support:start -->` y `<!-- support:end -->` de
// README.md (inglés) y README.es.md (español) con los enlaces de donación que tengan valor.
// Es idempotente: se puede correr de nuevo cuando cambie la configuración.
//
// No usa dependencias externas, solo Node.
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { GOOGLE_VERIFY, indexPages, isGoogleVerification, relUrl, robotsTxt, seoBlock, sitemapXml } from './site-seo.mjs';

export const KEYS = ['REPO_URL', 'SPONSORS_URL', 'KOFI_URL'];
/** Marcadores que calcula el script para cada página (no vienen de site/config.json). */
export const DERIVED = ['ROOT', 'HOME', 'ALT_URL', 'VERSION'];

/** Lee y valida la configuración: solo las claves conocidas, vacías o con una URL http(s) sin espacios ni comillas. */
export function parseConfig(text) {
  const raw = JSON.parse(text);
  const out = {};
  for (const key of KEYS) {
    const value = typeof raw[key] === 'string' ? raw[key].trim() : '';
    if (value !== '' && !/^https?:\/\/[^\s"'<>{}]+$/.test(value)) {
      throw new Error(`${key}: debe estar vacío o ser una URL http(s) sin espacios ni comillas, recibí: ${value}`);
    }
    out[key] = value.replace(/\/+$/, '');
  }
  for (const key of Object.keys(raw)) {
    if (!key.startsWith('_') && !KEYS.includes(key)) throw new Error(`clave desconocida en la configuración: ${key}`);
  }
  return out;
}

const MARKER = /\{\{([A-Z_]+)\}\}/g;

function assertKnown(text, where) {
  for (const [, name] of text.matchAll(MARKER)) {
    if (!KEYS.includes(name) && !DERIVED.includes(name)) throw new Error(`${where}: marcador desconocido {{${name}}}`);
  }
}

/**
 * HTML: reemplaza los marcadores con valor. Un enlace `<a href="{{X}}">…</a>` cuyo valor está vacío
 * se quita (no queda un botón roto ni un `{{X}}` visible); el CSS (`.optgroup`) muestra entonces el
 * aviso de "aún no hay..." si el grupo se queda sin enlaces.
 */
export function fillHtml(html, config, where = 'html') {
  assertKnown(html, where);
  const sinVacios = html.replace(/<a\b[^>]*\bhref="\{\{([A-Z_]+)\}\}"[^>]*>[\s\S]*?<\/a>/g, (all, name) => (config[name] ? all : ''));
  return sinVacios.replace(MARKER, (all, name) => (config[name] ? config[name] : all));
}

/** Markdown: un marcador vacío quita su línea entera; con valor se reemplaza. */
export function fillMarkdown(text, config, where = 'markdown') {
  assertKnown(text, where);
  return text
    .split('\n')
    .filter((line) => ![...line.matchAll(MARKER)].some(([, name]) => !config[name]))
    .join('\n')
    .replace(MARKER, (all, name) => config[name] || all);
}

const SUPPORT_START = '<!-- support:start -->';
const SUPPORT_END = '<!-- support:end -->';

const SUPPORT_TEXT = {
  en: { none: 'There are no donation accounts yet. When they exist, their links will appear here and in Help > Donate to the project… inside the app.' },
  es: { none: 'Todavía no hay cuentas de donación. Cuando existan, sus enlaces aparecerán aquí y en Ayuda > Donar al proyecto… dentro de la app.' },
};

/** Bloque de apoyo de un README (`lang`: 'en' o 'es'): un renglón por enlace configurado, o el aviso de que aún no hay. */
export function supportBlock(config, lang) {
  const lines = [];
  if (config.SPONSORS_URL) lines.push(`- GitHub Sponsors: ${config.SPONSORS_URL}`);
  if (config.KOFI_URL) lines.push(`- Ko-fi: ${config.KOFI_URL}`);
  return lines.length ? lines.join('\n') : SUPPORT_TEXT[lang].none;
}

/** Reemplaza lo que hay entre los marcadores de apoyo; sin marcadores lanza un error. */
export function fillSupport(text, config, lang, where = 'markdown') {
  const a = text.indexOf(SUPPORT_START);
  const b = text.indexOf(SUPPORT_END);
  if (a < 0 || b < a) throw new Error(`${where}: faltan los marcadores ${SUPPORT_START} y ${SUPPORT_END}`);
  return `${text.slice(0, a + SUPPORT_START.length)}\n${supportBlock(config, lang)}\n${text.slice(b)}`;
}

function htmlFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = join(dir, e.name);
    if (e.isDirectory()) return htmlFiles(p);
    return e.name.endsWith('.html') ? [p] : [];
  });
}

const EXCLUDED = new Set(['config.json', 'pages.json', '_partials']);

/** Sustituye `<!--@include nombre-->` por el parcial del idioma (o el común). */
export function includePartials(html, siteDir, lang, where = 'html') {
  return html.replace(/<!--@include ([a-z-]+)-->/g, (all, name) => {
    for (const p of [join(siteDir, '_partials', lang, `${name}.html`), join(siteDir, '_partials', `${name}.html`)]) {
      if (existsSync(p)) return readFileSync(p, 'utf8').replace(/\n$/, '');
    }
    throw new Error(`${where}: no existe el parcial ${name}`);
  });
}

export function build({ root, configPath, outDir }) {
  const siteDir = join(root, 'site');
  const shots = join(root, 'apps', 'desktop', 'docs', 'screenshots');
  const config = parseConfig(readFileSync(configPath, 'utf8'));
  const registry = JSON.parse(readFileSync(join(siteDir, 'pages.json'), 'utf8'));
  const known = indexPages(registry);
  const version = JSON.parse(readFileSync(join(root, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json'), 'utf8')).version;
  if (!/^\d+\.\d+\.\d+/.test(version)) throw new Error(`versión inválida en tauri.conf.json: ${version}`);

  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });
  cpSync(siteDir, outDir, { recursive: true, filter: (src) => !EXCLUDED.has(basename(src)) });

  // Cada .html del sitio debe estar en pages.json y viceversa (salvo la verificación de Google).
  const files = htmlFiles(outDir).filter((f) => {
    if (dirname(f) !== outDir || !GOOGLE_VERIFY.test(basename(f))) return true;
    if (!isGoogleVerification(basename(f), readFileSync(f, 'utf8'))) throw new Error(`${relative(root, f)}: no tiene el contenido de verificación de Google`);
    return false;
  });
  const seen = new Set();
  for (const file of files) {
    const path = relative(outDir, dirname(file)).split('\\').join('/');
    const key = path === '' ? '' : `${path}/`;
    if (basename(file) !== 'index.html' || !known.has(key)) throw new Error(`${relative(root, file)}: la página no está en site/pages.json (rutas como «falcon2/» con su index.html)`);
    seen.add(key);
  }
  for (const key of known.keys()) if (!seen.has(key)) throw new Error(`site/pages.json registra «${key}» pero no existe su index.html`);
  for (const lang of ['es', 'en']) {
    if (!existsSync(join(siteDir, registry.og[lang].image))) throw new Error(`falta la imagen para redes site/${registry.og[lang].image} (scripts/site-og.py)`);
  }

  // Páginas: parciales, marcadores, SEO y las capturas que piden con img/<archivo>.png.
  const used = new Set();
  for (const file of files) {
    const where = relative(root, file);
    const path = relative(outDir, dirname(file)).split('\\').join('/');
    const key = path === '' ? '' : `${path}/`;
    const entry = known.get(key);
    const raw = readFileSync(file, 'utf8');
    const lang = raw.match(/<html lang="([a-z]{2})"/)?.[1];
    if (lang !== entry.lang) throw new Error(`${where}: <html lang="${lang}"> no coincide con pages.json (${entry.lang})`);
    const values = {
      ...config,
      ROOT: relUrl(key, ''),
      HOME: relUrl(key, lang === 'en' ? 'en/' : ''),
      ALT_URL: relUrl(key, entry.page[lang === 'en' ? 'es' : 'en']),
      VERSION: version,
    };
    let html = fillHtml(includePartials(raw, siteDir, lang, where), values, where);
    if (!html.includes('<!--@seo-->')) throw new Error(`${where}: falta <!--@seo--> en el <head>`);
    html = html.replace('<!--@seo-->', () => seoBlock({ html, path: key, entry, registry, version, repoUrl: config.REPO_URL }));
    for (const [, name] of html.matchAll(/(?:\.\.\/)?img\/([A-Za-z0-9._-]+\.png)/g)) used.add(name);
    writeFileSync(file, html);
  }
  mkdirSync(join(outDir, 'img'), { recursive: true });
  for (const name of used) {
    const from = join(shots, name);
    if (!existsSync(from)) throw new Error(`falta la captura ${name} en apps/desktop/docs/screenshots/`);
    cpSync(from, join(outDir, 'img', name));
  }
  writeFileSync(join(outDir, 'sitemap.xml'), sitemapXml(registry));
  writeFileSync(join(outDir, 'robots.txt'), robotsTxt(registry));
  return { config, version, images: [...used].sort(), files: files.length };
}

function main() {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
  const args = process.argv.slice(2);
  const opt = (name, fallback) => {
    const i = args.indexOf(name);
    return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
  };
  const configPath = resolve(root, opt('--config', 'site/config.json'));
  const outDir = resolve(root, opt('--out', 'site-dist'));
  if (!existsSync(configPath)) throw new Error(`no existe ${configPath}`);
  if (outDir === root || outDir === join(root, 'site')) throw new Error('la carpeta de salida no puede ser la raíz ni site/');

  const { config, images, files } = build({ root, configPath, outDir });
  console.log(`Sitio construido en ${relative(root, outDir) || '.'}: ${files} páginas, ${images.length} capturas, sitemap.xml y robots.txt.`);
  for (const key of KEYS) console.log(`  ${key}: ${config[key] || '(vacío: se oculta)'}`);

  if (args.includes('--readme')) {
    for (const [name, lang] of [['README.md', 'en'], ['README.es.md', 'es']]) {
      const readme = join(root, name);
      const before = readFileSync(readme, 'utf8');
      const after = fillSupport(before, config, lang, name);
      if (after !== before) {
        writeFileSync(readme, after);
        console.log(`${name} actualizado.`);
      }
    }
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (e) {
    console.error(`build-site: ${e instanceof Error ? e.message : e}`);
    process.exit(1);
  }
}
