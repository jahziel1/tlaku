import type { BezPath, DesignObject, Point } from '@/api/types';
import { ellipseBez, regularPolygonPoints, roundedPolygonBez, roundedRectBez, starPoints } from '@/lib/bezpath';
import { applyAffine } from '@/lib/geometry';

/**
 * Formas paramétricas: rectángulo, elipse, polígono y estrella siguen siendo "formas" (con sus
 * parámetros editables) mientras no se les editen los nodos. El documento solo guarda los
 * contornos (el contrato no tiene campo para los parámetros), así que los parámetros viven aquí,
 * ligados al objeto `kind` del contorno con un `WeakMap`: mover, escalar, girar, duplicar, copiar y
 * deshacer conservan la misma referencia y por eso conservan los parámetros; editar nodos, unir,
 * recortar o cualquier otra operación crea un `kind` nuevo y la forma deja de ser paramétrica.
 * No se guardan en el archivo: al reabrir el proyecto las formas pasan a ser trayectos.
 */
export type ShapeParams =
  | { type: 'rect'; w: number; h: number; radius: number }
  | { type: 'ellipse'; w: number; h: number }
  | { type: 'polygon'; sides: number; radius: number; rounding: number }
  | { type: 'star'; points: number; outer: number; ratio: number; rounding: number };

export type ShapeType = ShapeParams['type'];

export interface ShapeInfo {
  params: ShapeParams;
  /** Centro de la forma en coordenadas locales del objeto. */
  center: Point;
}

const registry = new WeakMap<object, ShapeInfo>();

/** Contornos locales de la forma `params` centrada en `center`. */
export function shapePaths(params: ShapeParams, center: Point = [0, 0]): BezPath[] {
  const [cx, cy] = center;
  switch (params.type) {
    case 'rect':
      return [roundedRectBez(cx - params.w / 2, cy - params.h / 2, cx + params.w / 2, cy + params.h / 2, params.radius)];
    case 'ellipse':
      return [ellipseBez(cx, cy, params.w / 2, params.h / 2)];
    case 'polygon':
      return [roundedPolygonBez(regularPolygonPoints(cx, cy, params.radius, params.sides), params.rounding)];
    case 'star':
      return [roundedPolygonBez(starPoints(cx, cy, params.outer, params.outer * params.ratio, params.points), params.rounding)];
  }
}

/** Anota que `obj` es una forma paramétrica (se ata a su contorno, no al objeto). */
export function registerShape(obj: DesignObject, info: ShapeInfo): void {
  if (obj.kind.type === 'vector') registry.set(obj.kind, info);
}

export function shapeOf(obj: DesignObject): ShapeInfo | null {
  return obj.kind.type === 'vector' ? (registry.get(obj.kind) ?? null) : null;
}

/** Objeto nuevo con la forma `params` centrada en `at` y girada `rotationRad`. Ya viene registrado. */
export function newShapeObject(id: string, layer: string, params: ShapeParams, at: Point, rotationRad = 0): DesignObject {
  const c = Math.cos(rotationRad);
  const s = Math.sin(rotationRad);
  const obj: DesignObject = {
    id,
    layer,
    transform: [c, s, -s, c, at[0], at[1]],
    kind: { type: 'vector', paths: shapePaths(params) },
    locked: false,
    hidden: false,
    group: null,
  };
  registerShape(obj, { params, center: [0, 0] });
  return obj;
}

/** Factores de escala por eje y ángulo del objeto; `skewed` si la matriz no es giro + escala. */
function decompose(m: DesignObject['transform']): { sx: number; sy: number; angle: number; skewed: boolean } {
  const sx = Math.hypot(m[0], m[1]);
  const sy = Math.hypot(m[2], m[3]);
  const skewed = Math.abs(m[0] * m[2] + m[1] * m[3]) > 1e-6 * Math.max(sx * sy, 1e-12);
  return { sx, sy, angle: Math.atan2(m[1], m[0]), skewed };
}

/**
 * Parámetros "reales" de la forma, en mm de documento (con la escala del objeto ya aplicada), o
 * `null` si no es paramétrica o está sesgada y ya no se puede describir con sus parámetros.
 */
export function effectiveShape(obj: DesignObject): ShapeParams | null {
  const info = shapeOf(obj);
  if (!info) return null;
  const { sx, sy, skewed } = decompose(obj.transform);
  if (skewed || sx < 1e-9 || sy < 1e-9) return null;
  const k = (sx + sy) / 2;
  const p = info.params;
  switch (p.type) {
    case 'rect':
      return { type: 'rect', w: p.w * sx, h: p.h * sy, radius: p.radius * k };
    case 'ellipse':
      return { type: 'ellipse', w: p.w * sx, h: p.h * sy };
    case 'polygon':
      return { type: 'polygon', sides: p.sides, radius: p.radius * k, rounding: p.rounding * k };
    case 'star':
      return { type: 'star', points: p.points, outer: p.outer * k, ratio: p.ratio, rounding: p.rounding * k };
  }
}

/** Limita cada parámetro a valores que dan una forma válida. */
export function clampShape(p: ShapeParams): ShapeParams {
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
 * Aplica un cambio de parámetros (en mm de documento) a una forma paramétrica. La forma se
 * regenera centrada en su centro actual y conserva el ángulo; la escala del objeto se hornea en
 * los parámetros (la matriz queda solo con giro y traslación). `null` si no es editable.
 */
export function editShape(obj: DesignObject, patch: Partial<ShapeParams>): DesignObject | null {
  const info = shapeOf(obj);
  const eff = effectiveShape(obj);
  if (!info || !eff) return null;
  const next = clampShape({ ...eff, ...patch } as ShapeParams);
  const [cx, cy] = applyAffine(obj.transform, info.center);
  const { angle } = decompose(obj.transform);
  const c = Math.cos(angle);
  const s = Math.sin(angle);
  const edited: DesignObject = {
    ...obj,
    transform: [c, s, -s, c, cx, cy],
    kind: { type: 'vector', paths: shapePaths(next) },
  };
  registerShape(edited, { params: next, center: [0, 0] });
  return edited;
}
