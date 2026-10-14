import type { DesignObject } from '@/api/types';
import { boxesIntersect, getGeom, type Box } from './geom';
import { diffObjects } from './objectDiff';

/**
 * Rejilla de cajas envolventes de los objetos del documento, para que un clic, un hover o un
 * rectángulo pequeño no recorran los 100 000 objetos: solo se revisan los que están en las celdas
 * del punto (o del rectángulo). Dentro de cada objeto, `hit.ts` ya descarta por trozos de trazo.
 *
 * Los objetos que cubren demasiadas celdas (un trayecto del tamaño de la cama) van a una lista
 * aparte que se revisa siempre; son pocos. Al cambiar el documento se actualiza solo lo que cambió
 * (`update`), no se reconstruye.
 */

/** Por debajo de esta cantidad de objetos, recorrer todo es más barato que mantener la rejilla. */
export const INDEX_MIN_OBJECTS = 1500;
/** Un objeto que cubriría más celdas que esto va a la lista de "grandes". */
const MAX_CELLS_PER_OBJECT = 64;
/** Una consulta que tocaría más celdas que esto recorre todos los objetos. */
const MAX_CELLS_PER_QUERY = 4096;
const OFFSET = 1 << 19;
const STRIDE = 1 << 20;

const keyOf = (cx: number, cy: number): number => (cx + OFFSET) * STRIDE + (cy + OFFSET);

export class SpatialIndex {
  readonly cell: number;
  private cells = new Map<number, number[]>();
  private big = new Set<number>();
  /** Caja con la que se insertó cada posición (para quitarla); `null` si no tiene geometría. */
  private boxes: (Box | null)[] = [];
  private stamp: Uint32Array;
  private tick = 0;
  /** Cuántos objetos están en la lista de grandes (para pruebas y diagnóstico). */
  get bigCount(): number {
    return this.big.size;
  }

  constructor(objects: readonly DesignObject[]) {
    this.cell = chooseCell(objects);
    this.stamp = new Uint32Array(objects.length);
    for (let i = 0; i < objects.length; i++) this.insert(i, getGeom(objects[i]!).box);
  }

  private range(b: Box): [number, number, number, number] {
    const c = this.cell;
    return [Math.floor(b[0] / c), Math.floor(b[1] / c), Math.floor(b[2] / c), Math.floor(b[3] / c)];
  }

  private insert(i: number, b: Box | null): void {
    this.boxes[i] = b;
    if (!b) return;
    const [x0, y0, x1, y1] = this.range(b);
    if (!Number.isFinite(x0 + y0 + x1 + y1) || Math.abs(x0) >= OFFSET || Math.abs(x1) >= OFFSET || Math.abs(y0) >= OFFSET || Math.abs(y1) >= OFFSET || (x1 - x0 + 1) * (y1 - y0 + 1) > MAX_CELLS_PER_OBJECT) {
      this.big.add(i);
      return;
    }
    for (let cx = x0; cx <= x1; cx++) {
      for (let cy = y0; cy <= y1; cy++) {
        const k = keyOf(cx, cy);
        const list = this.cells.get(k);
        if (list) list.push(i);
        else this.cells.set(k, [i]);
      }
    }
  }

  private remove(i: number): void {
    const b = this.boxes[i];
    if (!b) return;
    if (this.big.delete(i)) return;
    const [x0, y0, x1, y1] = this.range(b);
    for (let cx = x0; cx <= x1; cx++) {
      for (let cy = y0; cy <= y1; cy++) {
        const k = keyOf(cx, cy);
        const list = this.cells.get(k);
        if (!list) continue;
        const at = list.indexOf(i);
        if (at >= 0) list.splice(at, 1);
        if (list.length === 0) this.cells.delete(k);
      }
    }
  }

  /** Pone al día las posiciones `changed` (y las nuevas al final) con los objetos de `next`. */
  update(next: readonly DesignObject[], changed: readonly number[]): void {
    if (next.length > this.stamp.length) {
      const s = new Uint32Array(next.length + (next.length >> 3));
      s.set(this.stamp);
      this.stamp = s;
    }
    for (const i of changed) {
      if (i < this.boxes.length) this.remove(i);
      this.insert(i, getGeom(next[i]!).box);
    }
  }

  /**
   * Posiciones (de mayor a menor: el objeto de más arriba primero) cuya caja, ensanchada `tol`, contiene
   * el punto.
   */
  atPoint(x: number, y: number, tol: number): number[] {
    return this.inBox([x - tol, y - tol, x + tol, y + tol], true);
  }

  /** Posiciones (de mayor a menor si `desc`, si no de menor a mayor) cuya caja toca `r`. */
  inBox(r: Box, desc = false): number[] {
    const out: number[] = [];
    const n = this.boxes.length;
    const [x0, y0, x1, y1] = this.range(r);
    const span = (x1 - x0 + 1) * (y1 - y0 + 1);
    if (!(span <= MAX_CELLS_PER_QUERY)) {
      for (let i = 0; i < n; i++) {
        const b = this.boxes[i];
        if (b && boxesIntersect(b, r)) out.push(i);
      }
    } else {
      const t = ++this.tick;
      const take = (i: number) => {
        if (this.stamp[i] === t) return;
        this.stamp[i] = t;
        const b = this.boxes[i];
        if (b && boxesIntersect(b, r)) out.push(i);
      };
      for (let cx = x0; cx <= x1; cx++) {
        for (let cy = y0; cy <= y1; cy++) {
          const list = this.cells.get(keyOf(cx, cy));
          if (list) for (const i of list) take(i);
        }
      }
      for (const i of this.big) take(i);
      out.sort(desc ? (a, b) => b - a : (a, b) => a - b);
    }
    if (!(span <= MAX_CELLS_PER_QUERY) && desc) out.reverse();
    return out;
  }
}

/** Celda de ~2 veces el tamaño típico de un objeto (muestra de hasta 512), entre 1 y 100 mm. */
function chooseCell(objects: readonly DesignObject[]): number {
  const step = Math.max(1, Math.floor(objects.length / 512));
  const sizes: number[] = [];
  for (let i = 0; i < objects.length; i += step) {
    const b = getGeom(objects[i]!).box;
    if (b) sizes.push(Math.max(b[2] - b[0], b[3] - b[1]));
  }
  if (sizes.length === 0) return 10;
  sizes.sort((a, b) => a - b);
  const typical = sizes[sizes.length >> 1]!;
  return Math.min(100, Math.max(1, typical * 2));
}

let current: { objects: readonly DesignObject[]; index: SpatialIndex } | null = null;

/** Índice de `objects`: el último, puesto al día con lo que cambió, o uno nuevo si cambió demasiado. */
export function spatialIndexFor(objects: readonly DesignObject[]): SpatialIndex {
  if (current && current.objects === objects) return current.index;
  if (current) {
    const changed = diffObjects(current.objects, objects, Math.max(64, objects.length >> 4));
    if (changed) {
      current.index.update(objects, changed);
      current.objects = objects;
      return current.index;
    }
  }
  const index = new SpatialIndex(objects);
  current = { objects, index };
  return index;
}

/** Solo para pruebas. */
export function resetSpatialIndex(): void {
  current = null;
}
