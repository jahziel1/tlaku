import { CLASS_COUNT, MoveClass, POWER_LEVELS, type PreviewModel } from './model';

/**
 * Caché de trazados por bloques. Los trazados se construyen en coordenadas del documento (mm),
 * así que sobreviven al zoom y al paneo: el dibujo solo cambia la transformación del contexto.
 * Cada bloque agrupa los segmentos por (clase, nivel de potencia) para dibujar con un solo
 * `stroke` por combinación en lugar de uno por segmento.
 */

/** Movimientos por bloque. Un bloque a medias se reconstruye en cada cuadro, por eso es chico. */
export const BLOCK_SIZE = 4096;
export const SLOT_COUNT = CLASS_COUNT * POWER_LEVELS;

/** Lo mínimo que se usa de `Path2D`; permite probar la caché sin DOM. */
export interface PathLike {
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
}

export type PathFactory<P extends PathLike> = () => P;

export interface BlockPaths<P extends PathLike> {
  /** Un trazado por (clase * POWER_LEVELS + nivel); null si el bloque no tiene de ese tipo. */
  paths: (P | null)[];
  /** [minX, minY, maxX, maxY] de todos los segmentos del bloque (siempre los reales, sin simplificar). */
  bbox: [number, number, number, number];
  /** Segmentos que se dibujan con este nivel de detalle (cuesta lo mismo que el `stroke`). */
  segs: number;
}

/**
 * Niveles de detalle: tolerancia (mm) por debajo de la cual se omiten vértices. El nivel 0 es el
 * recorrido exacto; cada nivel siguiente duplica la tolerancia. Con el zoom bajo, millones de
 * segmentos menores que un píxel se funden en unos pocos sin que se note.
 */
export const LOD_TOLERANCES_MM: readonly number[] = [0, 0.02, 0.04, 0.08, 0.16, 0.32, 0.64, 1.28, 2.56, 5.12, 10.24];
export const MAX_LOD = LOD_TOLERANCES_MM.length - 1;
/**
 * Los niveles `MAX_LOD + 1 ...` repiten las tolerancias pero además funden los niveles de potencia
 * de corte y grabado en uno solo: con trazos más chicos que un píxel (una imagen a vista de
 * pájaro) la potencia ya no se distingue y cambiar de trazado en cada tramo costaría lo mismo
 * que dibujarlo todo.
 */
export const COLLAPSED_POWER_LEVEL = 2;

export interface LodSettings {
  minStep: number;
  collapsePower: boolean;
}

export function lodSettings(level: number): LodSettings {
  const collapsePower = level > MAX_LOD;
  return { minStep: LOD_TOLERANCES_MM[collapsePower ? level - MAX_LOD - 1 : level] ?? 0, collapsePower };
}

export function slotOf(cls: number, level: number): number {
  return cls * POWER_LEVELS + level;
}

/**
 * Construye los trazados de los movimientos [from, to). Con `tail`, además el movimiento `to`
 * dibujado solo hasta ese punto (el tramo recorrido del movimiento en curso).
 */
