import type { Affine, DesignObject, DeviceOrigin, Point, TextObject } from '@/api/types';
import { topGroup } from '@/lib/groups';
import { textFont, textLines } from '@/lib/textobj';
import { gridSteps, worldToScreen, type ViewTransform } from '@/lib/viewport';
import { boxesIntersect, fillPath, getGeom, visibleBox, type Box, type ObjectGeom } from './geom';
import { backdropKeyOf, paintBackdrop, sameBackdropKey, type BackdropKey } from './backdrop';
import { frameRect, handleSpots } from './handles';
import { bezToPath2D } from '@/lib/maskClip';
import { ImageCache } from './imageSource';
import { drawMarks } from './marksDraw';
import { drawTiles } from '@/features/tiling/draw';
import { drawVirtualCopies } from './virtualDraw';
import { drawPreview } from './previewLayer';
import type { CanvasPalette, Renderer, Scene } from './renderer';
import { boundsOf, keepsAxes, canRotate as selectionCanRotate } from './transform';
import { drawDraft, drawGuides, drawHead, drawMarquee, drawMeasure, drawNodes, drawPreviewBounds, drawSnapMark, strokeGeom, worldTransform } from './overlay2d';

/** Ancho de línea de los objetos en px de pantalla (constante: no depende del zoom). */
const STROKE_PX = 1.4;
const HOVER_PX = 2.6;
const DIM_ALPHA = 0.22;
/** Por encima de este número de segmentos visibles, el diseño se pinta en un bitmap que se reutiliza al hacer zoom o paneo. */
export const HEAVY_SEGMENTS = 12_000;
/** Margen extra del bitmap (fracción del lado mayor) para que el paneo no muestre bordes vacíos. */
const CACHE_MARGIN = 0.2;
/** Tiempo sin mover la vista tras el cual se repinta nítido (ms). */
const SETTLE_MS = 120;

interface CacheKey {
  objects: DesignObject[];
  layers: Scene['layers'];
  wireframe: boolean;
  dim: boolean;
  w: number;
  h: number;
  dpr: number;
  images: number;
}

interface ObjectBitmap {
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D;
  margin: number;
  /** Vista con la que se pintó (sin el margen). */
  view: ViewTransform;
  key: CacheKey;
}

function keyEquals(a: CacheKey, b: CacheKey): boolean {
  return a.objects === b.objects && a.layers === b.layers && a.wireframe === b.wireframe && a.dim === b.dim &&
    a.w === b.w && a.h === b.h && a.dpr === b.dpr && a.images === b.images;
}

/** Renderer de referencia en Canvas 2D: cama, cuadrícula, objetos, selección y vista previa. */
export class Canvas2DRenderer implements Renderer {
  readonly kind = 'canvas2d' as const;
  private canvas: HTMLCanvasElement | null = null;
  private ctx: CanvasRenderingContext2D | null = null;
  private w = 0;
  private h = 0;
  private dpr = 1;
  private invalidate: () => void = () => {};
  private imageVersion = 0;
  private images = new ImageCache(() => {
    this.imageVersion++;
    this.invalidate();
  });
  private bitmap: ObjectBitmap | null = null;
  /** Imagen de la cámara ya proyectada (a opacidad plena); solo existe mientras hay cámara. */
  private backdropLayer: { canvas: HTMLCanvasElement; ctx: CanvasRenderingContext2D; key: BackdropKey | null } | null = null;
  private settleTimer = 0;
  private settleNow = false;

  setInvalidate(cb: () => void): void {
    this.invalidate = cb;
  }

  attach(canvas: HTMLCanvasElement): void {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
  }

  resize(width: number, height: number, dpr: number): void {
    this.w = width;
    this.h = height;
    this.dpr = dpr;
    if (!this.canvas) return;
    this.canvas.width = Math.max(1, Math.round(width * dpr));
    this.canvas.height = Math.max(1, Math.round(height * dpr));
  }

  dispose(): void {
    if (this.settleTimer) clearTimeout(this.settleTimer);
    this.canvas = null;
    this.ctx = null;
    this.bitmap = null;
    this.backdropLayer = null;
  }

