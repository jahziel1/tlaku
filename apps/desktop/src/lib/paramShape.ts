import type { BezPath, DesignObject, Document, Point } from '@/api/types';
import type { ShapeParams } from '@/bindings/ShapeParams';
import { bezBounds, ellipseBez, regularPolygonPoints, roundedPolygonBez, roundedRectBez, starPoints } from './bezpath';

/**
 * Formas paramétricas: rectángulo, elipse, polígono y estrella conservan sus parámetros en
 * `DesignObject.shape` (contrato del documento) mientras no se les editen los nodos.
 *
 * Convención del documento guardado:
 * - Los parámetros están en mm y la forma está centrada en el ORIGEN LOCAL del objeto: `kind.paths`
 *   contiene el contorno generado alrededor de (0, 0) y el `transform` lo coloca y lo gira.
 * - Cuando la matriz es solo giro + traslación (lo normal), los parámetros son las medidas reales.
 *   Si trae escala (documentos de otros orígenes), las medidas reales son los parámetros por la escala.
 * - `shape` solo se conserva si sigue describiendo la geometría: `reconcileShapes` lo pone en `null`
 *   cuando un cambio deja los contornos sin actualizar los parámetros (edición de nodos, unir, recortar...).
 *
 * Escala: al escalar un objeto con forma, la escala se hornea en los parámetros y en los contornos
 * (la matriz vuelve a ser giro + traslación). Una escala uniforme siempre se puede hornear. Una
 * escala no uniforme se hornea solo en rectángulos sin esquinas redondeadas y en elipses (cambian su
 * ancho y alto); en polígonos, estrellas y rectángulos redondeados el resultado ya no es una figura
 * regular (esquinas elípticas, polígono estirado), así que la forma deja de ser paramétrica
 * (`shape = null`) y el objeto se queda como trayecto. Lo mismo ocurre con una matriz sesgada.
 */

/** Parámetros de edición (nombres de la interfaz): los del contrato, con medidas más cómodas. */
export type ShapeSpec =
  | { type: 'rect'; w: number; h: number; radius: number }
  | { type: 'ellipse'; w: number; h: number }
  | { type: 'polygon'; sides: number; radius: number; rounding: number }
  | { type: 'star'; points: number; outer: number; ratio: number; rounding: number };

export type ShapeType = ShapeSpec['type'];

const EPS_SCALE = 1e-9;

/** Del contrato guardado a los parámetros de edición. */
export function paramsToSpec(p: ShapeParams): ShapeSpec {
  switch (p.type) {
    case 'rect':
      return { type: 'rect', w: p.width, h: p.height, radius: p.radius };
    case 'ellipse':
      return { type: 'ellipse', w: p.rx * 2, h: p.ry * 2 };
    case 'polygon':
      return { type: 'polygon', sides: p.sides, radius: p.radius, rounding: p.corner };
    case 'star':
      return { type: 'star', points: p.points, outer: p.outer, ratio: p.outer > 0 ? p.inner / p.outer : 0.5, rounding: p.corner };
  }
}

/** De los parámetros de edición al contrato que se guarda. */
export function specToParams(s: ShapeSpec): ShapeParams {
  switch (s.type) {
    case 'rect':
      return { type: 'rect', width: s.w, height: s.h, radius: s.radius };
    case 'ellipse':
      return { type: 'ellipse', rx: s.w / 2, ry: s.h / 2 };
    case 'polygon':
      return { type: 'polygon', sides: s.sides, radius: s.radius, corner: s.rounding };
    case 'star':
      return { type: 'star', points: s.points, outer: s.outer, inner: s.outer * s.ratio, corner: s.rounding };
  }
}

/** Contornos locales de la forma `spec` centrada en `center` (por defecto, el origen local). */
export function shapePaths(spec: ShapeSpec, center: Point = [0, 0]): BezPath[] {
  const [cx, cy] = center;
  switch (spec.type) {
    case 'rect':
      return [roundedRectBez(cx - spec.w / 2, cy - spec.h / 2, cx + spec.w / 2, cy + spec.h / 2, spec.radius)];
    case 'ellipse':
      return [ellipseBez(cx, cy, spec.w / 2, spec.h / 2)];
    case 'polygon':
      return [roundedPolygonBez(regularPolygonPoints(cx, cy, spec.radius, spec.sides), spec.rounding)];
    case 'star':
      return [roundedPolygonBez(starPoints(cx, cy, spec.outer, spec.outer * spec.ratio, spec.points), spec.rounding)];
  }
}

function finiteSpec(s: ShapeSpec): boolean {
  return Object.values(s).every((v) => typeof v === 'string' || (Number.isFinite(v) && v >= 0));
}

