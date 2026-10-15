import { MoveClass } from '@/features/preview/model';
import { isLaserMove, layerOfMove, type RouteShape } from './route';

/**
 * Capas fuera de pantalla de la vista en vivo, en coordenadas del documento:
 *
 *  - `pending`: la ruta pendiente (cortes discontinuos, grabados tenues), pre-renderizada UNA vez;
 *  - `burned`: lo ya «quemado», que se acumula de forma incremental: cada cuadro solo traza los
 *    movimientos nuevos desde donde había llegado (no se redibuja todo).
 *
 * Cada cuadro de pantalla solo hace dos `drawImage` (más el cabezal), sin importar cuántos
 * movimientos tenga el trabajo. Las capas cubren la caja del trabajo, con un tope de píxeles por
 * lado (`MAX_SIDE`); el zoom y el paneo no las invalidan. Sin DOM: el lienzo lo crea `createCanvas`.
 */

export const MAX_SIDE = 2048;
export const MAX_SCALE = 64;
const MARGIN_MM = 1;
/** Movimientos por tanda al trazar (entre tanda y tanda se mira el reloj). */
const BATCH = 4096;

export interface Canvas2D {
  width: number;
  height: number;
  getContext(kind: '2d'): CanvasRenderingContext2D | null;
}
export type CanvasFactory = () => Canvas2D;

/** Mezcla `hex` (#rrggbb) con el tono tostado del material quemado; `k` = cuánto del tostado (0 a 1). */
export function charred(hex: string, k = 0.72): string {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex.trim());
  const [r, g, b] = m ? [parseInt(m[1]!, 16), parseInt(m[2]!, 16), parseInt(m[3]!, 16)] : [128, 128, 128];
  const burnt = [38, 24, 16];
  const mix = (c: number, i: number) => Math.round(c * (1 - k) + burnt[i]! * k);
  return `rgb(${mix(r, 0)}, ${mix(g, 1)}, ${mix(b, 2)})`;
}

export interface LayerGeometry {
  /** Esquina del documento (mm) que es el píxel (0, 0): la de más a la izquierda y arriba. */
  x0: number;
  y1: number;
  /** px por mm. */
  scale: number;
  width: number;
  height: number;
}

/** Escala y tamaño de las capas para una ruta. */
export function layerGeometry(route: RouteShape, maxSide = MAX_SIDE, wantScale = Infinity): LayerGeometry {
  const [[bx0, by0], [bx1, by1]] = route.bounds;
  const x0 = bx0 - MARGIN_MM;
  const y1 = by1 + MARGIN_MM;
  const wMm = bx1 - bx0 + 2 * MARGIN_MM;
  const hMm = y1 - (by0 - MARGIN_MM);
  const scale = Math.max(0.05, Math.min(MAX_SCALE, wantScale, maxSide / Math.max(wMm, hMm, 1e-6)));
  return { x0, y1, scale, width: Math.max(1, Math.ceil(wMm * scale)), height: Math.max(1, Math.ceil(hMm * scale)) };
}

export interface LayerStyle {
  /** Color neutro de los cortes/grabados cuando no se conocen las capas. */
  fallbackCut: string;
  fallbackEngrave: string;
  /** Color de los desplazamientos ya hechos (muy tenues). */
  travel: string;
}

export interface LayersOptions {
  /** Dibujar los desplazamientos ya hechos. */
  showTravel?: boolean;
  maxSide?: number;
  /**
   * Zoom de pantalla (px por mm) con el que se va a mirar: los grosores se piden en px de pantalla y
   * aquí se pasan a los de la capa (que es más grande), para que se vean igual de marcados.
   */
  zoom?: number;
  /** px de pantalla por mm que se quieren como mínimo en la capa (zoom × densidad de píxeles); el tope de tamaño manda. */
  wantScale?: number;
}

/** Grosores en px de pantalla al zoom de `LayersOptions.zoom`. */
const PEN_PX = { burnCut: 2.2, burnEngrave: 1.7, pendCut: 1.4, pendEngrave: 1, travel: 0.8, dash: [5, 4] } as const;