  render(scene: Scene): void {
    const ctx = this.ctx;
    if (!ctx || this.w === 0) return;
    const { palette: p, view } = scene;
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    ctx.fillStyle = p.workspace;
    ctx.fillRect(0, 0, this.w, this.h);

    this.drawBed(ctx, scene);
    const selected = scene.objects.filter((o) => scene.selectedIds.has(o.id));
    this.drawObjects(ctx, scene);
    drawVirtualCopies(ctx, scene, this.dpr);
    if (scene.preview) {
      drawPreview(ctx, scene);
      drawPreviewBounds(ctx, scene, this.dpr);
    } else {
      this.drawHover(ctx, scene, selected);
      this.drawSelection(ctx, scene, selected);
      this.drawPrimary(ctx, scene);
      drawNodes(ctx, scene, this.dpr);
    }
    drawMeasure(ctx, scene, this.dpr);
    drawDraft(ctx, scene, this.dpr);
    drawGuides(ctx, scene, this.dpr);
    drawMarks(ctx, scene, this.dpr);
    drawTiles(ctx, scene, this.dpr);
    drawSnapMark(ctx, scene, this.dpr);
    drawMarquee(ctx, scene, this.dpr);
    this.drawOrigin(ctx, view, scene.bed, p);
    drawHead(ctx, scene, this.dpr);
  }

  // ---- cama y cuadrícula -------------------------------------------------

  private drawBed(ctx: CanvasRenderingContext2D, scene: Scene): void {
    const { view, bed, palette: p, width, height } = scene;
    const [x0, y0] = worldToScreen(view, [0, 0]);
    const [x1, y1] = worldToScreen(view, [bed.width, bed.height]);
    const left = Math.min(x0, x1);
    const right = Math.max(x0, x1);
    const top = Math.min(y0, y1);
    const bottom = Math.max(y0, y1);

    ctx.fillStyle = p.bed;
    ctx.fillRect(left, top, right - left, bottom - top);
    this.drawBackdrop(ctx, scene);

    // Cuadrícula recortada a la cama; líneas de 1 px físico (alineadas a la mitad del píxel).
    ctx.save();
    ctx.beginPath();
    ctx.rect(left, top, right - left, bottom - top);
    ctx.clip();
    const { minor, major } = gridSteps(view.zoom, 7, scene.gridUnit);
    const hair = 1 / this.dpr;
    const snap = (v: number) => Math.round(v * this.dpr) / this.dpr + hair / 2;
    const lines = (step: number, color: string) => {
      ctx.strokeStyle = color;
      ctx.lineWidth = hair;
      ctx.beginPath();
      for (let i = Math.ceil(0 / step); i * step <= bed.width + 1e-9; i++) {
        const sx = snap(view.panX + i * step * view.zoom);
        if (sx < -2 || sx > width + 2) continue;
        ctx.moveTo(sx, top);
        ctx.lineTo(sx, bottom);
      }
      for (let j = 0; j * step <= bed.height + 1e-9; j++) {
        const sy = snap(view.panY - j * step * view.zoom);
        if (sy < -2 || sy > height + 2) continue;
        ctx.moveTo(left, sy);
        ctx.lineTo(right, sy);
      }
      ctx.stroke();
    };
    if (minor * view.zoom >= 7) lines(minor, p.gridMinor);
    lines(major, p.gridMajor);
    ctx.restore();

    // Borde de la cama y marcas de registro en las esquinas.
    ctx.strokeStyle = p.bedEdge;
    ctx.lineWidth = 1;
    ctx.strokeRect(left + 0.5, top + 0.5, right - left - 1, bottom - top - 1);
    const t = Math.min(14, (right - left) / 6, (bottom - top) / 6);
    ctx.lineWidth = 2;
    ctx.beginPath();
    for (const [cx, cy, dx, dy] of [
      [left, top, 1, 1],
      [right, top, -1, 1],
      [left, bottom, 1, -1],
      [right, bottom, -1, -1],
    ] as const) {
      const ox = cx - dx * 3; // la marca queda 3 px fuera del borde
      const oy = cy - dy * 3;
      ctx.moveTo(ox + dx * t, oy);
      ctx.lineTo(ox, oy);
      ctx.lineTo(ox, oy + dy * t);
    }
    ctx.stroke();
  }

