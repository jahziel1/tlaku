import type { DesignObject, Point } from '@/api/types';
import { t } from '@/i18n';
import { objectPolylines, pointsBounds } from '@/lib/geometry';

/** Qué es un objeto, a grandes rasgos, para nombrarlo con algo útil en vez de su id interno. */
export type UnitKind = 'rect' | 'square' | 'circle' | 'ellipse' | 'polygon' | 'closed' | 'line' | 'open' | 'compound' | 'text' | 'image' | 'group' | 'empty';

export interface UnitDescription {
  kind: UnitKind;
  /** Valores ya formateados para el texto (`w`, `h`, `d`, `len`, `n`, `text`). */
  params: Record<string, string | number>;
}

type Flat = { points: Point[]; closed: boolean };

const TEXT_MAX = 24;
const POLYGON_MAX_SIDES = 12;
/** Un trazo de más puntos que esto ya no es "simple": no se dibuja miniatura. */
const THUMB_MAX_POINTS = 600;
const THUMB_SIZE = 100;
/** Margen alrededor del dibujo (mismas unidades) para que una línea recta tenga caja y el trazo no se corte. */
const THUMB_PAD = 6;

/** Milímetros con coma decimal y a lo más un decimal ("40", "12,5"). */
export const mm = (v: number): string => new Intl.NumberFormat('es', { maximumFractionDigits: 1 }).format(v);

const dist = (a: Point, b: Point) => Math.hypot(a[0] - b[0], a[1] - b[1]);

/** Lados (w, h) si los cuatro puntos son un rectángulo, girado o no; si no, `null`. */
function rectangleSides(points: Point[]): [number, number] | null {
  if (points.length !== 4) return null;
  const e = points.map((p, i) => {
    const q = points[(i + 1) % 4]!;
    return [q[0] - p[0], q[1] - p[1]] as Point;
  });
  const [w, h] = [Math.hypot(...e[0]!), Math.hypot(...e[1]!)];
  if (w < 1e-9 || h < 1e-9) return null;
  const perpendicular = (a: Point, b: Point) => Math.abs(a[0] * b[0] + a[1] * b[1]) <= 1e-6 * Math.hypot(...a) * Math.hypot(...b);
  const ok = perpendicular(e[0]!, e[1]!) && perpendicular(e[1]!, e[2]!) && perpendicular(e[2]!, e[3]!);
  const equal = Math.abs(Math.hypot(...e[2]!) - w) <= 1e-6 * w && Math.abs(Math.hypot(...e[3]!) - h) <= 1e-6 * h;
  return ok && equal ? [w, h] : null;
}