/** Caja de `a` ≈ caja de `b` (posición y tamaño), con tolerancia en mm. */
function sameBox(a: [Point, Point] | null, b: [Point, Point] | null): boolean {
  if (!a || !b) return false;
  const size = Math.max(b[1][0] - b[0][0], b[1][1] - b[0][1]);
  const tol = Math.max(1e-3, size * 1e-4);
  return a[0].every((v, i) => Math.abs(v - b[0][i]!) <= tol) && a[1].every((v, i) => Math.abs(v - b[1][i]!) <= tol);
}

// Un objeto es inmutable: el resultado de validar (misma `kind` y mismo `shape`) se recuerda.
const localCache = new WeakMap<object, { shape: ShapeParams; spec: ShapeSpec | null }>();

/**
 * Parámetros guardados (sin aplicar la escala de la matriz) si el objeto es un vector con `shape`
 * y los contornos siguen coincidiendo con él (misma caja alrededor del origen local); si no, `null`.
 */
export function storedShape(obj: DesignObject): ShapeSpec | null {
  const shape = obj.shape;
  if (!shape || obj.kind.type !== 'vector') return null;
  const hit = localCache.get(obj.kind);
  if (hit && hit.shape === shape) return hit.spec;
  const spec = paramsToSpec(shape);
  const ok = finiteSpec(spec) && sameBox(bezBounds(obj.kind.paths), bezBounds(shapePaths(spec)));
  localCache.set(obj.kind, { shape, spec: ok ? spec : null });
  return ok ? spec : null;
}

/** Escala por eje, ángulo y si hay reflexión; `skewed` si la matriz no es giro + escala. */
function decompose(m: DesignObject['transform']): { sx: number; sy: number; angle: number; skewed: boolean; flipped: boolean } {
  const sx = Math.hypot(m[0], m[1]);
  const sy = Math.hypot(m[2], m[3]);
  const skewed = Math.abs(m[0] * m[2] + m[1] * m[3]) > 1e-6 * Math.max(sx * sy, 1e-12);
  return { sx, sy, angle: Math.atan2(m[1], m[0]), skewed, flipped: m[0] * m[3] - m[1] * m[2] < 0 };
}

const isUnit = (s: number) => Math.abs(s - 1) < EPS_SCALE;
const uniform = (sx: number, sy: number) => Math.abs(sx - sy) <= 1e-6 * Math.max(sx, sy);

/** Aplica la escala por eje a los parámetros, o `null` si el resultado no es una forma de ese tipo. */
function scaleSpec(p: ShapeSpec, sx: number, sy: number): ShapeSpec | null {
  if (uniform(sx, sy)) {
    const k = (sx + sy) / 2;
    switch (p.type) {
      case 'rect':
        return { type: 'rect', w: p.w * k, h: p.h * k, radius: p.radius * k };
      case 'ellipse':
        return { type: 'ellipse', w: p.w * k, h: p.h * k };
      case 'polygon':
        return { type: 'polygon', sides: p.sides, radius: p.radius * k, rounding: p.rounding * k };
      case 'star':
        return { type: 'star', points: p.points, outer: p.outer * k, ratio: p.ratio, rounding: p.rounding * k };
    }
  }
  if (p.type === 'ellipse') return { type: 'ellipse', w: p.w * sx, h: p.h * sy };
  if (p.type === 'rect' && p.radius === 0) return { type: 'rect', w: p.w * sx, h: p.h * sy, radius: 0 };
  return null;
}

/**
 * Parámetros "reales" de la forma, en mm de documento (con la escala del objeto ya aplicada), o
 * `null` si no es paramétrica, sus contornos ya no coinciden con `shape`, o la escala/sesgo
 * impiden describirla con sus parámetros.
 */
export function effectiveShape(obj: DesignObject): ShapeSpec | null {
  const stored = storedShape(obj);
  if (!stored) return null;
  const { sx, sy, skewed } = decompose(obj.transform);
  if (skewed || sx < 1e-9 || sy < 1e-9) return null;
  return scaleSpec(stored, sx, sy);
}

/** ¿El objeto es hoy una forma paramétrica editable? */
export function isParametric(obj: DesignObject): boolean {
  return effectiveShape(obj) !== null;
}

