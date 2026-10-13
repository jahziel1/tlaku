import type { Affine, BezPath, DesignObject, LayerSettings, PathSeg, Point } from '@/api/types';
import { ellipseBez, polylineBez, roundedRectBez, transformBez } from '@/lib/bezpath';
import { IDENTITY, multiplyAffine, objectCurves, unionBounds, objectBounds } from '@/lib/geometry';

/**
 * Portapapeles entre aplicaciones: convierte objetos a texto SVG (para copiar) y texto SVG a
 * objetos (para pegar desde Inkscape, Illustrator y otras). No usa el DOM: un lector de etiquetas
 * propio y un intérprete de datos de trayecto (M L H V C S Q T A Z), con transformaciones, estilos
 * heredados y unidades. Lo no vectorial (texto, imágenes, degradados) se omite y se cuenta.
 */

// ---- escribir ---------------------------------------------------------------------------------------------

const num = (n: number): string => String(Math.round(n * 10000) / 10000 + 0);

function pathData(path: BezPath, ox: number, oy: number): string {
  const p = ([x, y]: Point) => `${num(x - ox)} ${num(oy - y)}`;
  return path
    .map((s) => {
      switch (s.type) {
        case 'move_to': return `M${p(s.to)}`;
        case 'line_to': return `L${p(s.to)}`;
        case 'quad_to': return `Q${p(s.ctrl)} ${p(s.to)}`;
        case 'cubic_to': return `C${p(s.ctrl1)} ${p(s.ctrl2)} ${p(s.to)}`;
        case 'close': return 'Z';
      }
    })
    .join('');
}

const escapeAttr = (v: string) => v.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');

/**
 * SVG con las formas de `objects` (texto con contorno incluido; las imágenes se omiten). Medidas en
 * milímetros, con el eje Y hacia abajo como pide SVG y la esquina del conjunto en (0, 0). Las capas
 * de relleno salen rellenas y las demás como trazo. `null` si no hay nada que escribir.
 */
export function objectsToSvg(objects: readonly DesignObject[], layers: readonly LayerSettings[]): string | null {
  const drawable = objects.filter((o) => !o.hidden && (o.kind.type === 'vector' || (o.kind.type === 'text' && o.kind.outline.length > 0)));
  const box = unionBounds(drawable.map(objectBounds));
  if (!box || drawable.length === 0) return null;
  const [[minX], [, maxY]] = [box[0], box[1]];
  const w = box[1][0] - box[0][0];
  const h = box[1][1] - box[0][1];
  const mode = new Map(layers.map((l) => [l.color.toLowerCase(), l.mode]));
  const body = drawable
    .map((o) => {
      const d = objectCurves(o).map((p) => pathData(p, minX, maxY)).join('');
      const color = escapeAttr(o.layer);
      const filled = mode.get(o.layer.toLowerCase()) === 'fill';
      const paint = filled ? `fill="${color}" fill-rule="evenodd" stroke="none"` : `fill="none" stroke="${color}" stroke-width="0.1"`;
      return `  <path id="${escapeAttr(o.id)}" d="${d}" ${paint}/>`;
    })
    .join('\n');
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${num(w)}mm" height="${num(h)}mm" viewBox="0 0 ${num(w)} ${num(h)}">\n${body}\n</svg>\n`;
}

// ---- leer: etiquetas --------------------------------------------------------------------------------------

interface Tag {
  name: string;
  attrs: Record<string, string>;
  /** `open` abre un elemento, `close` lo cierra y `empty` es de una sola etiqueta (`<a/>`). */
  kind: 'open' | 'close' | 'empty';
}