/** Semiejes (rx, ry) si todos los puntos caen sobre la elipse de su caja (alineada con los ejes). */
function ellipseRadii(points: Point[]): [number, number] | null {
  if (points.length < 12) return null;
  const b = pointsBounds(points);
  if (!b) return null;
  const [cx, cy] = [(b[0][0] + b[1][0]) / 2, (b[0][1] + b[1][1]) / 2];
  const [rx, ry] = [(b[1][0] - b[0][0]) / 2, (b[1][1] - b[0][1]) / 2];
  if (rx < 1e-6 || ry < 1e-6) return null;
  // El aplanado se aparta hasta 0,01 mm de la curva: la tolerancia en unidades normalizadas crece con radios chicos.
  const tol = Math.max(0.01, 0.06 / Math.min(rx, ry));
  const fits = points.every(([x, y]) => Math.abs(((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 - 1) <= tol);
  return fits ? [rx, ry] : null;
}

function describeFlat(paths: Flat[]): UnitDescription {
  const box = pointsBounds(paths.flatMap((p) => p.points));
  if (paths.length === 0 || !box) return { kind: 'empty', params: {} };
  const size = { w: mm(box[1][0] - box[0][0]), h: mm(box[1][1] - box[0][1]) };
  if (paths.length > 1) return { kind: 'compound', params: { n: paths.length, ...size } };
  const path = paths[0]!;
  if (!path.closed) {
    if (path.points.length === 2) return { kind: 'line', params: { len: mm(dist(path.points[0]!, path.points[1]!)) } };
    return { kind: 'open', params: size };
  }
  const sides = rectangleSides(path.points);
  if (sides) {
    const [w, h] = sides;
    return Math.abs(w - h) <= 1e-6 * w ? { kind: 'square', params: { w: mm(w) } } : { kind: 'rect', params: { w: mm(w), h: mm(h) } };
  }
  const radii = ellipseRadii(path.points);
  if (radii) {
    const [rx, ry] = radii;
    return Math.abs(rx - ry) <= 1e-3 * Math.max(rx, ry) ? { kind: 'circle', params: { d: mm(2 * rx) } } : { kind: 'ellipse', params: { w: mm(2 * rx), h: mm(2 * ry) } };
  }
  if (path.points.length <= POLYGON_MAX_SIDES) return { kind: 'polygon', params: { n: path.points.length, ...size } };
  return { kind: 'closed', params: size };
}

/** Texto de una línea para el rótulo: primera línea, recortada. */
function shortText(text: string): string {
  const line = (text.split('\n')[0] ?? '').trim();
  return line.length > TEXT_MAX ? `${line.slice(0, TEXT_MAX - 1)}…` : line;
}

export function describeObject(o: DesignObject): UnitDescription {
  if (o.kind.type === 'text') return { kind: 'text', params: { text: shortText(o.kind.text) } };
  const flat = objectPolylines(o);
  if (o.kind.type === 'image') {
    const box = pointsBounds(flat.flatMap((p) => p.points));
    return { kind: 'image', params: box ? { w: mm(box[1][0] - box[0][0]), h: mm(box[1][1] - box[0][1]) } : {} };
  }
  return describeFlat(flat);
}

/** Un grupo (varios objetos) se nombra por su cantidad; un objeto suelto, por lo que es. */
export function describeUnit(u: { objects: readonly DesignObject[] }): UnitDescription {
  if (u.objects.length > 1) return { kind: 'group', params: { n: u.objects.length } };
  const only = u.objects[0];
  return only ? describeObject(only) : { kind: 'empty', params: {} };
}

/** Nombre legible de una unidad: "Rectángulo 40×20 mm", "Texto «Hola»", "Imagen 30×20 mm", "Grupo (3)". */
export function unitLabel(u: { objects: readonly DesignObject[] }): string {
  const d = describeUnit(u);
  switch (d.kind) {
    case 'rect':
      return t('arrange.label.rect', d.params);
    case 'square':
      return t('arrange.label.square', d.params);
    case 'circle':
      return t('arrange.label.circle', d.params);
    case 'ellipse':
      return t('arrange.label.ellipse', d.params);
    case 'polygon':
      return t('arrange.label.polygon', d.params);
    case 'closed':
      return t('arrange.label.closed', d.params);
    case 'line':
      return t('arrange.label.line', d.params);
    case 'open':
      return t('arrange.label.open', d.params);
    case 'compound':
      return t('arrange.label.compound', d.params);
    case 'text':
      return d.params.text ? t('arrange.label.text', d.params) : t('arrange.label.textEmpty');
    case 'image':
      return 'w' in d.params ? t('arrange.label.imageSized', d.params) : t('arrange.label.image');
    case 'group':
      return t('arrange.label.group', d.params);
    case 'empty':
      return t('arrange.label.empty');
  }
}

export interface Thumbnail {
  /** Tamaño del lienzo de la miniatura (el dibujo mide 100 en su dimensión mayor, más el margen). */
  width: number;
  height: number;
  /** Datos de trazado SVG ya con Y hacia abajo. */
  d: string;
}

/**
 * Miniatura del contorno de una unidad, o `null` si no es simple: imágenes, textos, formas con más
 * de 600 puntos o sin tamaño. La caja es la de la unidad completa (un grupo se dibuja entero).
 */
export function unitThumbnail(u: { objects: readonly DesignObject[] }): Thumbnail | null {
  if (u.objects.some((o) => o.kind.type !== 'vector')) return null;
  const paths = u.objects.flatMap((o) => objectPolylines(o));
  const count = paths.reduce((n, p) => n + p.points.length, 0);
  if (count === 0 || count > THUMB_MAX_POINTS) return null;
  const box = pointsBounds(paths.flatMap((p) => p.points));
  if (!box) return null;
  const [w, h] = [box[1][0] - box[0][0], box[1][1] - box[0][1]];
  const big = Math.max(w, h);
  if (!(big > 1e-9)) return null;
  const k = THUMB_SIZE / big;
  const d = paths
    .map((p) => {
      const pts = p.points.map(([x, y]) => `${((x - box[0][0]) * k + THUMB_PAD).toFixed(2)} ${((box[1][1] - y) * k + THUMB_PAD).toFixed(2)}`);
      return `M${pts.join('L')}${p.closed ? 'Z' : ''}`;
    })
    .join('');
  return { width: w * k + 2 * THUMB_PAD, height: h * k + 2 * THUMB_PAD, d };
}
