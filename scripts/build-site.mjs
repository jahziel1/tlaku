#!/usr/bin/env node
// Construye el sitio estático de `site/` y reemplaza los marcadores con los valores de
// `site/config.json`.
//
//   node scripts/build-site.mjs [--config site/config.json] [--out site-dist]
//   node scripts/build-site.mjs --readme            # también llena los marcadores de README.md
//
// Marcadores: {{REPO_URL}}, {{SPONSORS_URL}}, {{KOFI_URL}}.
//
// Sitio: copia `site/` a la carpeta de salida (sin config.json), copia a `img/` las capturas de
// `apps/desktop/docs/screenshots/` que las páginas usan y reemplaza los marcadores en los .html.
// Un valor vacío NO se reemplaza: el marcador queda y el CSS (`a.opt[href^="{{"]`) oculta el botón
// y muestra el aviso de "aún no hay...". Con un valor, el botón aparece.
//
// README (`--readme`, en el mismo archivo): un marcador vacío borra su línea; con valor, lo reemplaza.
//
// No usa dependencias externas, solo Node.
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const KEYS = ['REPO_URL', 'SPONSORS_URL', 'KOFI_URL'];

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
    if (!KEYS.includes(name)) throw new Error(`${where}: marcador desconocido {{${name}}}`);
  }
}

/** HTML: reemplaza los marcadores con valor y deja los vacíos (el CSS los oculta). */
export function fillHtml(html, config, where = 'html') {
  assertKnown(html, where);
  return html.replace(MARKER, (all, name) => (config[name] ? config[name] : all));
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

function htmlFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = join(dir, e.name);
    if (e.isDirectory()) return htmlFiles(p);
    return e.name.endsWith('.html') ? [p] : [];
  });
}

export function build({ root, configPath, outDir }) {
  const siteDir = join(root, 'site');
  const shots = join(root, 'apps', 'desktop', 'docs', 'screenshots');
  const config = parseConfig(readFileSync(configPath, 'utf8'));

  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });
  cpSync(siteDir, outDir, { recursive: true, filter: (src) => basename(src) !== 'config.json' });

  // Capturas: las que las páginas piden con img/<archivo>.png.
  const used = new Set();
  for (const file of htmlFiles(outDir)) {
    const html = readFileSync(file, 'utf8');
    for (const [, name] of html.matchAll(/(?:\.\.\/)?img\/([A-Za-z0-9._-]+\.png)/g)) used.add(name);
    writeFileSync(file, fillHtml(html, config, relative(root, file)));
  }
  mkdirSync(join(outDir, 'img'), { recursive: true });
  for (const name of used) {
    const from = join(shots, name);
    if (!existsSync(from)) throw new Error(`falta la captura ${name} en apps/desktop/docs/screenshots/`);
    cpSync(from, join(outDir, 'img', name));
  }
  return { config, images: [...used].sort(), files: htmlFiles(outDir).length };
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
  console.log(`Sitio construido en ${relative(root, outDir) || '.'}: ${files} páginas, ${images.length} capturas.`);
  for (const key of KEYS) console.log(`  ${key}: ${config[key] || '(vacío: se oculta)'}`);

  if (args.includes('--readme')) {
    const readme = join(root, 'README.md');
    const before = readFileSync(readme, 'utf8');
    const after = fillMarkdown(before, config, 'README.md');
    if (after !== before) {
      writeFileSync(readme, after);
      console.log('README.md actualizado.');
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