const TAG_RE = /<!--[\s\S]*?-->|<!\[CDATA\[[\s\S]*?\]\]>|<!DOCTYPE[^>]*>|<\?[\s\S]*?\?>|<\/?[A-Za-z][^>]*>/g;
const ATTR_RE = /([^\s=/]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/g;

function decodeEntities(v: string): string {
  return v.replace(/&(#x[0-9a-f]+|#\d+|amp|lt|gt|quot|apos);/gi, (_, e: string) => {
    const l = e.toLowerCase();
    if (l === 'amp') return '&';
    if (l === 'lt') return '<';
    if (l === 'gt') return '>';
    if (l === 'quot') return '"';
    if (l === 'apos') return "'";
    return String.fromCodePoint(l.startsWith('#x') ? parseInt(l.slice(2), 16) : parseInt(l.slice(1), 10));
  });
}

function tokenize(text: string): Tag[] {
  const tags: Tag[] = [];
  for (const m of text.matchAll(TAG_RE)) {
    const raw = m[0];
    if (raw.startsWith('<!') || raw.startsWith('<?')) continue;
    const closing = raw.startsWith('</');
    const nameMatch = /^<\/?([A-Za-z][^\s/>]*)/.exec(raw);
    if (!nameMatch) continue;
    const name = nameMatch[1]!.replace(/^.*:/, '').toLowerCase(); // sin prefijo de espacio de nombres
    const attrs: Record<string, string> = {};
    if (!closing) {
      for (const a of raw.matchAll(ATTR_RE)) attrs[a[1]!.replace(/^.*:/, '').toLowerCase()] = decodeEntities(a[2] ?? a[3] ?? '');
    }
    tags.push({ name, attrs, kind: closing ? 'close' : raw.endsWith('/>') ? 'empty' : 'open' });
  }
  return tags;
}

/** ¿El texto parece un documento SVG? (para decidir si un texto pegado se importa como dibujo). */
export function looksLikeSvg(text: string): boolean {
  return /^\s*(<\?xml[^>]*>\s*)?(<!--[\s\S]*?-->\s*)*(<!DOCTYPE[^>]*>\s*)?<svg[\s>]/i.test(text);
}

// ---- leer: números, colores, transformaciones -------------------------------------------------------------

const PX_MM = 25.4 / 96;
const UNIT_MM: Record<string, number> = { mm: 1, cm: 10, in: 25.4, pt: 25.4 / 72, pc: 25.4 / 6, px: PX_MM, '': PX_MM };

/** Longitud SVG a mm (`null` si es un porcentaje o no se entiende). */
function lengthMM(v: string | undefined): number | null {
  if (!v) return null;
  const m = /^\s*([+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?)\s*(mm|cm|in|pt|pc|px)?\s*$/i.exec(v);
  if (!m) return null;
  return parseFloat(m[1]!) * UNIT_MM[(m[2] ?? '').toLowerCase()]!;
}

const numbers = (v: string | undefined): number[] => (v?.match(/[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?/gi) ?? []).map(Number);

const NAMED: Record<string, string> = {
  black: '#000000', white: '#ffffff', red: '#ff0000', lime: '#00ff00', green: '#008000', blue: '#0000ff', yellow: '#ffff00',
  cyan: '#00ffff', aqua: '#00ffff', magenta: '#ff00ff', fuchsia: '#ff00ff', orange: '#ffa500', purple: '#800080', gray: '#808080',
  grey: '#808080', silver: '#c0c0c0', maroon: '#800000', navy: '#000080', olive: '#808000', teal: '#008080', brown: '#a52a2a', pink: '#ffc0cb',
};

const hex2 = (n: number) => Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, '0');

/** Color CSS a `#rrggbb`; `null` si no es uno (`none`, `url(...)`, `currentColor`...). */
export function parseColor(v: string | undefined): string | null {
  if (!v) return null;
  const s = v.trim().toLowerCase();
  if (NAMED[s]) return NAMED[s]!;
  let m = /^#([0-9a-f]{3})$/.exec(s);
  if (m) return `#${m[1]!.split('').map((c) => c + c).join('')}`;
  m = /^#([0-9a-f]{6})(?:[0-9a-f]{2})?$/.exec(s);
  if (m) return `#${m[1]}`;
  m = /^rgba?\(([^)]+)\)$/.exec(s);
  if (m) {
    const parts = m[1]!.split(/[\s,/]+/).filter(Boolean).slice(0, 3);
    if (parts.length < 3) return null;
    const vals = parts.map((p) => (p.endsWith('%') ? (parseFloat(p) / 100) * 255 : parseFloat(p)));
    return vals.some((n) => !Number.isFinite(n)) ? null : `#${vals.map(hex2).join('')}`;
  }
  return null;
}

/** Cadena `transform` de SVG a afín (en el espacio SVG, Y hacia abajo). */
export function parseTransform(v: string | undefined): Affine {
  let m: Affine = IDENTITY;
  if (!v) return m;
  for (const part of v.matchAll(/(matrix|translate|scale|rotate|skewX|skewY)\s*\(([^)]*)\)/g)) {
    const a = numbers(part[2]);
    let t: Affine | null = null;
    switch (part[1]) {
      case 'matrix': if (a.length >= 6) t = [a[0]!, a[1]!, a[2]!, a[3]!, a[4]!, a[5]!]; break;
      case 'translate': t = [1, 0, 0, 1, a[0] ?? 0, a[1] ?? 0]; break;
      case 'scale': t = [a[0] ?? 1, 0, 0, a[1] ?? a[0] ?? 1, 0, 0]; break;
      case 'rotate': {
        const r = ((a[0] ?? 0) * Math.PI) / 180;
        const [cx, cy] = [a[1] ?? 0, a[2] ?? 0];
        const c = Math.cos(r);
        const s = Math.sin(r);
        t = [c, s, -s, c, cx - (c * cx - s * cy), cy - (s * cx + c * cy)];
        break;
      }
      case 'skewX': t = [1, 0, Math.tan(((a[0] ?? 0) * Math.PI) / 180), 1, 0, 0]; break;
      case 'skewY': t = [1, Math.tan(((a[0] ?? 0) * Math.PI) / 180), 0, 1, 0, 0]; break;
    }
    if (t) m = multiplyAffine(m, t);
  }
  return m;
}