export class LiveLayers {
  readonly geo: LayerGeometry;
  readonly pending: Canvas2D;
  readonly burned: Canvas2D;
  private readonly pctx: CanvasRenderingContext2D;
  private readonly bctx: CanvasRenderingContext2D;
  /** Hasta dónde se trazó lo pendiente (movimiento). */
  private pendingAt = 0;
  /** Hasta dónde se trazó lo quemado. */
  private burnMove = 0;
  private burnFrac = 0;
  private showTravel: boolean;
  /** px de la capa por px de pantalla. */
  private readonly penK: number;
  private readonly zoomHint: number;

  constructor(
    private readonly route: RouteShape,
    private readonly style: LayerStyle,
    createCanvas: CanvasFactory,
    opts: LayersOptions = {},
  ) {
    this.geo = layerGeometry(route, opts.maxSide ?? MAX_SIDE, opts.wantScale);
    this.pending = createCanvas();
    this.burned = createCanvas();
    for (const c of [this.pending, this.burned]) {
      c.width = this.geo.width;
      c.height = this.geo.height;
    }
    const p = this.pending.getContext('2d');
    const b = this.burned.getContext('2d');
    if (!p || !b) throw new Error('canvas 2d');
    this.pctx = p;
    this.bctx = b;
    this.showTravel = opts.showTravel ?? false;
    this.zoomHint = opts.zoom ?? 0;
    this.penK = opts.zoom && opts.zoom > 0 ? Math.max(1, this.geo.scale / opts.zoom) : 1;
  }

  /** Lo pendiente ya está trazado entero. */
  get pendingDone(): boolean {
    return this.pendingAt >= this.route.count;
  }

  /** Zoom de pantalla para el que se armó (ver `LayersOptions.zoom`). */
  get builtZoom(): number {
    return this.zoomHint;
  }

  /** Hasta dónde llega lo quemado. */
  get burnedTo(): { move: number; frac: number } {
    return { move: this.burnMove, frac: this.burnFrac };
  }

  private px(x: number): number {
    return (x - this.geo.x0) * this.geo.scale;
  }
  private py(y: number): number {
    return (this.geo.y1 - y) * this.geo.scale;
  }

  private colorOf(k: number, burned: boolean): string {
    const layer = layerOfMove(this.route, k);
    const hex = layer >= 0 ? this.route.layerColors[layer] : undefined;
    const cut = this.route.model.cls[k] === MoveClass.Cut;
    if (hex) return burned ? charred(hex) : hex;
    return burned ? charred(cut ? this.style.fallbackCut : this.style.fallbackEngrave, 0.55) : cut ? this.style.fallbackCut : this.style.fallbackEngrave;
  }

  /** Traza lo pendiente por tandas hasta agotar `budgetMs`. Devuelve si terminó. */
  renderPending(budgetMs: number, now: () => number = () => performance.now()): boolean {
    const start = now();
    const ctx = this.pctx;
    ctx.lineCap = 'butt';
    ctx.lineJoin = 'round';
    while (this.pendingAt < this.route.count) {
      const to = Math.min(this.route.count, this.pendingAt + BATCH);
      this.strokeRange(ctx, this.pendingAt, 0, to, 0, 'pending');
      this.pendingAt = to;
      if (now() - start >= budgetMs) break;
    }
    return this.pendingDone;
  }

  /**
   * Traza lo quemado hasta `move`/`frac` (solo lo nuevo) por tandas hasta agotar `budgetMs`.
   * Devuelve si llegó. Si el destino quedó ATRÁS de lo ya trazado (la máquina retrocedió), borra
   * y vuelve a trazar desde el principio.
   */
  advance(move: number, frac: number, budgetMs: number, now: () => number = () => performance.now()): boolean {
    const behind = move < this.burnMove || (move === this.burnMove && frac < this.burnFrac - 1e-9);
    if (behind) this.resetBurn();
    const start = now();
    const ctx = this.bctx;
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    while (this.burnMove < move || (this.burnMove === move && this.burnFrac < frac)) {
      const toMove = Math.min(move, this.burnMove + BATCH);
      const toFrac = toMove === move ? frac : 0;
      this.strokeRange(ctx, this.burnMove, this.burnFrac, toMove, toFrac, 'burned');
      this.burnMove = toMove;
      this.burnFrac = toFrac;
      if (now() - start >= budgetMs && (this.burnMove < move || this.burnFrac < frac)) return false;
    }
    return true;
  }

  /** Borra lo quemado (para volver a trazarlo). */
  resetBurn(): void {
    this.bctx.clearRect(0, 0, this.geo.width, this.geo.height);
    this.burnMove = 0;
    this.burnFrac = 0;
  }