  /**
   * Imagen de la cámara bajo la cuadrícula y los objetos. Se proyecta (malla de triángulos, ver
   * backdrop.ts) en una capa del tamaño de la vista que se reutiliza mientras no cambie el cuadro,
   * la vista ni las esquinas; solo se compone con su opacidad. Sin cámara no cuesta nada.
   */
  private drawBackdrop(ctx: CanvasRenderingContext2D, scene: Scene): void {
    const b = scene.backdrop;
    if (!b) {
      this.backdropLayer = null;
      return;
    }
    const cw = Math.max(1, Math.round(this.w * this.dpr));
    const ch = Math.max(1, Math.round(this.h * this.dpr));
    let layer = this.backdropLayer;
    if (!layer || layer.canvas.width !== cw || layer.canvas.height !== ch) {
      const canvas = document.createElement('canvas');
      canvas.width = cw;
      canvas.height = ch;
      const lctx = canvas.getContext('2d');
      if (!lctx) return;
      layer = { canvas, ctx: lctx, key: null };
      this.backdropLayer = layer;
    }
    const key = backdropKeyOf(b, scene.view, this.w, this.h, this.dpr);
    if (!layer.key || !sameBackdropKey(layer.key, key)) {
      try {
        paintBackdrop(layer.ctx, b, scene.view, this.w, this.h, this.dpr);
        layer.key = key;
      } catch {
        return; // la imagen aún no se puede leer (cargando o rota): se reintenta en el próximo cuadro
      }
    }
    ctx.save();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalAlpha = Math.min(1, Math.max(0, b.opacity));
    ctx.drawImage(layer.canvas, 0, 0);
    ctx.restore();
  }

  /** Marca el origen de la máquina con ejes X (derecha) e Y (arriba). */
  private drawOrigin(ctx: CanvasRenderingContext2D, view: ViewTransform, bed: Scene['bed'], p: CanvasPalette): void {
    const corner: Record<DeviceOrigin, Point> = {
      bottom_left: [0, 0],
      top_left: [0, bed.height],
      bottom_right: [bed.width, 0],
      top_right: [bed.width, bed.height],
    };
    const [ox, oy] = worldToScreen(view, corner[bed.origin]);
    const sx = bed.origin.endsWith('left') ? 1 : -1;
    const sy = bed.origin.startsWith('bottom') ? -1 : 1;
    const len = 22;
    ctx.strokeStyle = p.origin;
    ctx.fillStyle = p.origin;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(ox + sx * len, oy);
    ctx.lineTo(ox, oy);
    ctx.lineTo(ox, oy + sy * len);
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(ox, oy, 3.5, 0, Math.PI * 2);
    ctx.fill();
  }

  // ---- diseño ------------------------------------------------------------

  /** Segmentos que caen en la vista (sobreestimado: cuenta objetos enteros). */
  private visibleSegments(scene: Scene, vb: Box): number {
    let n = 0;
    for (const obj of scene.objects) {
      if (obj.hidden) continue;
      const g = getGeom(obj);
      if (g.box && boxesIntersect(g.box, vb)) n += g.segments;
    }
    return n;
  }

  private drawObjects(ctx: CanvasRenderingContext2D, scene: Scene): void {
    const { view, width, height, overlay } = scene;
    const heavy = overlay.live === null && !overlay.nodes?.editing && typeof document !== 'undefined' &&
      this.visibleSegments(scene, visibleBox(view, width, height)) > HEAVY_SEGMENTS;
    if (!heavy) {
      this.paintObjects(ctx, scene, view, width, height, this.dpr, overlay.live);
      return;
    }
    this.drawFromBitmap(ctx, scene);
  }

