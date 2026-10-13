import { roundedRectBez, polylineBez } from '@/lib/bezpath';
import { objectPolylines, pointsBounds, unionBounds, type Bounds } from '@/lib/geometry';
import type { LaserApi } from '../api';
import type { DesignObject, GeometryOpArgs } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

const list = (ids: string[]) => ids.map((id) => `«${id}»`).join(', ');

/** Misma validación que el motor: sin imágenes, sin contornos vacíos y sin formas abiertas. */
function boundsOfOperands(objects: DesignObject[]): { id: string; layer: string; bounds: Bounds }[] {
  const images = objects.filter((o) => o.kind.type === 'image').map((o) => o.id);
  if (images.length) throw new Error(`Las imágenes no pueden participar en esta operación: ${list(images)}.`);
  const flat = objects.map((o) => ({ o, paths: objectPolylines(o) }));
  const open = flat.filter((f) => f.paths.some((p) => !p.closed)).map((f) => f.o.id);
  if (open.length) {
    throw new Error(`Hay formas abiertas que no pueden participar: ${list(open)}. Ciérralas antes (por ejemplo con «Cerrar polilínea»).`);
  }
  const empty = flat.filter((f) => f.paths.length === 0).map((f) => f.o.id);
  if (empty.length) throw new Error(`Estos objetos no tienen contorno que operar (¿un texto sin contorno generado?): ${list(empty)}.`);
  return flat.map((f) => ({ id: f.o.id, layer: f.o.layer, bounds: pointsBounds(f.paths.flatMap((p) => p.points))! }));
}

function rectObject(id: string, layer: string, [[x0, y0], [x1, y1]]: Bounds, radius = 0): DesignObject {
  const path = radius > 0 ? roundedRectBez(x0, y0, x1, y1, radius) : polylineBez([[x0, y0], [x1, y0], [x1, y1], [x0, y1]], true);
  return { id, layer, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [path] }, locked: false, hidden: false, group: null };
}

const area = (b: Bounds) => Math.max(0, b[1][0] - b[0][0]) * Math.max(0, b[1][1] - b[0][1]);

/** Intersección de cajas, o `null` si no se tocan. */
function intersect(a: Bounds, b: Bounds): Bounds | null {
  const r: Bounds = [[Math.max(a[0][0], b[0][0]), Math.max(a[0][1], b[0][1])], [Math.min(a[1][0], b[1][0]), Math.min(a[1][1], b[1][1])]];
  return area(r) > 0 ? r : null;
}

/** A menos B con cajas: si B cruza A de lado a lado, se recorta la parte cubierta; si no, A queda igual. */
function subtract(a: Bounds, b: Bounds): Bounds | null {
  const cut = intersect(a, b);
  if (!cut) return a;
  const coversY = cut[0][1] <= a[0][1] && cut[1][1] >= a[1][1];
  const coversX = cut[0][0] <= a[0][0] && cut[1][0] >= a[1][0];
  if (coversX && coversY) return null;
  if (coversY) return cut[0][0] <= a[0][0] ? [[cut[1][0], a[0][1]], a[1]] : cut[1][0] >= a[1][0] ? [a[0], [cut[0][0], a[1][1]]] : a;
  if (coversX) return cut[0][1] <= a[0][1] ? [[a[0][0], cut[1][1]], a[1]] : cut[1][1] >= a[1][1] ? [a[0], [a[1][0], cut[0][1]]] : a;
  return a;
}

/**
 * Resultado plausible sin el motor: trabaja con las cajas envolventes. Unión, soldar y xor dan la
 * caja unida; la intersección, la caja común; la diferencia, A (recortada si B la cruza de lado a
 * lado); el offset, cada caja crecida (con esquinas redondas si se pidió).
 */
function mockGeometryOp(args: GeometryOpArgs): DesignObject[] {
  const operands = boundsOfOperands(args.objects);
  const first = operands[0];
  if (!first) throw new Error('No hay objetos sobre los que operar.');
  const one = (b: Bounds | null, id = 'geo-1') => (b ? [rectObject(id, first.layer, b)] : []);
  switch (args.op) {
    case 'weld':
    case 'union':
    case 'xor':
      if (args.op !== 'weld' && operands.length < 2) throw new Error('Las booleanas necesitan al menos dos objetos.');
      return one(unionBounds(operands.map((o) => o.bounds)));
    case 'intersection':
    case 'difference': {
      if (operands.length < 2) throw new Error('Las booleanas necesitan al menos dos objetos.');
      const rest = unionBounds(operands.slice(1).map((o) => o.bounds))!;
      return one(args.op === 'intersection' ? intersect(first.bounds, rest) : subtract(first.bounds, rest));
    }
    case 'offset': {
      const d = args.distance;
      if (d === undefined || !Number.isFinite(d)) throw new Error('Falta la distancia del offset.');
      return operands.flatMap((o) => {
        const [[x0, y0], [x1, y1]] = o.bounds;
        const grown: Bounds = [[x0 - d, y0 - d], [x1 + d, y1 + d]];
        if (grown[1][0] - grown[0][0] <= 0 || grown[1][1] - grown[0][1] <= 0) return [];
        const round = (args.join ?? 'round') === 'round' && d > 0;
        return [{ ...rectObject(`${o.id}-off-1`, o.layer, grown, round ? d : 0) }];
      });
    }
    case 'join':
    case 'close':
    case 'dedupe': {
      // TODO(fase4): simulación de unir, cerrar y quitar duplicados.
      return args.objects.map((o) => structuredClone(o));
    }
  }
}

/** Métodos simulados del área `geometry`. Trabajan con cajas envolventes para poder desarrollar la interfaz sin el motor. */
export function mockGeometry(lag: Lag): Pick<LaserApi, 'geometryOp'> {
  return {
    geometryOp: (args) => lag(() => mockGeometryOp(args)),
  };
}