  setShowTravel(on: boolean): void {
    if (on !== this.showTravel) {
      this.showTravel = on;
      this.resetBurn();
    }
  }

  /**
   * Traza los movimientos desde `fromMove`/`fromFrac` hasta `toMove`/`toFrac`. Los movimientos
   * seguidos de la misma capa y clase van en un solo trazado.
   */
  private strokeRange(ctx: CanvasRenderingContext2D, fromMove: number, fromFrac: number, toMove: number, toFrac: number, kind: 'pending' | 'burned'): void {
    const { pos, cls } = this.route.model;
    const last = Math.min(toMove, this.route.count - 1);
    let open: { style: string; cut: boolean; travel: boolean } | null = null;
    let cx = NaN;
    let cy = NaN;
    const flush = () => {
      if (open) ctx.stroke();
      open = null;
    };
    for (let k = fromMove; k <= last; k++) {
      const a = k === fromMove ? fromFrac : 0;
      const b = k === toMove ? toFrac : 1;
      if (!(b > a)) continue;
      const travel = cls[k] === MoveClass.Travel || cls[k] === MoveClass.Overscan;
      // Lo que no se dibuja corta el trazo (el siguiente empieza con un `moveTo`) pero no cierra el trazado.
      if ((travel && (kind === 'pending' || !this.showTravel || cls[k] === MoveClass.Overscan)) || (!travel && !isLaserMove(this.route, k))) {
        cx = NaN;
        continue;
      }
      const cut = cls[k] === MoveClass.Cut;
      const style = travel ? this.style.travel : this.colorOf(k, kind === 'burned');
      if (!open || open.style !== style || open.cut !== cut || open.travel !== travel) {
        flush();
        ctx.beginPath();
        this.setPen(ctx, kind, cut, travel, style);
        open = { style, cut, travel };
        cx = NaN;
      }
      const i = 2 * k;
      const ax = pos[i]!;
      const ay = pos[i + 1]!;
      const dx = pos[i + 2]! - ax;
      const dy = pos[i + 3]! - ay;
      const sx = this.px(ax + dx * a);
      const sy = this.py(ay + dy * a);
      const ex = this.px(ax + dx * b);
      const ey = this.py(ay + dy * b);
      // Con `cx` en NaN (trazo nuevo o tras un movimiento no dibujado) siempre se empieza con `moveTo`: un `lineTo` sin punto previo no dibuja el primer tramo.
      if (!(Math.abs(sx - cx) <= 0.01 && Math.abs(sy - cy) <= 0.01)) ctx.moveTo(sx, sy);
      ctx.lineTo(ex, ey);
      cx = ex;
      cy = ey;
    }
    flush();
  }

  private setPen(ctx: CanvasRenderingContext2D, kind: 'pending' | 'burned', cut: boolean, travel: boolean, style: string): void {
    const k = this.penK;
    ctx.strokeStyle = style;
    if (travel) {
      ctx.lineWidth = PEN_PX.travel * k;
      ctx.globalAlpha = 0.28;
      ctx.setLineDash([]);
    } else if (kind === 'pending') {
      ctx.lineWidth = (cut ? PEN_PX.pendCut : PEN_PX.pendEngrave) * k;
      ctx.globalAlpha = cut ? 0.6 : 0.3;
      ctx.setLineDash(cut ? PEN_PX.dash.map((d) => d * k) : []);
    } else {
      ctx.lineWidth = (cut ? PEN_PX.burnCut : PEN_PX.burnEngrave) * k;
      ctx.globalAlpha = 1;
      ctx.setLineDash([]);
    }
  }

  /** Dibuja las capas sobre `ctx` (px de pantalla) con la vista `zoom`/`panX`/`panY`. */
  blit(ctx: CanvasRenderingContext2D, view: { zoom: number; panX: number; panY: number }): void {
    const { geo } = this;
    const k = view.zoom / geo.scale;
    const sx = view.panX + geo.x0 * view.zoom;
    const sy = view.panY - geo.y1 * view.zoom;
    const w = geo.width * k;
    const h = geo.height * k;
    ctx.imageSmoothingEnabled = true;
    ctx.drawImage(this.pending as unknown as CanvasImageSource, sx, sy, w, h);
    ctx.drawImage(this.burned as unknown as CanvasImageSource, sx, sy, w, h);
  }
}
