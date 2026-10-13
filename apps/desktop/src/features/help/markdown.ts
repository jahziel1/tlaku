/**
 * Intérprete de Markdown mínimo para el manual. Solo entiende el subconjunto que usa docs/manual/:
 * títulos, párrafos, listas (con anidado), tablas, bloques de código, citas, línea horizontal,
 * imágenes, `código`, **negrita**, *cursiva* y enlaces. No hay HTML: el resultado es un árbol de
 * datos que `MarkdownView` pinta con elementos de React, así que ningún texto del manual puede
 * inyectar marcado.
 */

export type Inline =
  | { t: 'text'; v: string }
  | { t: 'code'; v: string }
  | { t: 'strong'; c: Inline[] }
  | { t: 'em'; c: Inline[] }
  | { t: 'link'; href: string; c: Inline[] }
  | { t: 'image'; alt: string; src: string };

export type Align = 'left' | 'center' | 'right' | null;

export interface ListItem {
  c: Inline[];
  /** Bloques dentro del elemento: listas anidadas, párrafos de continuación, código. */
  sub: Block[];
}

export type Block =
  | { t: 'heading'; level: number; id: string; c: Inline[] }
  | { t: 'p'; c: Inline[] }
  | { t: 'list'; ordered: boolean; items: ListItem[] }
  | { t: 'table'; head: Inline[][]; rows: Inline[][][]; align: Align[] }
  | { t: 'code'; lang: string; v: string }
  | { t: 'quote'; c: Block[] }
  | { t: 'hr' }
  | { t: 'image'; alt: string; src: string };

// ---- inline ---------------------------------------------------------------------------------

const ESCAPABLE = /[\\`*_{}[\]()#+\-.!|<>~]/;

/** Busca `[texto](destino)` que empieza en `i` (donde está el `[`). */
function matchLink(src: string, i: number): { text: string; href: string; end: number } | null {
  let depth = 0;
  let j = i;
  for (; j < src.length; j++) {
    const ch = src[j];
    if (ch === '\\') j++;
    else if (ch === '[') depth++;
    else if (ch === ']' && --depth === 0) break;
  }
  if (j >= src.length || src[j + 1] !== '(') return null;
  const text = src.slice(i + 1, j);
  let k = j + 2;
  let paren = 1;
  for (; k < src.length; k++) {
    if (src[k] === '(') paren++;
    else if (src[k] === ')' && --paren === 0) break;
  }
  if (k >= src.length) return null;
  const href = src
    .slice(j + 2, k)
    .trim()
    .replace(/\s+"[^"]*"$/, '');
  return { text, href, end: k + 1 };
}

/** Posición del `*` que cierra una cursiva abierta justo antes de `from`, o -1. */
function closeEmphasis(src: string, from: number): number {
  for (let k = from; k < src.length; k++) {
    if (src[k] === '`') {
      const end = src.indexOf('`', k + 1);
      if (end < 0) return -1;
      k = end;
    } else if (src[k] === '*' && src[k + 1] !== '*' && src[k - 1] !== '*' && !/\s/.test(src[k - 1] ?? ' ')) {
      return k;
    }
  }
  return -1;
}

const isWord = (ch: string | undefined) => ch !== undefined && /[\p{L}\p{N}]/u.test(ch);

export function parseInline(src: string): Inline[] {
  const out: Inline[] = [];
  let buf = '';
  const flush = () => {
    if (buf) out.push({ t: 'text', v: buf });
    buf = '';
  };
  let i = 0;
  while (i < src.length) {
    const ch = src[i];
    if (ch === '\\' && i + 1 < src.length && ESCAPABLE.test(src[i + 1])) {
      buf += src[i + 1];
      i += 2;
      continue;
    }
    if (ch === '`') {
      let n = 1;
      while (src[i + n] === '`') n++;
      const fence = '`'.repeat(n);
      const end = src.indexOf(fence, i + n);
      if (end > i) {
        flush();
        out.push({ t: 'code', v: src.slice(i + n, end).trim() });
        i = end + n;
      } else {
        buf += fence;
        i += n;
      }
      continue;
    }
    if (ch === '!' && src[i + 1] === '[') {
      const m = matchLink(src, i + 1);
      if (m) {
        flush();
        out.push({ t: 'image', alt: m.text, src: m.href });
        i = m.end;
        continue;
      }
    }
    if (ch === '[') {
      const m = matchLink(src, i);
      if (m) {
        flush();
        out.push({ t: 'link', href: m.href, c: parseInline(m.text) });
        i = m.end;
        continue;
      }
    }
    if (ch === '<') {
      const auto = /^<((?:https?:\/\/|mailto:)[^>\s]+)>/.exec(src.slice(i));
      if (auto) {
        flush();
        out.push({ t: 'link', href: auto[1], c: [{ t: 'text', v: auto[1] }] });
        i += auto[0].length;
        continue;
      }
    }
    if (ch === '*' && src[i + 1] === '*') {
      const end = src.indexOf('**', i + 2);
      if (end > i + 2) {
        flush();
        out.push({ t: 'strong', c: parseInline(src.slice(i + 2, end)) });
        i = end + 2;
        continue;
      }
    }
    if (ch === '*' && src[i + 1] !== '*' && !/\s/.test(src[i + 1] ?? ' ')) {
      const end = closeEmphasis(src, i + 1);
      if (end > i + 1) {
        flush();
        out.push({ t: 'em', c: parseInline(src.slice(i + 1, end)) });
        i = end + 1;
        continue;
      }
    }
    if (ch === '_' && !isWord(src[i - 1]) && src[i + 1] !== '_' && !/\s/.test(src[i + 1] ?? ' ')) {
      const end = src.indexOf('_', i + 1);
      if (end > i + 1 && !isWord(src[end + 1]) && !/\s/.test(src[end - 1])) {
        flush();
        out.push({ t: 'em', c: parseInline(src.slice(i + 1, end)) });
        i = end + 1;
        continue;
      }
    }
    buf += ch;
    i++;
  }
  flush();
  return out;
}

