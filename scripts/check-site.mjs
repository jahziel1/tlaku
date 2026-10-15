#!/usr/bin/env node
// Revisa el sitio ya construido (lo que publica GitHub Pages):
//
//   node scripts/build-site.mjs --out site-dist && node scripts/check-site.mjs site-dist
//
// Comprueba, para cada página de site/pages.json:
//   - enlaces y recursos internos (archivo existente y, si lleva #ancla, que el id exista), sin marcadores
//     {{…}} sin llenar y sin recursos externos (la CSP es `default-src 'none'`);
//   - título, descripción, un solo <h1>, <html lang>, alt en las imágenes y sin estilos ni scripts en línea
//     (la CSP los bloquearía; el JSON-LD no es un script ejecutable y se permite);
//   - canonical, hreflang (es, en, x-default, recíprocos), Open Graph y Twitter Card con URL absolutas bajo
//     la URL del sitio, y la imagen para redes (PNG de 1200 × 630);
//   - JSON-LD: parsea como JSON, SoftwareApplication con sus campos en la portada, FAQPage igual a las
//     preguntas visibles en la página de preguntas;
//   - sitemap.xml (todas las páginas, URLs absolutas, hreflang) y robots.txt.
// Sale con código 1 si hay algún error. Sin dependencias externas, solo Node.
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { OG_SIZE, decode, faqFromHtml, indexPages, pageDescription, pageTitle } from './site-seo.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const outDir = resolve(process.argv[2] ?? 'site-dist');
const registry = JSON.parse(readFileSync(join(root, 'site', 'pages.json'), 'utf8'));
const known = indexPages(registry);
const base = registry.site_url;

const errors = [];
const warnings = [];
const err = (where, msg) => errors.push(`${where}: ${msg}`);
const warn = (where, msg) => warnings.push(`${where}: ${msg}`);
const ok = (cond, where, msg) => {
  if (!cond) err(where, msg);
  return cond;
};

if (!existsSync(outDir) || !statSync(outDir).isDirectory()) {
  console.error(`check-site: no existe la carpeta ${outDir} (corre antes node scripts/build-site.mjs --out ${outDir})`);
  process.exit(1);
}

/** Archivo servido para una ruta del sitio (`en/faq/` -> en/faq/index.html). */
function fileFor(urlPath) {
  const p = urlPath.endsWith('/') || urlPath === '' ? `${urlPath}index.html` : urlPath;
  return join(outDir, p);
}

const pageHtml = new Map();
for (const [path] of known) {
  const file = fileFor(path);
  if (!ok(existsSync(file), path || '/', 'no existe su index.html en la carpeta construida')) continue;
  pageHtml.set(path, readFileSync(file, 'utf8'));
}

const idsOf = (html) => new Set([...html.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]));
let linkCount = 0;
let externalCount = 0;

