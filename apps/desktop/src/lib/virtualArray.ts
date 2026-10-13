import type { Affine, DesignObject, VirtualArray } from '@/api/types';
import { bezBounds, transformBez } from './bezpath';
import { objectBounds, objectCurves, unionBounds, type Bounds } from './geometry';

/** Tope de piezas de un arreglo virtual (el mismo que el del motor, `MAX_VIRTUAL_COPIES`). */
export const MAX_VIRTUAL_COPIES = 2000;

const translate = (dx: number, dy: number): Affine => [1, 0, 0, 1, dx, dy];

/** Giro antihorario de `deg` grados alrededor de (cx, cy). */
function rotateAbout(cx: number, cy: number, deg: number): Affine {
  const a = (deg * Math.PI) / 180;
  const c = Math.cos(a);
  const s = Math.sin(a);
  return [c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy];
}

/** Por qué un arreglo virtual no es válido, o `null` si lo es (mismas reglas que el motor). */
export function virtualArrayIssue(array: VirtualArray): 'counts' | 'tooMany' | 'numbers' | null {
  if (array.type === 'grid') {
    if (!Number.isInteger(array.cols) || !Number.isInteger(array.rows) || array.cols < 1 || array.rows < 1) return 'counts';
    if (!Number.isFinite(array.dx) || !Number.isFinite(array.dy)) return 'numbers';
    return array.cols * array.rows > MAX_VIRTUAL_COPIES ? 'tooMany' : null;
  }
  if (!Number.isInteger(array.count) || array.count < 1) return 'counts';
  if (![array.center[0], array.center[1], array.sweep_deg].every(Number.isFinite)) return 'numbers';
  return array.count > MAX_VIRTUAL_COPIES ? 'tooMany' : null;
}

/**
 * Transformaciones de las copias virtuales de un objeto, **sin** la original, en coordenadas de
 * documento: la copia se dibuja con `multiplyAffine(copia, obj.transform)`. Vacío si el objeto no
 * tiene arreglo, si es inválido o si no dibuja nada. Es la misma regla que `virtual_copies` del
 * motor (`crates/tlaku-pipeline/src/place.rs`), así que lo que se ve en el lienzo es lo que se corta.
 *
 * - `grid`: la original es la esquina inferior izquierda; columnas hacia la derecha y filas hacia
 *   arriba, separadas `dx` y `dy` mm entre bordes de la caja del objeto.
 * - `circular`: `count` piezas en total, contando la original. Desde 360° se reparte sin repetir la
 *   original; con menos, el arco incluye ambos extremos. Con `rotate` cada copia gira con el arreglo;
 *   sin él conserva la orientación y solo su centro recorre el arco.
 *
 * Para dibujar fantasmas en el lienzo: por cada matriz `m` de esta lista, dibujar las curvas del
 * objeto con `multiplyAffine(m, obj.transform)` (por ejemplo, el contorno en trazo tenue).
 */
export function virtualCopies(obj: DesignObject): Affine[] {
  const array = obj.array;
  if (!array || virtualArrayIssue(array) !== null) return [];
  const box = objectBounds(obj);
  if (!box) return [];
  if (array.type === 'grid') {
    const sx = box[1][0] - box[0][0] + array.dx;
    const sy = box[1][1] - box[0][1] + array.dy;
    const out: Affine[] = [];
    for (let j = 0; j < array.rows; j++) {
      for (let i = 0; i < array.cols; i++) {
        if (i !== 0 || j !== 0) out.push(translate(i * sx, j * sy));
      }
    }
    return out;
  }
  const n = array.count;
  const step = Math.abs(array.sweep_deg) >= 360 - 1e-9 ? array.sweep_deg / n : n > 1 ? array.sweep_deg / (n - 1) : 0;
  const [cx, cy] = array.center;
  const mx = (box[0][0] + box[1][0]) / 2;
  const my = (box[0][1] + box[1][1]) / 2;
  const out: Affine[] = [];
  for (let k = 1; k < n; k++) {
    const rot = rotateAbout(cx, cy, k * step);
    if (array.rotate) {
      out.push(rot);
    } else {
      const px = rot[0] * mx + rot[2] * my + rot[4];
      const py = rot[1] * mx + rot[3] * my + rot[5];
      out.push(translate(px - mx, py - my));
    }
  }
  return out;
}

/**
 * Caja del objeto y de sus copias virtuales (arreglo virtual): lo que ocupa en el trabajo. Es la
 * contraparte de `DesignObject::bounds_with_copies` del motor (`tlaku-doc`) y se usa para «ajustar
 * al diseño» y para el tamaño de la selección. Editar un objeto (barra numérica, alinear) sigue
 * usando `objectBounds`: las copias no se pueden mover por separado.
 */
export function objectBoundsWithCopies(o: DesignObject): Bounds | null {
  const own = objectBounds(o);
  if (!own || !o.array) return own;
  const curves = objectCurves(o);
  const boxes = virtualCopies(o).map((m) =>
    m[0] === 1 && m[1] === 0 && m[2] === 0 && m[3] === 1
      ? ([[own[0][0] + m[4], own[0][1] + m[5]], [own[1][0] + m[4], own[1][1] + m[5]]] as Bounds)
      : // Exacta, como en el motor: la afín de una Bézier es otra Bézier.
        bezBounds(curves.map((c) => transformBez(c, m))),
  );
  return unionBounds([own, ...boxes]);
}