/** Texto plano de un fragmento inline (para ids de títulos, búsqueda y títulos de página). */
export function plainText(nodes: Inline[]): string {
  return nodes
    .map((n) => {
      switch (n.t) {
        case 'text':
        case 'code':
          return n.v;
        case 'image':
          return n.alt;
        default:
          return plainText(n.c);
      }
    })
    .join('');
}

export function slugify(text: string): string {
  return text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

// ---- bloques --------------------------------------------------------------------------------

const LIST_RE = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/;
const FENCE_RE = /^\s*```\s*([\w-]*)\s*$/;
const HEADING_RE = /^(#{1,6})\s+(.*?)(?:\s+#+)?\s*$/;
const HR_RE = /^\s*([-*_])(\s*\1){2,}\s*$/;
const TABLE_SEP_RE = /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/;

const indentOf = (line: string) => /^\s*/.exec(line)![0].length;
const isBlank = (line: string) => line.trim() === '';

function splitRow(line: string): string[] {
  let s = line.trim();
  if (s.startsWith('|')) s = s.slice(1);
  if (s.endsWith('|') && !s.endsWith('\\|')) s = s.slice(0, -1);
  const cells: string[] = [];
  let cur = '';
  let inCode = false;
  for (let i = 0; i < s.length; i++) {
    const ch = s[i];
    if (ch === '`') inCode = !inCode;
    if (ch === '\\' && s[i + 1] === '|') {
      cur += '|';
      i++;
    } else if (ch === '|' && !inCode) {
      cells.push(cur.trim());
      cur = '';
    } else cur += ch;
  }
  cells.push(cur.trim());
  return cells;
}

function alignOf(cell: string): Align {
  const l = cell.startsWith(':');
  const r = cell.endsWith(':');
  if (l && r) return 'center';
  if (r) return 'right';
  if (l) return 'left';
  return null;
}

function startsBlock(line: string, next: string | undefined): boolean {
  return (
    FENCE_RE.test(line) ||
    HEADING_RE.test(line) ||
    HR_RE.test(line) ||
    /^\s*>/.test(line) ||
    LIST_RE.test(line) ||
    (line.includes('|') && next !== undefined && TABLE_SEP_RE.test(next))
  );
}

function dedent(lines: string[]): string[] {
  const widths = lines.filter((l) => !isBlank(l)).map(indentOf);
  const cut = widths.length ? Math.min(...widths) : 0;
  return lines.map((l) => l.slice(Math.min(cut, indentOf(l))));
}

function parseList(lines: string[], start: number): { block: Block; next: number } {
  const first = LIST_RE.exec(lines[start])!;
  const base = first[1].length;
  const ordered = /\d/.test(first[2]);
  const items: ListItem[] = [];
  let i = start;
  while (i < lines.length) {
    // Una línea en blanco solo continúa la lista si le sigue otro elemento al mismo nivel.
    if (isBlank(lines[i])) {
      let j = i;
      while (j < lines.length && isBlank(lines[j])) j++;
      const m = j < lines.length ? LIST_RE.exec(lines[j]) : null;
      if (m && m[1].length === base && /\d/.test(m[2]) === ordered) {
        i = j;
        continue;
      }
      break;
    }
    const m = LIST_RE.exec(lines[i]);
    if (!m || m[1].length !== base || /\d/.test(m[2]) !== ordered) break;
    let text = m[3];
    i++;
    const body: string[] = [];
    while (i < lines.length) {
      if (isBlank(lines[i])) {
        let j = i;
        while (j < lines.length && isBlank(lines[j])) j++;
        if (j < lines.length && indentOf(lines[j]) > base) {
          body.push('');
          i++;
          continue;
        }
        break;
      }
      if (indentOf(lines[i]) <= base) break;
      body.push(lines[i]);
      i++;
    }
    // Líneas de continuación del mismo párrafo (sangradas, sin marcador).
    while (body.length && !isBlank(body[0]) && !LIST_RE.test(body[0]) && !FENCE_RE.test(body[0])) {
      text += ` ${body.shift()!.trim()}`;
    }
    items.push({ c: parseInline(text), sub: parseBlocks(dedent(body)) });
  }
  return { block: { t: 'list', ordered, items }, next: i };
}