/** Limita cada parámetro a valores que dan una forma válida. */
export function clampShape(p: ShapeSpec): ShapeSpec {
  const pos = (v: number) => (Number.isFinite(v) ? Math.max(0.01, v) : 0.01);
  const nonNeg = (v: number) => (Number.isFinite(v) ? Math.max(0, v) : 0);
  const count = (v: number) => Math.min(100, Math.max(3, Math.round(Number.isFinite(v) ? v : 3)));
  switch (p.type) {
    case 'rect': {
      const w = pos(p.w);
      const h = pos(p.h);
      return { type: 'rect', w, h, radius: Math.min(nonNeg(p.radius), w / 2, h / 2) };
    }
    case 'ellipse':
      return { type: 'ellipse', w: pos(p.w), h: pos(p.h) };
    case 'polygon':
      return { type: 'polygon', sides: count(p.sides), radius: pos(p.radius), rounding: nonNeg(p.rounding) };
    case 'star':
      return { type: 'star', points: count(p.points), outer: pos(p.outer), ratio: Math.min(0.95, Math.max(0.05, Number.isFinite(p.ratio) ? p.ratio : 0.5)), rounding: nonNeg(p.rounding) };
  }
}

/**
 * Objeto con la forma `spec` (medidas reales) regenerada alrededor del origen local: la matriz
 * conserva el origen (centro de la forma) y el ángulo, y pierde la escala. Con reflexión se usa
 * el giro equivalente (las formas son simétricas respecto al eje vertical local).
 */
function rebuild(obj: DesignObject, spec: ShapeSpec): DesignObject {
  const { angle, flipped } = decompose(obj.transform);
  const a = flipped ? angle + Math.PI : angle;
  const c = Math.cos(a);
  const s = Math.sin(a);
  return {
    ...obj,
    transform: [c, s, -s, c, obj.transform[4], obj.transform[5]],
    kind: { type: 'vector', paths: shapePaths(spec) },
    shape: specToParams(spec),
  };
}

/**
 * Aplica un cambio de parámetros (en mm de documento) a una forma paramétrica. La forma se
 * regenera centrada en su centro actual y conserva el ángulo; la escala del objeto se hornea en
 * los parámetros. `null` si no es editable.
 */
export function editShape(obj: DesignObject, patch: Partial<ShapeSpec>): DesignObject | null {
  const eff = effectiveShape(obj);
  if (!eff) return null;
  return rebuild(obj, clampShape({ ...eff, ...patch } as ShapeSpec));
}

/** Objeto nuevo con la forma `spec` centrada en `at` y girada `rotationRad`, con `shape` ya guardado. */
export function newShapeObject(id: string, layer: string, spec: ShapeSpec, at: Point, rotationRad = 0): DesignObject {
  const c = Math.cos(rotationRad);
  const s = Math.sin(rotationRad);
  return {
    id,
    layer,
    transform: [c, s, -s, c, at[0], at[1]],
    kind: { type: 'vector', paths: shapePaths(spec) },
    locked: false,
    hidden: false,
    group: null,
    shape: specToParams(spec),
  };
}

/** El mismo objeto sin parámetros de forma (pasa a ser un trayecto). */
export function withoutShape(obj: DesignObject): DesignObject {
  return obj.shape == null ? obj : { ...obj, shape: null };
}

/** Ajusta un objeto cambiado (`o`) para que `shape` siga siendo verdad; ver la descripción del módulo. */
function reconcileObject(prev: DesignObject | undefined, o: DesignObject): DesignObject {
  if (o.kind.type !== 'vector') return withoutShape(o);
  // Contornos nuevos con los mismos parámetros: alguien tocó la geometría sin pasar por la forma.
  if (prev && prev.kind !== o.kind && prev.shape === o.shape) return withoutShape(o);
  const stored = storedShape(o);
  if (!stored) return withoutShape(o);
  const { sx, sy, skewed } = decompose(o.transform);
  if (!skewed && isUnit(sx) && isUnit(sy)) return o;
  if (skewed || sx < 1e-9 || sy < 1e-9) return withoutShape(o);
  const scaled = scaleSpec(stored, sx, sy);
  return scaled ? rebuild(o, scaled) : withoutShape(o);
}

/**
 * Mantiene coherentes los parámetros de forma tras un cambio del documento: hornea la escala,
 * y quita `shape` de los objetos cuya geometría ya no coincide. Solo recorre lo que cambió;
 * devuelve `next` tal cual si no hay nada que ajustar.
 */
export function reconcileShapes(prev: Document, next: Document): Document {
  if (prev === next) return next;
  let prevById: Map<string, DesignObject> | null = null;
  let changed = false;
  const objects = next.objects.map((o, i) => {
    if (!o.shape) return o;
    let before: DesignObject | undefined = prev.objects[i];
    if (before?.id !== o.id) {
      prevById ??= new Map(prev.objects.map((x) => [x.id, x]));
      before = prevById.get(o.id);
    }
    if (before === o) return o;
    const fixed = reconcileObject(before, o);
    if (fixed !== o) changed = true;
    return fixed;
  });
  return changed ? { ...next, objects } : next;
}