export function buildPaths<P extends PathLike>(
  model: Pick<PreviewModel, 'pos' | 'cls' | 'level' | 'count'>,
  from: number,
  to: number,
  factory: PathFactory<P>,
  tail?: { x: number; y: number },
  lod: LodSettings = { minStep: 0, collapsePower: false },
): BlockPaths<P> {
  const paths: (P | null)[] = new Array<P | null>(SLOT_COUNT).fill(null);
  // Último punto real de cada trazado: si el siguiente segmento empieza ahí no hace falta un `moveTo`.
  const curX = new Float64Array(SLOT_COUNT).fill(NaN);
  const curY = new Float64Array(SLOT_COUNT).fill(NaN);
  // Simplificación: último vértice dibujado y vértice omitido que falta por dibujar al cerrar la línea.
  const emitX = new Float64Array(SLOT_COUNT);
  const emitY = new Float64Array(SLOT_COUNT);
  const pending = new Uint8Array(SLOT_COUNT);
  const min2 = lod.minStep * lod.minStep;
  let segs = 0;
  const flush = (slot: number) => {
    if (pending[slot]) {
      paths[slot]!.lineTo(curX[slot]!, curY[slot]!);
      segs++;
      pending[slot] = 0;
    }
  };
  const bbox: [number, number, number, number] = [Infinity, Infinity, -Infinity, -Infinity];
  const last = tail && to < model.count ? to : to - 1;
  for (let i = from; i <= last; i++) {
    const sx = model.pos[2 * i]!;
    const sy = model.pos[2 * i + 1]!;
    const isTail = tail !== undefined && i === to;
    const ex = isTail ? tail.x : model.pos[2 * i + 2]!;
    const ey = isTail ? tail.y : model.pos[2 * i + 3]!;
    const cls = model.cls[i]!;
    const slot = slotOf(cls, lod.collapsePower && (cls === MoveClass.Cut || cls === MoveClass.Engrave) ? COLLAPSED_POWER_LEVEL : model.level[i]!);
    let p = paths[slot];
    if (!p) {
      p = factory();
      paths[slot] = p;
    }
    if (curX[slot] !== sx || curY[slot] !== sy) {
      flush(slot);
      p.moveTo(sx, sy);
      emitX[slot] = sx;
      emitY[slot] = sy;
    }
    curX[slot] = ex;
    curY[slot] = ey;
    const dx = ex - emitX[slot]!;
    const dy = ey - emitY[slot]!;
    if (dx * dx + dy * dy < min2) {
      pending[slot] = 1; // vértice a menos de la tolerancia del último dibujado: se omite
    } else {
      p.lineTo(ex, ey);
      segs++;
      emitX[slot] = ex;
      emitY[slot] = ey;
      pending[slot] = 0;
    }
    if (sx < bbox[0]) bbox[0] = sx;
    if (ex < bbox[0]) bbox[0] = ex;
    if (sy < bbox[1]) bbox[1] = sy;
    if (ey < bbox[1]) bbox[1] = ey;
    if (sx > bbox[2]) bbox[2] = sx;
    if (ex > bbox[2]) bbox[2] = ex;
    if (sy > bbox[3]) bbox[3] = sy;
    if (ey > bbox[3]) bbox[3] = ey;
  }
  for (let slot = 0; slot < SLOT_COUNT; slot++) flush(slot);
  return { paths, bbox, segs };
}

/** Caché perezosa de los bloques completos de un modelo, por nivel de detalle. */
export class BlockCache<P extends PathLike> {
  private levels: (BlockPaths<P> | undefined)[][] = [];
  /** Cuántos bloques se han construido en total (para pruebas y para saber si falta precalentar). */
  built = 0;

  constructor(
    private model: Pick<PreviewModel, 'pos' | 'cls' | 'level' | 'count'>,
    private factory: PathFactory<P>,
    readonly blockSize: number = BLOCK_SIZE,
  ) {}

  get blockCount(): number {
    return Math.ceil(this.model.count / this.blockSize);
  }

  has(b: number, level = 0): boolean {
    return this.levels[level]?.[b] !== undefined;
  }

  /** Bloque `b` con el nivel de detalle `level`, construyéndolo la primera vez. */
  get(b: number, level = 0): BlockPaths<P> {
    const row = (this.levels[level] ??= new Array<BlockPaths<P> | undefined>(this.blockCount));
    let hit = row[b];
    if (!hit) {
      const from = b * this.blockSize;
      const to = Math.min(this.model.count, from + this.blockSize);
      hit = buildPaths(this.model, from, to, this.factory, undefined, lodSettings(level));
      row[b] = hit;
      this.built++;
    }
    return hit;
  }

  /**
   * Construye bloques pendientes del nivel `level` durante `budgetMs` milisegundos. Devuelve true
   * si ya están todos. Se llama en ratos libres para que el primer dibujo con el recorrido
   * completo no se trabe.
   */
  warm(budgetMs: number, level = 0, now: () => number = () => performance.now()): boolean {
    const end = now() + budgetMs;
    for (let b = 0; b < this.blockCount; b++) {
      if (this.has(b, level)) continue;
      this.get(b, level);
      if (now() >= end) return this.levels[level]!.filter(Boolean).length === this.blockCount;
    }
    return true;
  }
}