  /**
   * Diseños densos: se pintan una vez en un bitmap con margen y, mientras la vista se mueve, se
   * reutiliza desplazado y escalado (fluido). Cuando la vista deja de moverse se repinta nítido.
   */
  private drawFromBitmap(ctx: CanvasRenderingContext2D, scene: Scene): void {
    const { view, width, height, overlay } = scene;
    const dpr = this.dpr;
    const key: CacheKey = {
      objects: scene.objects, layers: scene.layers, wireframe: scene.wireframe, dim: scene.preview !== null,
      w: width, h: height, dpr, images: this.imageVersion,
    };
    let bm = this.bitmap;
    const valid = bm !== null && keyEquals(bm.key, key);
    const sameView = valid && bm!.view.zoom === view.zoom && bm!.view.panX === view.panX && bm!.view.panY === view.panY;
    if (!valid || (!sameView && this.settleNow)) {
      bm = this.rebuildBitmap(scene, key);
      this.settleNow = false;
    } else if (!sameView) {
      this.armSettle(); // se ve el bitmap viejo transformado hasta que la vista se quede quieta
    }
    if (!bm) {
      this.paintObjects(ctx, scene, view, width, height, dpr, overlay.live);
      return;
    }
    const k = view.zoom / bm.view.zoom;
    ctx.save();
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.transform(k, 0, 0, k, view.panX - k * (bm.view.panX + bm.margin), view.panY - k * (bm.view.panY + bm.margin));
    ctx.drawImage(bm.canvas, 0, 0, bm.canvas.width / dpr, bm.canvas.height / dpr);
    ctx.restore();
  }

  private rebuildBitmap(scene: Scene, key: CacheKey): ObjectBitmap | null {
    const { view, width, height } = scene;
    const dpr = this.dpr;
    const margin = Math.round(Math.max(width, height) * CACHE_MARGIN);
    const cw = Math.ceil((width + margin * 2) * dpr);
    const ch = Math.ceil((height + margin * 2) * dpr);
    let bm = this.bitmap;
    if (!bm || bm.canvas.width !== cw || bm.canvas.height !== ch) {
      const canvas = document.createElement('canvas');
      canvas.width = cw;
      canvas.height = ch;
      const cctx = canvas.getContext('2d');
      if (!cctx) return null;
      bm = { canvas, ctx: cctx, margin, view, key };
    }
    bm.margin = margin;
    bm.view = { zoom: view.zoom, panX: view.panX, panY: view.panY };
    bm.key = key;
    bm.ctx.setTransform(1, 0, 0, 1, 0, 0);
    bm.ctx.clearRect(0, 0, cw, ch);
    this.paintObjects(bm.ctx, scene, { zoom: view.zoom, panX: view.panX + margin, panY: view.panY + margin }, width + margin * 2, height + margin * 2, dpr, null);
    this.bitmap = bm;
    return bm;
  }

  private armSettle(): void {
    if (this.settleTimer) clearTimeout(this.settleTimer);
    this.settleTimer = window.setTimeout(() => {
      this.settleTimer = 0;
      this.settleNow = true;
      this.invalidate();
    }, SETTLE_MS);
  }

  /** Pinta los objetos de `scene` en `ctx` con la vista dada (la de pantalla o la de un bitmap). */
  private paintObjects(ctx: CanvasRenderingContext2D, scene: Scene, view: ViewTransform, w: number, h: number, dpr: number, live: Affine | null): void {
    const { objects, layers, wireframe, preview, selectedIds } = scene;
    const vb = visibleBox(view, w, h, STROKE_PX * 2);
    const dim = preview ? DIM_ALPHA : 1;
    const minSize = 0.35 / view.zoom; // menos de ~1/3 px: no se ve
    ctx.save();
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    let color = '';
    for (const obj of objects) {
      if (obj.hidden) continue;
      const layer = layers.get(obj.layer) ?? layers.get(obj.layer.toLowerCase());
      if (layer && !layer.visible) continue;
      const g = getGeom(obj);
      if (!g.box) continue;
      const moving = live !== null && selectedIds.has(obj.id);
      if (!moving) {
        if (!boxesIntersect(g.box, vb)) continue;
        if (g.box[2] - g.box[0] < minSize && g.box[3] - g.box[1] < minSize) continue;
      }
      if (obj.layer !== color) {
        color = obj.layer;
        ctx.strokeStyle = color;
        ctx.fillStyle = color;
      }
      const scale = worldTransform(ctx, view, moving ? live : null, dpr);
      ctx.lineWidth = STROKE_PX / (view.zoom * scale);
      ctx.globalAlpha = dim;

      if (g.image) {
        this.drawImage(ctx, obj, g, wireframe, dim, vb);
        continue;
      }
      if (g.placeholder && obj.kind.type === 'text') {
        this.drawTextMarker(ctx, obj, obj.kind, g, wireframe, dim, view.zoom * scale);
        continue;
      }
      if (layer?.mode === 'fill' && !wireframe) {
        ctx.globalAlpha = dim * 0.3;
        ctx.fill(fillPath(g), 'evenodd');
        ctx.globalAlpha = dim;
      }
      strokeGeom(ctx, g, moving ? null : vb);
    }
    ctx.restore();
  }

