// SEO del sitio de Tlaku: metadatos por página, JSON-LD, sitemap.xml y robots.txt.
// Lo usan scripts/build-site.mjs (al construir) y scripts/check-site.mjs (al verificar).
// Los datos de las páginas están en site/pages.json; el título y la descripción salen del propio HTML.
// Sin dependencias externas, solo Node.
import { posix } from 'node:path';

export const OG_SIZE = { width: 1200, height: 630 };
const LOCALES = { es: 'es_MX', en: 'en_US' };
const OTHER = { es: 'en', en: 'es' };

/** Ruta relativa (con `/` final si `to` lo trae) entre dos rutas del sitio, p. ej. `en/faq/` -> `` = `../../`. */
export function relUrl(fromPage, to) {
  const r = posix.relative(fromPage.replace(/\/$/, ''), to.replace(/\/$/, ''));
  if (r === '') return './';
  return r + (to === '' || to.endsWith('/') ? '/' : '');
}

/** Valida `site/pages.json` y devuelve una tabla ruta -> {page, lang}. */
export function indexPages(registry) {
  const out = new Map();
  if (!/^https:\/\/[^\s"'<>]+\/$/.test(registry.site_url ?? '')) throw new Error('pages.json: site_url debe ser una URL https que termina en /');
  for (const lang of ['es', 'en']) {
    if (!registry.og?.[lang]?.image || !registry.og?.[lang]?.alt) throw new Error(`pages.json: falta og.${lang}.image u og.${lang}.alt`);
  }
  for (const page of registry.pages) {
    for (const lang of ['es', 'en']) {
      const path = page[lang];
      if (typeof path !== 'string' || (path !== '' && !/^[a-z0-9-]+(\/[a-z0-9-]+)*\/$/.test(path))) {
        throw new Error(`pages.json: ruta inválida en ${page.id}.${lang}: ${JSON.stringify(path)}`);
      }
      if (out.has(path)) throw new Error(`pages.json: la ruta ${JSON.stringify(path)} está repetida`);
      out.set(path, { page, lang });
    }
  }
  return out;
}

const ENTITIES = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: '\u00a0' };

/** Decodifica las entidades de HTML que usan las páginas. */
export function decode(text) {
  return text.replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi, (all, e) => {
    if (e[0] === '#') return String.fromCodePoint(e[1].toLowerCase() === 'x' ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10));
    return ENTITIES[e.toLowerCase()] ?? all;
  });
}

export function escAttr(text) {
  return text.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

const stripTags = (html) => decode(html.replace(/<[^>]*>/g, ' ')).replace(/\s+/g, ' ').trim();

export function pageTitle(html) {
  const m = html.match(/<title>([\s\S]*?)<\/title>/);
  return m ? decode(m[1]).trim() : '';
}

export function pageDescription(html) {
  const m = html.match(/<meta name="description" content="([^"]*)"/);
  return m ? decode(m[1]).trim() : '';
}

/** Preguntas y respuestas de la página de preguntas frecuentes: `<div class="qa"><h3>…</h3><div class="a">…</div></div>`. */
export function faqFromHtml(html) {
  const items = [];
  for (const m of html.matchAll(/<div class="qa">\s*<h3[^>]*>([\s\S]*?)<\/h3>\s*<div class="a">([\s\S]*?)<\/div>\s*<\/div>/g)) {
    items.push({ question: stripTags(m[1]), answer: stripTags(m[2]) });
  }
  return items;
}

/** Texto seguro para meter dentro de <script type="application/ld+json">. */
export function ldJson(data) {
  return JSON.stringify(data, null, 2).replace(/</g, '\\u003c').replace(/>/g, '\\u003e').replace(/&/g, '\\u0026');
}

/**
 * Metadatos de una página ya con sus marcadores llenos: canonical, hreflang, Open Graph, Twitter Card y JSON-LD.
 * `entry` = {page, lang} de `indexPages`; `path` = ruta de la página (`en/faq/`).
 */