for (const [path, html] of pageHtml) {
  const where = path || '/';
  const entry = known.get(path);
  const { page, lang } = entry;

  // --- marcadores y parciales sin resolver
  ok(!/\{\{[A-Z_]+\}\}/.test(html), where, 'quedó un marcador {{…}} sin llenar');
  ok(!html.includes('<!--@'), where, 'quedó un <!--@…--> sin resolver');

  // --- idioma, título, descripción, h1
  ok(new RegExp(`<html lang="${lang}"`).test(html), where, `<html lang> debe ser "${lang}"`);
  const title = pageTitle(html);
  const description = pageDescription(html);
  if (ok(title !== '', where, 'falta <title>')) {
    ok(title.length <= 75, where, `el título mide ${title.length} caracteres (máx. 75)`);
    if (title.length > 62) warn(where, `el título mide ${title.length} caracteres: Google puede cortarlo`);
  }
  if (ok(description !== '', where, 'falta la meta description')) {
    ok(description.length >= 70 && description.length <= 200, where, `la descripción mide ${description.length} caracteres (70 a 200)`);
    if (description.length > 175) warn(where, `la descripción mide ${description.length} caracteres: Google puede cortarla`);
  }
  ok((html.match(/<h1[\s>]/g) ?? []).length === 1, where, 'debe haber un solo <h1>');
  ok(/<meta http-equiv="Content-Security-Policy" content="default-src 'none'/.test(html), where, 'falta la CSP con default-src \'none\'');

  // --- CSP: sin estilos ni scripts en línea ni recursos externos
  ok(!/\sstyle="/.test(html), where, 'hay un atributo style="…" en línea (la CSP lo bloquea)');
  ok(!/<style[\s>]/.test(html), where, 'hay un <style> en línea (la CSP lo bloquea)');
  ok(!/\son[a-z]+="/.test(html), where, 'hay un manejador de eventos en línea (la CSP lo bloquea)');
  for (const m of html.matchAll(/<script\b([^>]*)>/g)) {
    const attrs = m[1];
    if (/type="application\/ld\+json"/.test(attrs)) continue;
    ok(/\ssrc="[^"]+"/.test(attrs) && !/src="https?:/.test(attrs), where, `<script${attrs}> debe ser un archivo propio (src relativo)`);
  }
  for (const m of html.matchAll(/<img\b[^>]*>/g)) {
    ok(/\salt="/.test(m[0]), where, `imagen sin alt: ${m[0].slice(0, 80)}`);
    ok(/\swidth="\d+"/.test(m[0]) && /\sheight="\d+"/.test(m[0]), where, `imagen sin width/height: ${m[0].slice(0, 80)}`);
  }

  // --- enlaces y recursos
  const dirUrl = path;
  for (const m of html.matchAll(/<(a|link|img|script)\b([^>]*)>/g)) {
    const attr = m[1] === 'a' || m[1] === 'link' ? 'href' : 'src';
    const v = m[2].match(new RegExp(`\\s${attr}="([^"]*)"`));
    if (!v) continue;
    const url = decode(v[1]);
    if (/^(https?:|mailto:)/.test(url)) {
      externalCount += 1;
      // Con la CSP solo pueden ser enlaces de navegación (<a>) o las URL de canonical/alternate.
      if (m[1] !== 'a' && !/rel="(canonical|alternate)"/.test(m[2])) err(where, `recurso externo no permitido por la CSP: ${url}`);
      continue;
    }
    if (url === '' && attr === 'href') {
      err(where, `enlace vacío: ${m[0].slice(0, 80)}`);
      continue;
    }
    linkCount += 1;
    const [pathPart, frag] = url.split('#');
    const targetUrl = pathPart === '' ? dirUrl : new URL(pathPart, `https://x/${dirUrl}`).pathname.slice(1);
    const targetFile = fileFor(targetUrl);
    if (!ok(existsSync(targetFile) && statSync(targetFile).isFile(), where, `enlace roto: ${url}`)) continue;
    if (frag && targetFile.endsWith('.html')) {
      const targetHtml = pathPart === '' ? html : readFileSync(targetFile, 'utf8');
      ok(idsOf(targetHtml).has(frag), where, `la ancla #${frag} no existe en ${targetUrl || '/'}`);
    }
  }

  // --- canonical, hreflang, Open Graph, Twitter
  const self = base + path;
  const meta = (re) => html.match(re)?.[1];
  ok(meta(/<link rel="canonical" href="([^"]+)"/) === self, where, `canonical debe ser ${self}`);
  for (const [hl, target] of [['es', page.es], ['en', page.en], ['x-default', page.es]]) {
    ok(meta(new RegExp(`<link rel="alternate" hreflang="${hl}" href="([^"]+)"`)) === base + target, where, `hreflang ${hl} debe apuntar a ${base + target}`);
    // reciprocidad: la página alterna declara la misma pareja
    const other = pageHtml.get(target);
    if (other && hl !== 'x-default') ok(other.includes(`hreflang="${lang}" href="${self}"`), where, `la alterna ${hl} no apunta de vuelta a esta página`);
  }
  const og = registry.og[lang];
  ok(meta(/<meta property="og:title" content="([^"]*)"/) === title.replace(/&/g, '&amp;'), where, 'og:title no coincide con <title>');
  ok(meta(/<meta property="og:url" content="([^"]+)"/) === self, where, 'og:url debe ser la URL de la página');
  ok(meta(/<meta property="og:image" content="([^"]+)"/) === base + og.image, where, `og:image debe ser ${base + og.image}`);
  ok(meta(/<meta property="og:image:width" content="([^"]+)"/) === String(OG_SIZE.width) && meta(/<meta property="og:image:height" content="([^"]+)"/) === String(OG_SIZE.height), where, 'og:image:width/height deben ser 1200 y 630');
  ok(Boolean(meta(/<meta property="og:description" content="([^"]+)"/)), where, 'falta og:description');
  ok(meta(/<meta name="twitter:card" content="([^"]+)"/) === 'summary_large_image', where, 'twitter:card debe ser summary_large_image');
  ok(meta(/<meta name="twitter:image" content="([^"]+)"/) === base + og.image, where, 'twitter:image debe ser la imagen para redes');
  ok(Boolean(meta(/<meta name="twitter:title" content="([^"]+)"/)) && Boolean(meta(/<meta name="twitter:description" content="([^"]+)"/)), where, 'faltan twitter:title o twitter:description');

  // --- JSON-LD
  const blocks = [...html.matchAll(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/g)].map((m) => m[1]);
  const docs = [];
  for (const raw of blocks) {
    try {
      docs.push(JSON.parse(raw));
    } catch (e) {
      err(where, `JSON-LD inválido: ${e.message}`);
    }
  }
  const byType = (t) => docs.find((d) => d['@type'] === t);
  for (const d of docs) ok(d['@context'] === 'https://schema.org', where, 'JSON-LD sin @context schema.org');
  if (page.kind === 'home') {
    const app = byType('SoftwareApplication');
    if (ok(Boolean(app), where, 'la portada debe tener JSON-LD SoftwareApplication')) {
      for (const f of ['name', 'operatingSystem', 'applicationCategory', 'license', 'softwareVersion', 'downloadUrl', 'url', 'description']) ok(typeof app[f] === 'string' && app[f] !== '', where, `SoftwareApplication sin ${f}`);
      ok(app.offers?.['@type'] === 'Offer' && app.offers.price === '0' && Boolean(app.offers.priceCurrency), where, 'SoftwareApplication.offers debe ser un Offer con price "0"');
      ok(/^\d+\.\d+\.\d+/.test(app.softwareVersion ?? ''), where, 'softwareVersion no parece una versión');
      ok(app.url === self, where, 'SoftwareApplication.url debe ser la URL de la página');
    }
  } else {
    ok(Boolean(byType('BreadcrumbList')), where, 'falta JSON-LD BreadcrumbList');
  }
  if (page.kind === 'faq') {
    const faq = byType('FAQPage');
    const visible = faqFromHtml(html);
    if (ok(Boolean(faq), where, 'la página de preguntas debe tener JSON-LD FAQPage')) {
      ok(Array.isArray(faq.mainEntity) && faq.mainEntity.length === visible.length && visible.length >= 5, where, `FAQPage debe tener las mismas preguntas que la página (${visible.length})`);
      for (const [i, q] of (faq.mainEntity ?? []).entries()) {
        ok(q['@type'] === 'Question' && q.name === visible[i]?.question && q.acceptedAnswer?.['@type'] === 'Answer' && q.acceptedAnswer.text === visible[i]?.answer, where, `la pregunta ${i + 1} del JSON-LD no coincide con la visible`);
      }
    }
  }
}

// --- imagen para redes
for (const lang of ['es', 'en']) {
  const file = join(outDir, registry.og[lang].image);
  if (!ok(existsSync(file), registry.og[lang].image, 'no existe en la carpeta construida')) continue;
  const buf = readFileSync(file);
  const isPng = buf.subarray(0, 8).toString('hex') === '89504e470d0a1a0a';
  if (ok(isPng, registry.og[lang].image, 'no es un PNG')) {
    ok(buf.readUInt32BE(16) === OG_SIZE.width && buf.readUInt32BE(20) === OG_SIZE.height, registry.og[lang].image, `debe medir ${OG_SIZE.width}×${OG_SIZE.height}, mide ${buf.readUInt32BE(16)}×${buf.readUInt32BE(20)}`);
    ok(buf.length < 600 * 1024, registry.og[lang].image, 'pesa más de 600 KB');
  }
}

// --- sitemap y robots
const sitemapFile = join(outDir, 'sitemap.xml');
let sitemapCount = 0;
if (ok(existsSync(sitemapFile), 'sitemap.xml', 'no existe')) {
  const xml = readFileSync(sitemapFile, 'utf8');
  const locs = [...xml.matchAll(/<loc>([^<]+)<\/loc>/g)].map((m) => m[1]);
  sitemapCount = locs.length;
  ok(new Set(locs).size === locs.length, 'sitemap.xml', 'hay URLs repetidas');
  for (const loc of locs) {
    ok(loc.startsWith(base) && /^https:\/\/[^\s<>"]+$/.test(loc), 'sitemap.xml', `URL no absoluta bajo ${base}: ${loc}`);
    ok(known.has(loc.slice(base.length)), 'sitemap.xml', `la URL ${loc} no es una página registrada`);
  }
  for (const [path] of known) ok(locs.includes(base + path), 'sitemap.xml', `falta la página ${base + path}`);
  ok(xml.includes('xmlns:xhtml="http://www.w3.org/1999/xhtml"') && xml.includes('hreflang="x-default"'), 'sitemap.xml', 'faltan los xhtml:link con hreflang');
}
const robotsFile = join(outDir, 'robots.txt');
if (ok(existsSync(robotsFile), 'robots.txt', 'no existe')) {
  const robots = readFileSync(robotsFile, 'utf8');
  ok(robots.includes(`Sitemap: ${base}sitemap.xml`), 'robots.txt', 'falta la línea Sitemap con la URL absoluta');
  ok(!/^Disallow:\s*\/\s*$/m.test(robots), 'robots.txt', 'bloquea todo el sitio');
}

// --- ningún .html fuera del registro
function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(dir, e.name)) : [join(dir, e.name)]));
}
for (const f of walk(outDir)) {
  if (!f.endsWith('.html')) continue;
  const rel = f.slice(outDir.length + 1);
  if (!known.has(rel === 'index.html' ? '' : rel.replace(/index\.html$/, ''))) err(rel, 'página que no está en site/pages.json');
}

for (const w of warnings) console.log(`aviso: ${w}`);
if (errors.length > 0) {
  for (const e of errors) console.error(`ERROR ${e}`);
  console.error(`\ncheck-site: ${errors.length} error(es).`);
  process.exit(1);
}
console.log(`check-site: ${pageHtml.size} páginas, ${linkCount} enlaces internos, ${externalCount} externos (no se visitan), ${sitemapCount} URL en el sitemap. Todo bien.`);
