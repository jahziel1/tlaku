import { completedMoves, getModel, lowerBound, MoveClass, type PreviewData, type PreviewModel } from '@/features/preview/model';

/**
 * Ruta de la vista en vivo: el modelo de la vista previa (posiciones, tiempos acumulados por
 * movimiento y clase) más lo que hace falta para UBICAR en ella la posición real que reporta la
 * máquina. Es lo mismo que hace `machine/resume/plan.rs::locate` y `assess.rs::locate_forward` con
 * el G-code guardado para reanudar, pero sobre la ruta de la vista previa (que además trae
 * `move_times`), y por eso sirve también donde no hay G-code (Ruida, galvo).
 *
 * Sin DOM ni store: todo se prueba con datos.
 */

/** Cuánto puede alejarse la posición reportada de la ruta y seguir contando como «encima» (mm). Igual que `plan.rs::ON_PATH_MM`. */
export const ON_PATH_MM = 0.5;
/** Empates: entre movimientos a esta distancia de la mejor se elige el primero (adelantar falta, retroceder sobra). Igual que `plan.rs::TIE_MM`. */
export const TIE_MM = 0.02;
/** Hasta dónde se mira hacia delante de la última ubicación, en segundos de trabajo. */
export const HORIZON_S = 3;

export interface LiveRoute {
  model: PreviewModel;
  /** Movimientos de la vista previa tal cual (`[kind, x, y, power]`). */
  moves: ArrayLike<number>;
  count: number;
  /** Duración de la ruta (s); es el 100 %. */
  total: number;
  /** Colores `#rrggbb` de las capas (en el orden de la vista previa). */
  layerColors: string[];
  /** Cajas mínima y máxima de lo que se mueve (mm), con el punto de partida. */
  bounds: [[number, number], [number, number]];
}

/**
 * Arma la ruta a partir de un `JobPreview` (reusa el modelo en caché de la vista previa). `null` si no hay
 * movimientos. `start` (mm del documento) es donde estaba la cabeza al empezar el trabajo: el primer
 * desplazamiento sale de ahí y no del (0, 0) del documento (con el origen arriba a la derecha, por
 * ejemplo, la máquina arranca en la otra esquina).
 */
export function buildLiveRoute(data: PreviewData, rapidMmMin?: number, start?: readonly [number, number] | null): LiveRoute | null {
  let model = getModel(data, rapidMmMin);
  if (model.count === 0 || !(model.total > 0)) return null;
  if (start && Number.isFinite(start[0]) && Number.isFinite(start[1]) && (start[0] !== model.pos[0] || start[1] !== model.pos[1])) {
    // Copia propia de las posiciones: el modelo en caché es el de la vista previa y no se toca.
    const pos = model.pos.slice();
    pos[0] = start[0];
    pos[1] = start[1];
    model = { ...model, pos };
  }
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  for (let i = 0; i <= model.count; i++) {
    const x = model.pos[2 * i]!;
    const y = model.pos[2 * i + 1]!;
    if (x < x0) x0 = x;
    if (x > x1) x1 = x;
    if (y < y0) y0 = y;
    if (y > y1) y1 = y;
  }
  return {
    model,
    moves: data.moves,
    count: model.count,
    total: model.total,
    layerColors: data.layers.map((l) => l.color),
    bounds: [[x0, y0], [x1, y1]],
  };
}

/** Posición sobre la ruta: movimiento, fracción recorrida de él (0 a 1) y tiempo de trabajo (s). */
export interface RoutePos {
  move: number;
  frac: number;
  t: number;
}

/** Tiempo de trabajo (s) al estar en la fracción `frac` del movimiento `move`. */
export function timeAt(route: LiveRoute, move: number, frac: number): number {
  const times = route.model.times;
  if (move >= route.count) return route.total;
  const t0 = move > 0 ? times[move - 1]! : 0;
  return t0 + (times[move]! - t0) * Math.min(1, Math.max(0, frac));
}