  /** Imagen del documento con su transform; si aún no carga (o no hay URL), un marcador con su caja. */
  private drawImage(ctx: CanvasRenderingContext2D, obj: DesignObject, g: ObjectGeom, wireframe: boolean, dim: number, vb: Box): void {
    if (obj.kind.type !== 'image') return;
    const [w, h] = obj.kind.size_mm;
    const img = wireframe ? null : this.images.get(obj.kind.source_path);
    if (img) {
      ctx.save();
      const [a, b, c, d, e, f] = obj.transform;
      ctx.transform(a, b, c, d, e, f);
      // Máscara de recorte (D81): solo se ve lo que queda dentro, en coordenadas locales de la imagen.
      const mask = obj.kind.mask;
      if (mask && mask.length > 0) ctx.clip(bezToPath2D(mask), 'evenodd');
      ctx.transform(1, 0, 0, -1, 0, h); // el documento tiene Y hacia arriba: la imagen se dibuja derecha
      ctx.drawImage(img, 0, 0, w, h);
      ctx.restore();
    } else if (!wireframe) {
      ctx.globalAlpha = dim * 0.16;
      ctx.fill(fillPath(g));
      ctx.globalAlpha = dim * 0.35;
      const p = g.polys[0].pts;
      const x = new Path2D();
      x.moveTo(p[0], p[1]);
      x.lineTo(p[4], p[5]);
      x.moveTo(p[2], p[3]);
      x.lineTo(p[6], p[7]);
      ctx.stroke(x);
      ctx.globalAlpha = dim;
    }
    strokeGeom(ctx, g, vb);
  }

  /**
   * Texto sin contorno generado: la caja estimada en trazo discontinuo y el texto en una fuente
   * del sistema, para que se vea algo hasta que el motor genere el contorno real.
   */
  private drawTextMarker(
    ctx: CanvasRenderingContext2D, obj: DesignObject, text: TextObject, g: ObjectGeom, wireframe: boolean, dim: number, pxPerMm: number,
  ): void {
    ctx.save();
    ctx.setLineDash([4 / pxPerMm, 3 / pxPerMm]);
    strokeGeom(ctx, g, null);
    ctx.restore();
    if (wireframe || text.text.trim() === '') return;
    ctx.save();
    const [a, b, c, d, e, f] = obj.transform;
    ctx.transform(a, b, c, d, e, f);
    ctx.scale(1, -1); // el lienzo tiene Y hacia arriba: el texto se dibuja derecho
    ctx.globalAlpha = dim * 0.7;
    ctx.font = textFont(text, Math.max(text.size_mm, 0.1));
    ctx.textAlign = text.align;
    ctx.textBaseline = 'alphabetic';
    if ('letterSpacing' in ctx) (ctx as CanvasRenderingContext2D & { letterSpacing: string }).letterSpacing = `${text.letter_spacing_mm}px`;
    textLines(text).forEach((line, i) => ctx.fillText(line, 0, i * text.size_mm * text.line_spacing));
    ctx.restore();
  }

  private drawHover(ctx: CanvasRenderingContext2D, scene: Scene, selected: DesignObject[]): void {
    const { overlay, view, objects, palette: p } = scene;
    if (!overlay.hoverId || overlay.live || overlay.marquee || overlay.draft) return;
    if (selected.some((o) => o.id === overlay.hoverId)) return;
    const obj = objects.find((o) => o.id === overlay.hoverId);
    if (!obj || obj.hidden) return;
    // Un objeto de un grupo resalta todo el grupo (Alt+clic elige uno solo).
    const top = topGroup(obj.group);
    const hovered = top ? objects.filter((o) => topGroup(o.group) === top && !o.hidden) : [obj];
    ctx.save();
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    worldTransform(ctx, view, null, this.dpr);
    ctx.strokeStyle = p.selection;
    ctx.lineWidth = HOVER_PX / view.zoom;
    for (const o of hovered) strokeGeom(ctx, getGeom(o), null);
    ctx.restore();
  }