function parseBlocks(lines: string[]): Block[] {
  const blocks: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (isBlank(line)) {
      i++;
      continue;
    }
    const fence = FENCE_RE.exec(line);
    if (fence) {
      const body: string[] = [];
      i++;
      while (i < lines.length && !FENCE_RE.test(lines[i])) body.push(lines[i++]);
      i++; // cierre
      blocks.push({ t: 'code', lang: fence[1], v: body.join('\n') });
      continue;
    }
    const h = HEADING_RE.exec(line);
    if (h) {
      blocks.push({ t: 'heading', level: h[1].length, id: '', c: parseInline(h[2]) });
      i++;
      continue;
    }
    if (HR_RE.test(line)) {
      blocks.push({ t: 'hr' });
      i++;
      continue;
    }
    if (/^\s*>/.test(line)) {
      const inner: string[] = [];
      while (i < lines.length && /^\s*>/.test(lines[i])) inner.push(lines[i++].replace(/^\s*> ?/, ''));
      blocks.push({ t: 'quote', c: parseBlocks(inner) });
      continue;
    }
    if (line.includes('|') && i + 1 < lines.length && TABLE_SEP_RE.test(lines[i + 1])) {
      const head = splitRow(line).map(parseInline);
      const align = splitRow(lines[i + 1]).map(alignOf);
      i += 2;
      const rows: Inline[][][] = [];
      while (i < lines.length && !isBlank(lines[i]) && lines[i].includes('|')) {
        const cells = splitRow(lines[i++]).map(parseInline);
        while (cells.length < head.length) cells.push([]);
        rows.push(cells.slice(0, head.length));
      }
      blocks.push({ t: 'table', head, rows, align });
      continue;
    }
    if (LIST_RE.test(line)) {
      const { block, next } = parseList(lines, i);
      blocks.push(block);
      i = next;
      continue;
    }
    const para: string[] = [];
    while (i < lines.length && !isBlank(lines[i]) && (para.length === 0 || !startsBlock(lines[i], lines[i + 1]))) {
      para.push(lines[i++].trim());
    }
    const inline = parseInline(para.join(' '));
    if (inline.length === 1 && inline[0].t === 'image') {
      blocks.push({ t: 'image', alt: inline[0].alt, src: inline[0].src });
    } else {
      blocks.push({ t: 'p', c: inline });
    }
  }
  return blocks;
}

/** Interpreta un documento. Los títulos reciben un `id` único (para enlaces a secciones). */
export function parseMarkdown(src: string): Block[] {
  const blocks = parseBlocks(src.replace(/\r\n?/g, '\n').split('\n'));
  const seen = new Map<string, number>();
  for (const b of blocks) {
    if (b.t !== 'heading') continue;
    const slug = slugify(plainText(b.c)) || 'seccion';
    const n = seen.get(slug) ?? 0;
    seen.set(slug, n + 1);
    b.id = n === 0 ? slug : `${slug}-${n}`;
  }
  return blocks;
}

/** Texto del primer `# Título` del documento, o `fallback`. */
export function documentTitle(blocks: Block[], fallback: string): string {
  const h = blocks.find((b) => b.t === 'heading' && b.level === 1);
  return h && h.t === 'heading' ? plainText(h.c) : fallback;
}

/** Texto plano de todo el documento, en minúsculas y sin acentos (búsqueda). */
export function searchText(blocks: Block[]): string {
  const parts: string[] = [];
  const walk = (bs: Block[]) => {
    for (const b of bs) {
      switch (b.t) {
        case 'heading':
        case 'p':
          parts.push(plainText(b.c));
          break;
        case 'list':
          for (const it of b.items) {
            parts.push(plainText(it.c));
            walk(it.sub);
          }
          break;
        case 'table':
          for (const row of [b.head, ...b.rows]) for (const cell of row) parts.push(plainText(cell));
          break;
        case 'quote':
          walk(b.c);
          break;
        case 'code':
          parts.push(b.v);
          break;
        default:
      }
    }
  };
  walk(blocks);
  return normalizeSearch(parts.join(' '));
}

export function normalizeSearch(text: string): string {
  return text.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
}

export type LinkTarget =
  | { kind: 'page'; file: string; anchor: string }
  | { kind: 'anchor'; anchor: string }
  | { kind: 'external'; url: string }
  | { kind: 'none' };

/** Qué hace un enlace del manual: otra página, una sección de esta, una dirección externa o nada. */
export function classifyLink(href: string): LinkTarget {
  if (href.startsWith('#')) return { kind: 'anchor', anchor: href.slice(1) };
  if (/^(https?:|mailto:)/i.test(href)) return { kind: 'external', url: href };
  const m = /^(?:\.{0,2}\/)*(?:[\w-]+\/)*([\w.-]+\.md)(?:#([\w-]*))?$/.exec(href);
  if (m) return { kind: 'page', file: m[1], anchor: m[2] ?? '' };
  return { kind: 'none' };
}