// ---- leer: datos de trayecto ----------------------------------------------------------------------------------

/** Arco SVG (parametrización por extremos) a cúbicas, según el apéndice de implementación del estándar SVG. */
function arcToCubics(from: Point, rx: number, ry: number, phiDeg: number, large: boolean, sweep: boolean, to: Point): PathSeg[] {
  if (from[0] === to[0] && from[1] === to[1]) return [];
  rx = Math.abs(rx);
  ry = Math.abs(ry);
  if (rx < 1e-12 || ry < 1e-12) return [{ type: 'line_to', to }];
  const phi = (phiDeg * Math.PI) / 180;
  const [cp, sp] = [Math.cos(phi), Math.sin(phi)];
  const dx = (from[0] - to[0]) / 2;
  const dy = (from[1] - to[1]) / 2;
  const x1 = cp * dx + sp * dy;
  const y1 = -sp * dx + cp * dy;
  const lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
  if (lambda > 1) {
    rx *= Math.sqrt(lambda);
    ry *= Math.sqrt(lambda);
  }
  const num2 = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
  const den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
  const coef = (large === sweep ? -1 : 1) * Math.sqrt(Math.max(0, num2 / den));
  const cxp = (coef * rx * y1) / ry;
  const cyp = (-coef * ry * x1) / rx;
  const cx = cp * cxp - sp * cyp + (from[0] + to[0]) / 2;
  const cy = sp * cxp + cp * cyp + (from[1] + to[1]) / 2;
  const angle = (ux: number, uy: number, vx: number, vy: number) => {
    const a = Math.atan2(ux * vy - uy * vx, ux * vx + uy * vy);
    return a;
  };
  const theta1 = angle(1, 0, (x1 - cxp) / rx, (y1 - cyp) / ry);
  let delta = angle((x1 - cxp) / rx, (y1 - cyp) / ry, (-x1 - cxp) / rx, (-y1 - cyp) / ry);
  if (!sweep && delta > 0) delta -= Math.PI * 2;
  if (sweep && delta < 0) delta += Math.PI * 2;
  const n = Math.max(1, Math.ceil(Math.abs(delta) / (Math.PI / 2) - 1e-9));
  const step = delta / n;
  const k = (4 / 3) * Math.tan(step / 4);
  const out: PathSeg[] = [];
  const at = (t: number): [Point, Point] => {
    const [ct, st] = [Math.cos(t), Math.sin(t)];
    const pt: Point = [cx + rx * ct * cp - ry * st * sp, cy + rx * ct * sp + ry * st * cp];
    const d: Point = [-rx * st * cp - ry * ct * sp, -rx * st * sp + ry * ct * cp];
    return [pt, d];
  };
  for (let i = 0; i < n; i++) {
    const [p0, d0] = at(theta1 + i * step);
    const [p1, d1] = at(theta1 + (i + 1) * step);
    out.push({
      type: 'cubic_to',
      ctrl1: [p0[0] + k * d0[0], p0[1] + k * d0[1]],
      ctrl2: [p1[0] - k * d1[0], p1[1] - k * d1[1]],
      to: i === n - 1 ? to : p1,
    });
  }
  return out;
}