export function seoBlock({ html, path, entry, registry, version, repoUrl }) {
  const { page, lang } = entry;
  const base = registry.site_url;
  const abs = (p) => base + p;
  const title = pageTitle(html);
  const description = pageDescription(html);
  if (!title) throw new Error(`${path || '/'}: falta <title>`);
  if (!description) throw new Error(`${path || '/'}: falta <meta name="description">`);

  const og = registry.og[lang];
  const ogImage = abs(og.image);
  const other = OTHER[lang];
  const t = escAttr(title);
  const d = escAttr(description);
  const lines = [
    `<link rel="canonical" href="${abs(path)}">`,
    `<link rel="alternate" hreflang="es" href="${abs(page.es)}">`,
    `<link rel="alternate" hreflang="en" href="${abs(page.en)}">`,
    `<link rel="alternate" hreflang="x-default" href="${abs(page.es)}">`,
    `<meta name="robots" content="index, follow, max-image-preview:large">`,
    `<meta property="og:site_name" content="Tlaku">`,
    `<meta property="og:type" content="website">`,
    `<meta property="og:locale" content="${LOCALES[lang]}">`,
    `<meta property="og:locale:alternate" content="${LOCALES[other]}">`,
    `<meta property="og:title" content="${t}">`,
    `<meta property="og:description" content="${d}">`,
    `<meta property="og:url" content="${abs(path)}">`,
    `<meta property="og:image" content="${ogImage}">`,
    `<meta property="og:image:type" content="image/png">`,
    `<meta property="og:image:width" content="${OG_SIZE.width}">`,
    `<meta property="og:image:height" content="${OG_SIZE.height}">`,
    `<meta property="og:image:alt" content="${escAttr(og.alt)}">`,
    `<meta name="twitter:card" content="summary_large_image">`,
    `<meta name="twitter:title" content="${t}">`,
    `<meta name="twitter:description" content="${d}">`,
    `<meta name="twitter:image" content="${ogImage}">`,
    `<meta name="twitter:image:alt" content="${escAttr(og.alt)}">`,
  ];

  const graphs = [];
  if (page.kind === 'home') {
    const app = {
      '@context': 'https://schema.org',
      '@type': 'SoftwareApplication',
      name: 'Tlaku',
      description,
      url: abs(path),
      applicationCategory: 'DesignApplication',
      operatingSystem: 'Windows 10 or later, macOS 11 or later, Linux',
      softwareVersion: version,
      license: 'https://opensource.org/licenses/MIT',
      isAccessibleForFree: true,
      offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
      inLanguage: ['es', 'en'],
      image: ogImage,
      author: { '@type': 'Organization', name: 'tlaku contributors' },
    };
    if (repoUrl) {
      app.downloadUrl = `${repoUrl}/releases/latest`;
      app.installUrl = `${repoUrl}/releases/latest`;
      app.releaseNotes = `${repoUrl}/releases/tag/v${version}`;
      app.sameAs = [repoUrl];
    }
    graphs.push(app);
  } else {
    const homePath = lang === 'en' ? 'en/' : '';
    graphs.push({
      '@context': 'https://schema.org',
      '@type': 'BreadcrumbList',
      itemListElement: [
        { '@type': 'ListItem', position: 1, name: 'Tlaku', item: abs(homePath) },
        { '@type': 'ListItem', position: 2, name: (html.match(/<h1[^>]*>([\s\S]*?)<\/h1>/)?.[1] ?? title).replace(/<[^>]*>/g, '').trim(), item: abs(path) },
      ],
    });
  }
  if (page.kind === 'faq') {
    const items = faqFromHtml(html);
    if (items.length === 0) throw new Error(`${path}: la página de preguntas no tiene <div class="qa">`);
    graphs.push({
      '@context': 'https://schema.org',
      '@type': 'FAQPage',
      inLanguage: lang,
      mainEntity: items.map((q) => ({
        '@type': 'Question',
        name: q.question,
        acceptedAnswer: { '@type': 'Answer', text: q.answer },
      })),
    });
  }
  for (const g of graphs) lines.push(`<script type="application/ld+json">\n${ldJson(g)}\n  </script>`);
  return lines.join('\n  ');
}

export function sitemapXml(registry) {
  const base = registry.site_url;
  const out = ['<?xml version="1.0" encoding="UTF-8"?>', '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">'];
  for (const page of registry.pages) {
    for (const lang of ['es', 'en']) {
      out.push('  <url>', `    <loc>${base + page[lang]}</loc>`);
      out.push(`    <xhtml:link rel="alternate" hreflang="es" href="${base + page.es}"/>`);
      out.push(`    <xhtml:link rel="alternate" hreflang="en" href="${base + page.en}"/>`);
      out.push(`    <xhtml:link rel="alternate" hreflang="x-default" href="${base + page.es}"/>`);
      out.push('  </url>');
    }
  }
  out.push('</urlset>', '');
  return out.join('\n');
}

export function robotsTxt(registry) {
  return `User-agent: *\nAllow: /\n\nSitemap: ${registry.site_url}sitemap.xml\n`;
}

/** Archivo de verificación de Google Search Console (`google<hex>.html` en la raíz): se publica tal cual. */
export const GOOGLE_VERIFY = /^google[0-9a-f]+\.html$/;
export function isGoogleVerification(name, text) {
  return GOOGLE_VERIFY.test(name) && text.trim() === `google-site-verification: ${name}`;
}