/** Posición sobre la ruta al cumplirse `t` segundos de trabajo. */
export function posAtTime(route: LiveRoute, t: number): RoutePos {
  const n = route.count;
  const tt = Math.min(Math.max(t, 0), route.total);
  const k = completedMoves(route.model.times, tt);
  if (k >= n) return { move: n - 1, frac: 1, t: route.total };
  const t0 = k > 0 ? route.model.times[k - 1]! : 0;
  const t1 = route.model.times[k]!;
  return { move: k, frac: t1 > t0 ? (tt - t0) / (t1 - t0) : 1, t: tt };
}

/** Punto (mm) de la ruta en `move`/`frac`. */
export function pointAt(route: LiveRoute, move: number, frac: number): [number, number] {
  const p = route.model.pos;
  const k = Math.min(Math.max(move, 0), route.count - 1);
  const a = 2 * k;
  return [p[a]! + (p[a + 2]! - p[a]!) * frac, p[a + 1]! + (p[a + 3]! - p[a + 1]!) * frac];
}

/** Capa (índice en la vista previa) del movimiento `k`, o -1 si no se conocen los límites de las capas. */
export function layerOfMove(route: LiveRoute, k: number): number {
  const s = route.model.layerStarts;
  if (!s || route.model.approximate) return -1;
  let lo = 0;
  let hi = s.length - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >>> 1;
    if (s[mid]! <= k) lo = mid;
    else hi = mid;
  }
  return lo;
}

/** El movimiento es de láser con potencia (corte o grabado), no un desplazamiento ni un overscan. */
export function isLaserMove(route: LiveRoute, k: number): boolean {
  const c = route.model.cls[k];
  return c === MoveClass.Cut || c === MoveClass.Engrave;
}

/** Distancia de `p` al segmento `a`-`b` y fracción del punto más cercano, con la fracción mínima `minT`. */
function project(px: number, py: number, ax: number, ay: number, bx: number, by: number, minT: number): [number, number] {
  const dx = bx - ax;
  const dy = by - ay;
  const len2 = dx * dx + dy * dy;
  let t = len2 <= 1e-18 ? 0 : ((px - ax) * dx + (py - ay) * dy) / len2;
  t = Math.min(1, Math.max(minT, t));
  return [Math.hypot(px - (ax + t * dx), py - (ay + t * dy)), t];
}

export interface Located extends RoutePos {
  /** Distancia de la posición reportada a la ruta (mm). */
  dist: number;
}

export interface LocateOptions {
  tolMm?: number;
  /** Segundos de trabajo que se miran hacia delante de `from`. */
  horizonS?: number;
  /**
   * Fracción 0..1 del trabajo según el progreso del streamer (líneas con `ok`). Solo desempata
   * cuando varios sitios de la ruta pasan por el mismo punto y no hay una ubicación previa.
   */
  prior?: number | null;
}

/**
 * Ubica `p` (mm del documento) en la ruta. Con `from` busca SIN RETROCEDER: del movimiento donde
 * estaba y hasta `horizonS` segundos de trabajo más adelante, y en ese mismo movimiento solo de la
 * fracción en que estaba en adelante. Sin `from` busca en toda la ruta. Si varios sitios quedan a
 * la misma distancia (un trazo que vuelve a pasar por el mismo punto) elige el primero que no
 * quede atrás; sin `from`, el más cercano al avance que dice el streamer (`prior`).
 * `null` si ningún sitio queda a menos de `tolMm`.
 */
export function locate(route: LiveRoute, p: readonly [number, number], from: RoutePos | null, opts: LocateOptions = {}): Located | null {
  const tol = opts.tolMm ?? ON_PATH_MM;
  const n = route.count;
  const pos = route.model.pos;
  const k0 = from ? Math.min(from.move, n - 1) : 0;
  const k1 = from ? Math.min(n, lowerBound(route.model.times, from.t + (opts.horizonS ?? HORIZON_S)) + 1) : n;
  const minT = (k: number) => (from && k === k0 ? from.frac : 0);
  const at = (k: number): [number, number] => {
    const a = 2 * k;
    return project(p[0], p[1], pos[a]!, pos[a + 1]!, pos[a + 2]!, pos[a + 3]!, minT(k));
  };
  let best = Infinity;
  for (let k = k0; k < k1; k++) {
    const d = at(k)[0];
    if (d < best) best = d;
  }
  if (!(best <= tol)) return null;

  let pick = -1;
  if (!from && opts.prior != null && Number.isFinite(opts.prior)) {
    // Sin ubicación previa: entre los empatados, el más cercano al avance del streamer.
    const want = Math.min(1, Math.max(0, opts.prior)) * n;
    let gap = Infinity;
    for (let k = k0; k < k1; k++) {
      if (at(k)[0] > best + TIE_MM) continue;
      const g = Math.abs(k + 0.5 - want);
      if (g < gap) {
        gap = g;
        pick = k;
      }
    }
  } else {
    for (let k = k0; k < k1; k++) {
      if (at(k)[0] <= best + TIE_MM) {
        pick = k;
        break;
      }
    }
  }
  if (pick < 0) return null;
  const [dist, frac] = at(pick);
  const t = timeAt(route, pick, frac);
  return { move: pick, frac, t: from ? Math.max(t, from.t) : t, dist };
}