  // ---- selección ------------------------------------------------------------

  /** Forma principal (A) de una booleana: trazo grueso y de otro color que la selección. */
  private drawPrimary(ctx: CanvasRenderingContext2D, scene: Scene): void {
    const { primaryId, objects, view, palette: p } = scene;
    if (!primaryId) return;
    const obj = objects.find((o) => o.id === primaryId);
    if (!obj || obj.hidden) return;
    ctx.save();
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    worldTransform(ctx, view, null, this.dpr);
    ctx.strokeStyle = p.crossing;
    ctx.lineWidth = 3 / view.zoom;
    strokeGeom(ctx, getGeom(obj), null);
    ctx.restore();
  }

  private drawSelection(ctx: CanvasRenderingContext2D, scene: Scene, selected: DesignObject[]): void {
    const { view, palette: p, overlay } = scene;
    if (selected.length === 0) return;
    const live = overlay.live;
    const base = boundsOf(selected);
    if (!base) return;

    // Contorno de lo seleccionado, con el mismo trazo que el hover pero más fino.
    ctx.save();
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    const k = worldTransform(ctx, view, live, this.dpr);
    ctx.strokeStyle = p.selection;
    ctx.lineWidth = 1 / (view.zoom * k);
    ctx.globalAlpha = 0.9;
    if (selected.length <= 400) for (const o of selected) strokeGeom(ctx, getGeom(o), null);
    ctx.restore();
    if (overlay.nodes) return; // con la herramienta de nodos no hay marco de escala ni giro

    ctx.save();
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    if (live && !keepsAxes(live)) {
      // Giro en curso: el marco original, girado.
      const corners: Point[] = [
        [base[0][0], base[0][1]], [base[1][0], base[0][1]], [base[1][0], base[1][1]], [base[0][0], base[1][1]],
      ];
      ctx.strokeStyle = p.selection;
      ctx.lineWidth = 1;
      ctx.setLineDash([5, 4]);
      ctx.beginPath();
      corners.forEach((c, i) => {
        const wx = live[0] * c[0] + live[2] * c[1] + live[4];
        const wy = live[1] * c[0] + live[3] * c[1] + live[5];
        const [sx, sy] = worldToScreen(view, [wx, wy]);
        if (i === 0) ctx.moveTo(sx, sy);
        else ctx.lineTo(sx, sy);
      });
      ctx.closePath();
      ctx.stroke();
      if (overlay.pivot) {
        const [px, py] = worldToScreen(view, overlay.pivot);
        ctx.setLineDash([]);
        ctx.beginPath();
        ctx.moveTo(px - 6, py);
        ctx.lineTo(px + 6, py);
        ctx.moveTo(px, py - 6);
        ctx.lineTo(px, py + 6);
        ctx.stroke();
      }
      ctx.restore();
      return;
    }
    const b = live ? (boundsOf(selected, live) ?? base) : base;
    const r = frameRect(b, view);
    ctx.fillStyle = p.selectionFill;
    ctx.fillRect(r.x, r.y, r.w, r.h);
    ctx.strokeStyle = p.selection;
    ctx.lineWidth = 1;
    ctx.setLineDash([5, 4]);
    ctx.strokeRect(Math.round(r.x) + 0.5, Math.round(r.y) + 0.5, r.w, r.h);
    ctx.setLineDash([]);
    if (!live) {
      const spots = handleSpots(b, view, { rotateMode: overlay.rotateMode, canRotate: selectionCanRotate(selected) });
      ctx.fillStyle = p.workspace;
      for (const s of spots) {
        if (s.id === 'knob') {
          ctx.beginPath();
          ctx.moveTo(s.x, r.y);
          ctx.lineTo(s.x, s.y + 5);
          ctx.stroke();
        }
        if (s.kind === 'rotate') {
          ctx.beginPath();
          ctx.arc(s.x, s.y, s.id === 'knob' ? 5 : 5.5, 0, Math.PI * 2);
          ctx.fill();
          ctx.stroke();
        } else {
          ctx.fillRect(s.x - 3.5, s.y - 3.5, 7, 7);
          ctx.strokeRect(s.x - 3.5, s.y - 3.5, 7, 7);
        }
      }
    }
    ctx.restore();
  }
}