/** Datos `d` de un `<path>` a trayectos (uno por cada `M`... no: un solo trayecto con varios subtrayectos). */
export function parsePathData(d: string): BezPath {
  const tokens = d.match(/[a-zA-Z]|[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?/g) ?? [];
  const path: PathSeg[] = [];
  let i = 0;
  let cmd = '';
  let cur: Point = [0, 0];
  let start: Point = [0, 0];
  let lastCtrl: Point | null = null; // para S y T
  let lastKind = '';
  const isNum = () => i < tokens.length && !/^[a-zA-Z]$/.test(tokens[i]!);
  const next = () => parseFloat(tokens[i++]!);
  const flag = () => {
    // Las banderas de arco pueden venir pegadas ("a1 1 0 00.5.5"): un solo dígito 0/1.
    const t = tokens[i]!;
    if (t.length > 1 && /^[01]/.test(t) && !t.includes('.')) {
      tokens[i] = t.slice(1);
      return t[0] === '1';
    }
    i++;
    return parseFloat(t) !== 0;
  };
  while (i < tokens.length) {
    if (!isNum()) cmd = tokens[i++]!;
    else if (cmd === '') break;
    const rel = cmd === cmd.toLowerCase();
    const up = cmd.toUpperCase();
    const abs = (x: number, y: number): Point => (rel ? [cur[0] + x, cur[1] + y] : [x, y]);
    if (up === 'Z') {
      path.push({ type: 'close' });
      cur = start;
      lastCtrl = null;
      lastKind = 'Z';
      continue;
    }
    if (!isNum()) continue; // comando sin argumentos
    switch (up) {
      case 'M': {
        const p = abs(next(), next());
        path.push({ type: 'move_to', to: p });
        cur = start = p;
        cmd = rel ? 'l' : 'L'; // los pares siguientes son rectas
        lastCtrl = null;
        lastKind = 'M';
        break;
      }
      case 'L': {
        const p = abs(next(), next());
        path.push({ type: 'line_to', to: p });
        cur = p;
        lastCtrl = null;
        lastKind = 'L';
        break;
      }
      case 'H': {
        const x = next();
        cur = [rel ? cur[0] + x : x, cur[1]];
        path.push({ type: 'line_to', to: cur });
        lastCtrl = null;
        lastKind = 'L';
        break;
      }
      case 'V': {
        const y = next();
        cur = [cur[0], rel ? cur[1] + y : y];
        path.push({ type: 'line_to', to: cur });
        lastCtrl = null;
        lastKind = 'L';
        break;
      }
      case 'C': {
        const c1 = abs(next(), next());
        const c2 = abs(next(), next());
        const to = abs(next(), next());
        path.push({ type: 'cubic_to', ctrl1: c1, ctrl2: c2, to });
        cur = to;
        lastCtrl = c2;
        lastKind = 'C';
        break;
      }
      case 'S': {
        const c1: Point = lastKind === 'C' && lastCtrl ? [2 * cur[0] - lastCtrl[0], 2 * cur[1] - lastCtrl[1]] : cur;
        const c2 = abs(next(), next());
        const to = abs(next(), next());
        path.push({ type: 'cubic_to', ctrl1: c1, ctrl2: c2, to });
        cur = to;
        lastCtrl = c2;
        lastKind = 'C';
        break;
      }
      case 'Q': {
        const c = abs(next(), next());
        const to = abs(next(), next());
        path.push({ type: 'quad_to', ctrl: c, to });
        cur = to;
        lastCtrl = c;
        lastKind = 'Q';
        break;
      }
      case 'T': {
        const c: Point = lastKind === 'Q' && lastCtrl ? [2 * cur[0] - lastCtrl[0], 2 * cur[1] - lastCtrl[1]] : cur;
        const to = abs(next(), next());
        path.push({ type: 'quad_to', ctrl: c, to });
        cur = to;
        lastCtrl = c;
        lastKind = 'Q';
        break;
      }
      case 'A': {
        const rx = next();
        const ry = next();
        const rot = next();
        const large = flag();
        const sweep = flag();
        const to = abs(next(), next());
        path.push(...arcToCubics(cur, rx, ry, rot, large, sweep, to));
        cur = to;
        lastCtrl = null;
        lastKind = 'A';
        break;
      }
      default:
        i = tokens.length; // comando desconocido: se corta aquí
    }
  }
  return path;
}

// ---- leer: documento ------------------------------------------------------------------------------------------------

export interface ParsedSvg {
  /** Objetos con la esquina inferior izquierda del conjunto en (0, 0), en mm y con Y hacia arriba. */
  objects: DesignObject[];
  /** Medidas del conjunto en mm. */
  width: number;
  height: number;
  /** Elementos que no se pudieron convertir (texto, imágenes, degradados...). */
  skipped: number;
  /** Colores de las capas que usan los objetos. */
  layers: { color: string; fill: boolean }[];
}

interface Style {
  stroke?: string;
  fill?: string;
  hidden?: boolean;
}

const SKIP_CONTAINERS = new Set(['defs', 'clippath', 'mask', 'symbol', 'pattern', 'marker', 'metadata', 'title', 'desc', 'style', 'lineargradient', 'radialgradient', 'filter', 'script']);
const CONTAINERS = new Set(['svg', 'g', 'a', 'switch']);
const UNSUPPORTED = new Set(['text', 'image', 'use', 'foreignobject']);

function styleOf(attrs: Record<string, string>, parent: Style): Style {
  const out: Style = { ...parent };
  const apply = (k: string, v: string) => {
    const key = k.trim().toLowerCase();
    const val = v.trim();
    if (key === 'stroke') out.stroke = val;
    else if (key === 'fill') out.fill = val;
    else if (key === 'display' && val === 'none') out.hidden = true;
    else if (key === 'visibility' && (val === 'hidden' || val === 'collapse')) out.hidden = true;
  };
  for (const k of ['stroke', 'fill', 'display', 'visibility']) if (attrs[k] !== undefined) apply(k, attrs[k]!);
  for (const decl of (attrs.style ?? '').split(';')) {
    const at = decl.indexOf(':');
    if (at > 0) apply(decl.slice(0, at), decl.slice(at + 1));
  }
  return out;
}

/**
 * Convierte texto SVG en objetos vectoriales. `null` si no es un SVG. Cada forma (`path`, `rect`,
 * `circle`, `ellipse`, `line`, `polyline`, `polygon`) es un objeto. La capa sale del color del
 * trazo (o del relleno si no hay trazo); lo que no tiene color propio queda en `fallbackLayer`.
 */
export function svgToObjects(text: string, fallbackLayer: string): ParsedSvg | null {
  if (!looksLikeSvg(text)) return null;
  const tags = tokenize(text);
  const root = tags.find((t) => t.name === 'svg' && t.kind !== 'close');
  if (!root) return null;
  const vb = numbers(root.attrs.viewbox);
  const widthMM = lengthMM(root.attrs.width);
  // Escala usuario -> mm: con ancho y viewBox se estira; si no, 1 unidad = 1 px a 96 ppp.
  const k = vb.length === 4 && widthMM && vb[2]! > 0 ? widthMM / vb[2]! : PX_MM;
  const base: Affine = [k, 0, 0, -k, -(vb[0] ?? 0) * k, (vb[1] ?? 0) * k];

  const stack: { ctm: Affine; style: Style }[] = [{ ctm: base, style: { fill: 'black' } }];
  const objects: DesignObject[] = [];
  const layers = new Map<string, boolean>();
  let skipDepth = 0;
  let skipped = 0;
  let seq = 0;

  const shape = (tag: Tag, ctm: Affine, style: Style) => {
    const a = tag.attrs;
    // Medidas del elemento en unidades de usuario: la matriz global las lleva a mm.
    const u = (v: string | undefined) => numbers(v)[0] ?? 0;
    let paths: BezPath[] = [];
    switch (tag.name) {
      case 'path': paths = [parsePathData(a.d ?? '')]; break;
      case 'rect': {
        const [x, y, w, h] = [u(a.x), u(a.y), u(a.width), u(a.height)];
        const rx = a.rx !== undefined ? u(a.rx) : a.ry !== undefined ? u(a.ry) : 0;
        // SVG tiene Y hacia abajo: el rectángulo se arma con las esquinas dadas y la matriz global lo vuelca.
        if (w > 0 && h > 0) paths = [roundedRectBez(x, y, x + w, y + h, rx)];
        break;
      }
      case 'circle': if (u(a.r) > 0) paths = [ellipseBez(u(a.cx), u(a.cy), u(a.r), u(a.r))]; break;
      case 'ellipse': if (u(a.rx) > 0 && u(a.ry) > 0) paths = [ellipseBez(u(a.cx), u(a.cy), u(a.rx), u(a.ry))]; break;
      case 'line': paths = [polylineBez([[u(a.x1), u(a.y1)], [u(a.x2), u(a.y2)]], false)]; break;
      case 'polyline':
      case 'polygon': {
        const v = numbers(a.points);
        const pts: Point[] = [];
        for (let j = 0; j + 1 < v.length; j += 2) pts.push([v[j]!, v[j + 1]!]);
        if (pts.length >= 2) paths = [polylineBez(pts, tag.name === 'polygon')];
        break;
      }
      default: return;
    }
    paths = paths.filter((p) => p.length >= 2);
    if (paths.length === 0 || style.hidden) return;
    const stroke = parseColor(style.stroke);
    const fill = parseColor(style.fill);
    const color = stroke ?? fill ?? fallbackLayer;
    const filled = !stroke && !!fill;
    layers.set(color, (layers.get(color) ?? false) || filled);
    const m = multiplyAffine(ctm, parseTransform(a.transform));
    objects.push({
      id: `svg-${++seq}`,
      layer: color,
      transform: IDENTITY,
      kind: { type: 'vector', paths: paths.map((p) => transformBez(p, m)) },
      locked: false,
      hidden: false,
      group: null,
    });
  };

  for (const tag of tags) {
    if (tag.kind === 'close') {
      if (skipDepth > 0) {
        if (SKIP_CONTAINERS.has(tag.name)) skipDepth--;
      } else if (CONTAINERS.has(tag.name) && stack.length > 1) stack.pop();
      continue;
    }
    if (skipDepth > 0) {
      if (tag.kind === 'open' && SKIP_CONTAINERS.has(tag.name)) skipDepth++;
      continue;
    }
    if (SKIP_CONTAINERS.has(tag.name)) {
      if (tag.kind === 'open') skipDepth++;
      continue;
    }
    const top = stack[stack.length - 1]!;
    if (CONTAINERS.has(tag.name)) {
      const frame = {
        ctm: tag === root ? top.ctm : multiplyAffine(top.ctm, parseTransform(tag.attrs.transform)),
        style: styleOf(tag.attrs, top.style),
      };
      if (tag.kind === 'open') stack.push(frame);
      continue;
    }
    if (UNSUPPORTED.has(tag.name)) {
      skipped++;
      continue;
    }
    shape(tag, top.ctm, styleOf(tag.attrs, top.style));
  }

  const box = unionBounds(objects.map(objectBounds));
  if (!box || objects.length === 0) return null;
  const shift: Affine = [1, 0, 0, 1, -box[0][0], -box[0][1]];
  return {
    objects: objects.map((o) => ({ ...o, kind: { type: 'vector', paths: o.kind.type === 'vector' ? o.kind.paths.map((p) => transformBez(p, shift)) : [] } })),
    width: box[1][0] - box[0][0],
    height: box[1][1] - box[0][1],
    skipped,
    layers: [...layers].map(([color, fill]) => ({ color, fill })),
  };
}