// ---------------------------------------------------------------------------
// Seguimiento con estado
// ---------------------------------------------------------------------------

export type LocatorStatus = 'tracking' | 'held' | 'resync' | 'lost';

export interface LocatorResult {
  status: LocatorStatus;
  /** Última ubicación buena (null si todavía no se encontró ninguna). */
  pos: RoutePos | null;
  /** La máquina retrocedió de verdad: la nueva ubicación es anterior a la que se tenía. */
  retreat: boolean;
}

/** Reportes seguidos sin encontrar la posición cerca de la ruta antes de buscar en toda ella. */
export const MISSES_BEFORE_RESYNC = 3;
/** Entre dos búsquedas en toda la ruta (ms): con rutas enormes cada una cuesta. */
export const RESYNC_EVERY_MS = 1000;
/** Retroceso (s de trabajo) a partir del cual una ubicación más atrás cuenta como retroceso real. */
export const RETREAT_S = 1;

/**
 * Sigue la posición de la máquina sobre la ruta reporte a reporte. Hacia delante, con una ventana
 * corta (barato aunque el trabajo tenga cientos de miles de movimientos); si la posición deja de
 * encajar varias veces seguidas (la máquina saltó, se reinició el trabajo, se activó la vista a
 * mitad) busca en toda la ruta, y ahí sí puede dar una ubicación anterior (`retreat`).
 */
export class RouteLocator {
  private last: RoutePos | null = null;
  private lastWall = 0;
  private misses = 0;
  private lastGlobal = -Infinity;

  constructor(private readonly route: LiveRoute) {}

  get pos(): RoutePos | null {
    return this.last;
  }

  /** `wallMs`: reloj en ms; `prior`: fracción 0..1 del streamer, si se conoce. */
  update(p: readonly [number, number], wallMs: number, prior: number | null = null): LocatorResult {
    const route = this.route;
    if (!this.last) {
      const first = locate(route, p, null, { prior });
      this.lastGlobal = wallMs;
      if (!first) return { status: 'lost', pos: null, retreat: false };
      this.accept(first, wallMs);
      return { status: 'resync', pos: this.last, retreat: false };
    }
    // Tras un rato sin reportes la cabeza pudo avanzar más: la ventana crece con el tiempo.
    const idleS = Math.max(0, (wallMs - this.lastWall) / 1000);
    const horizonS = Math.max(HORIZON_S, idleS * 6);
    const near = locate(route, p, this.last, { horizonS, prior });
    if (near) {
      this.accept(near, wallMs);
      return { status: 'tracking', pos: this.last, retreat: false };
    }
    this.misses++;
    if (this.misses >= MISSES_BEFORE_RESYNC && wallMs - this.lastGlobal >= RESYNC_EVERY_MS) {
      this.lastGlobal = wallMs;
      const global = locate(route, p, null, { prior });
      if (global) {
        const retreat = global.t < this.last.t - RETREAT_S;
        this.last = null;
        this.accept(global, wallMs);
        return { status: 'resync', pos: this.last, retreat };
      }
    }
    return { status: this.misses >= MISSES_BEFORE_RESYNC ? 'lost' : 'held', pos: this.last, retreat: false };
  }

  private accept(l: Located, wallMs: number): void {
    this.last = { move: l.move, frac: l.frac, t: l.t };
    this.lastWall = wallMs;
    this.misses = 0;
  }
}
