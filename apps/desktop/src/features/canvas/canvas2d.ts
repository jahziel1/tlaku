import type { Affine, DesignObject, DeviceOrigin, Point, TextObject } from '@/api/types';
import { topGroup } from '@/lib/groups';
import { textFont, textLines } from '@/lib/textobj';
import { gridSteps, worldToScreen, type ViewTransform } from '@/lib/viewport';
import { boxContains, boxesIntersect, fillPath, getGeom, visibleBox, type Box, type ObjectGeom } from './geom';
import { CACHE_MARGIN, LayerBitmap, REGION_PAD_PX, limitRegions, padBox, sameView, sourceBox } from './layerBitmap';
import { diffObjects } from './objectDiff';
import { objectIndex } from '@/store/objectIndex';
import { INDEX_MIN_OBJECTS, spatialIndexFor } from './spatialIndex';
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
/** Tiempo sin mover la vista tras el cual se repinta nítido (ms). */
const SETTLE_MS = 120;
/** Una selección con más segmentos que esto durante un gesto va en un bitmap propio (si no, se traza directo). */
export const MOVING_BITMAP_SEGMENTS = 4_000;

/** Qué parte de la lista de objetos se pinta y con qué matriz viva (ver `paintObjects`). */
interface PaintOpts {
  live: Affine | null;
  only?: ReadonlySet<string> | null;
  skip?: ReadonlySet<string> | null;
  region?: Box | null;
}

/** A qué documento y selección corresponde el contenido del bitmap principal. */
interface MainState {
  objects: DesignObject[];
  /** Ids que se dejaron fuera mientras dura un gesto (la selección que se arrastra), o `null`. */
  hole: ReadonlySet<string> | null;
  /** Zonas (mm) donde estaban esos objetos: al terminar el gesto hay que repintarlas. */
  holeRects: Box[];
}

const segmentsMemo = new WeakMap<readonly DesignObject[], number>();

/** Segmentos aplanados de los objetos (se guarda por arreglo). */
function segmentsOf(objs: readonly DesignObject[]): number {
  let n = segmentsMemo.get(objs);
  if (n === undefined) {
    n = 0;
    for (const o of objs) n += getGeom(o).segments;
    segmentsMemo.set(objs, n);
  }
  return n;
}

function addBox(out: Box[], b: Box | null): void {
  if (b) out.push(b);
}

/** Cajas de los objetos (unidas en pocas si son muchas): las zonas que hay que repintar al mover o soltar la selección. */
function selectionRegions(objs: readonly DesignObject[]): Box[] {
  const out: Box[] = [];
  for (const o of objs) addBox(out, getGeom(o).box);
  return limitRegions(out);
}

/** Objeto por id sin recorrer la lista (índice por arreglo; si hay ids repetidos, recorre). */
function objectById(objects: readonly DesignObject[], id: string): DesignObject | undefined {
  const idx = objectIndex(objects);
  if (!idx.unique) return objects.find((o) => o.id === id);
  const i = idx.at.get(id);
  return i === undefined ? undefined : objects[i];
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
  /** Bitmap del diseño denso, el de lo que se arrastra en un gesto y la memoria de qué contiene el principal. */
  private main = new LayerBitmap();
  private moving = new LayerBitmap();
  private mainState: MainState | null = null;
  private heavyMemo: { objects: DesignObject[]; w: number; h: number; view: ViewTransform; heavy: boolean } | null = null;
  private selectedMemo: { objects: DesignObject[]; ids: ReadonlySet<string>; list: DesignObject[] } | null = null;
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
    this.main.release();
    this.moving.release();
    this.mainState = null;
    this.heavyMemo = null;
    this.selectedMemo = null;
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
    const selected = this.selectedOf(scene);
    this.drawObjects(ctx, scene, selected);
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
    this.settleNow = false;
  }

  /** Objetos seleccionados (en el orden del documento); se guarda por documento y selección, no se recorre en cada cuadro. */
  private selectedOf(scene: Scene): DesignObject[] {
    if (scene.selected) return scene.selected;
    const m = this.selectedMemo;
    if (m && m.objects === scene.objects && m.ids === scene.selectedIds) return m.list;
    const list = scene.selectedIds.size === 0 ? [] : scene.objects.filter((o) => scene.selectedIds.has(o.id));
    this.selectedMemo = { objects: scene.objects, ids: scene.selectedIds, list };
    return list;
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

  /** ¿Es denso lo que se ve? (segmentos que caen en la vista, sobreestimado: cuenta objetos enteros). Se guarda por documento y vista. */
  private isHeavy(scene: Scene): boolean {
    const { view, width, height, objects } = scene;
    const m = this.heavyMemo;
    if (m && m.objects === objects && m.w === width && m.h === height && sameView(m.view, view)) return m.heavy;
    const vb = visibleBox(view, width, height);
    let n = 0;
    for (const obj of objects) {
      if (obj.hidden) continue;
      const g = getGeom(obj);
      if (g.box && boxesIntersect(g.box, vb)) n += g.segments;
      if (n > HEAVY_SEGMENTS) break;
    }
    const heavy = n > HEAVY_SEGMENTS;
    this.heavyMemo = { objects, w: width, h: height, view: { zoom: view.zoom, panX: view.panX, panY: view.panY }, heavy };
    return heavy;
  }

  private drawObjects(ctx: CanvasRenderingContext2D, scene: Scene, selected: DesignObject[]): void {
    const { view, width, height, overlay } = scene;
    const heavy = !overlay.nodes?.editing && typeof document !== 'undefined' && this.isHeavy(scene);
    if (!heavy) {
      this.paintObjects(ctx, scene, view, width, height, this.dpr, { live: overlay.live });
      return;
    }
    this.drawHeavy(ctx, scene, selected);
  }

  /**
   * Diseños densos: se pintan una vez en un bitmap con margen y, mientras la vista se mueve, se
   * reutiliza desplazado y escalado (fluido). Cuando la vista deja de moverse se repinta nítido.
   * Con un gesto en curso (mover, escalar, girar) lo seleccionado sale del bitmap principal y va en otro
   * propio que se compone con la matriz viva: nada se vuelve a trazar mientras se arrastra.
   */
  private drawHeavy(ctx: CanvasRenderingContext2D, scene: Scene, selected: DesignObject[]): void {
    const { view, width, height, overlay } = scene;
    const live = overlay.live;
    const gesture = live !== null && scene.selectedIds.size > 0;
    const hole = gesture ? scene.selectedIds : null;
    const main = this.syncMain(scene, hole, selected);
    if (!main) {
      this.paintObjects(ctx, scene, view, width, height, this.dpr, { live });
      return;
    }
    main.drawOnto(ctx, view);
    if (gesture && live) this.drawMoving(ctx, scene, selected, live);
    else this.moving.release(); // sin gesto no hace falta (se vuelve a crear en el siguiente)
  }

  /** Identidades de las que depende el bitmap principal además de los objetos y del hueco de la selección. */
  private envTag(scene: Scene): unknown[] {
    return [scene.layers, scene.wireframe, scene.preview !== null, scene.width, scene.height, this.dpr, this.imageVersion];
  }

  /** Deja el bitmap principal al día con `scene` (parchándolo si cambió poco) y sin `hole`; `null` si no hay dónde pintar. */
  private syncMain(scene: Scene, hole: ReadonlySet<string> | null, selected: DesignObject[]): LayerBitmap | null {
    const { view, objects } = scene;
    const env = this.envTag(scene);
    const bm = this.main;
    const st = this.mainState;
    const envOk = st !== null && bm.hasTag(env);
    const viewOk = envOk && sameView(bm.view, view);
    if (envOk && st.objects === objects && st.hole === hole) {
      if (viewOk) return bm;
      if (!this.settleNow) {
        this.armSettle(); // se ve el bitmap viejo transformado hasta que la vista se quede quieta
        return bm;
      }
    } else if (envOk && viewOk) {
      if (this.beginHoleFromMain(scene, st, hole, selected)) return bm;
      if (this.patchMain(scene, st, hole, selected)) return bm;
    }
    return this.rebuildMain(scene, env, hole, selected);
  }

  private rebuildMain(scene: Scene, env: unknown[], hole: ReadonlySet<string> | null, selected: DesignObject[]): LayerBitmap | null {
    const { view, width, height } = scene;
    const dpr = this.dpr;
    const ok = this.main.paintAll(view, width, height, dpr, env, (c, v, bw, bh) => {
      this.paintObjects(c, scene, v, bw, bh, dpr, { live: null, skip: hole });
    });
    if (!ok) {
      this.mainState = null;
      return null;
    }
    this.mainState = { objects: scene.objects, hole, holeRects: hole ? selectionRegions(selected) : [] };
    return this.main;
  }

  /**
   * Cambió el documento o la selección en gesto: en vez de repintar todo, repinta solo las zonas que
   * cambiaron (cajas de los objetos que cambiaron, antes y después, y del hueco anterior y el nuevo).
   * `false` si cambió demasiado o la zona cubre media vista: entonces conviene repintar todo.
   */
  private patchMain(scene: Scene, st: MainState, hole: ReadonlySet<string> | null, selected: DesignObject[]): boolean {
    const regions: Box[] = [];
    if (st.objects !== scene.objects) {
      const changed = diffObjects(st.objects, scene.objects, Math.max(64, scene.objects.length >> 4));
      if (!changed) return false;
      for (const i of changed) {
        if (i < st.objects.length) addBox(regions, getGeom(st.objects[i]!).box);
        addBox(regions, getGeom(scene.objects[i]!).box);
      }
    }
    if (st.hole !== hole) {
      for (const r of st.holeRects) regions.push(r);
      if (hole) for (const r of selectionRegions(selected)) regions.push(r);
    }
    const parts = limitRegions(regions);
    const { zoom } = scene.view;
    let px = 0;
    for (const r of parts) px += (r[2] - r[0] + (2 * REGION_PAD_PX) / zoom) * (r[3] - r[1] + (2 * REGION_PAD_PX) / zoom) * zoom * zoom;
    if (px > 0.5 * (scene.width * scene.height)) return false;
    const dpr = this.dpr;
    const env = this.envTag(scene);
    this.main.paintRegions(parts, env, (c, v, bw, bh, region) => {
      this.paintObjects(c, scene, v, bw, bh, dpr, { live: null, skip: hole, region });
    });
    this.mainState = { objects: scene.objects, hole, holeRects: hole ? selectionRegions(selected) : [] };
    return true;
  }

  /**
   * Al empezar un gesto sobre un bitmap principal limpio en el que lo único que se ve es la selección
   * (un trayecto enorme, o seleccionar todo), el bitmap ya es el de lo que se mueve: se copia al de la
   * selección en vez de volver a trazarlo, y el principal queda vacío.
   */
  private beginHoleFromMain(scene: Scene, st: MainState, hole: ReadonlySet<string> | null, selected: DesignObject[]): boolean {
    if (!hole || st.hole !== null || st.objects !== scene.objects || segmentsOf(selected) <= MOVING_BITMAP_SEGMENTS) return false;
    const { view, width, height, selectedIds } = scene;
    const cover = this.main.coverage();
    for (const obj of scene.objects) {
      if (selectedIds.has(obj.id)) continue;
      const b = getGeom(obj).box;
      if (b && boxesIntersect(b, cover)) return false;
    }
    const tag = this.movingTag(scene);
    if (!this.moving.copyFrom(this.main, tag)) return false;
    // El principal queda sin nada: solo hay que borrarlo.
    const ok = this.main.paintAll(view, width, height, this.dpr, this.envTag(scene), () => {});
    if (!ok) return false;
    this.mainState = { objects: scene.objects, hole, holeRects: selectionRegions(selected) };
    return true;
  }

  private movingTag(scene: Scene): unknown[] {
    return [scene.objects, scene.selectedIds, ...this.envTag(scene)];
  }

  /** Lo seleccionado durante un gesto: bitmap propio compuesto con la matriz viva, o trazado directo si es poco o se sale del bitmap. */
  private drawMoving(ctx: CanvasRenderingContext2D, scene: Scene, selected: DesignObject[], live: Affine): void {
    const { view, width, height } = scene;
    const direct = () => this.paintObjects(ctx, scene, view, width, height, this.dpr, { live, only: scene.selectedIds });
    if (segmentsOf(selected) <= MOVING_BITMAP_SEGMENTS) return direct();
    const margin = Math.round(Math.max(width, height) * CACHE_MARGIN);
    const bounds = boundsOf(selected);
    const bm = this.moving;
    const tag = this.movingTag(scene);
    // Si parte de lo seleccionado queda fuera del bitmap, ahí no hay nada que componer: se traza directo.
    if (bounds && !bm.hasTag(tag) && !boxContains(visibleBox(view, width, height, margin), [bounds[0][0], bounds[0][1], bounds[1][0], bounds[1][1]])) return direct();
    const ok = this.ensureLayer(bm, scene, tag, (c, v, bw, bh) => {
      this.paintObjects(c, scene, v, bw, bh, this.dpr, { live: null, only: scene.selectedIds });
    });
    if (!ok) return direct();
    bm.drawOnto(ctx, view, live);
  }

  /** Bitmap `bm` con contenido `tag` y vista actual: lo reutiliza, lo deja viejo hasta que la vista se asiente, o lo repinta con `paint`. */
  private ensureLayer(
    bm: LayerBitmap, scene: Scene, tag: readonly unknown[], paint: (ctx: CanvasRenderingContext2D, v: ViewTransform, bw: number, bh: number) => void,
  ): boolean {
    const { view, width, height } = scene;
    if (bm.hasTag(tag)) {
      if (sameView(bm.view, view)) return true;
      if (!this.settleNow) {
        this.armSettle();
        return true;
      }
    }
    return bm.paintAll(view, width, height, this.dpr, tag, paint);
  }

  private armSettle(): void {
    if (this.settleTimer) clearTimeout(this.settleTimer);
    this.settleTimer = window.setTimeout(() => {
      this.settleTimer = 0;
      this.settleNow = true;
      this.invalidate();
    }, SETTLE_MS);
  }

  /**
   * Pinta los objetos de `scene` en `ctx` con la vista dada (la de pantalla o la de un bitmap).
   * `live`: matriz viva que se aplica a lo seleccionado; `only`/`skip`: solo esos ids / todos menos esos;
   * `region`: zona de documento a la que se limita el descarte (al repintar una parte de un bitmap).
   */
  private paintObjects(ctx: CanvasRenderingContext2D, scene: Scene, view: ViewTransform, w: number, h: number, dpr: number, o: PaintOpts): void {
    const { objects, layers, wireframe, preview, selectedIds } = scene;
    const { live, only, skip } = o;
    // Al repintar una zona, los objetos que la tocan con su trazo (la caja de la geometría no incluye el grosor).
    const vb = o.region ? padBox(o.region, STROKE_PX * 2, view.zoom) : visibleBox(view, w, h, STROKE_PX * 2);
    // Lo que se mueve no se descarta contra la vista sino contra la vista vista desde su propio espacio.
    const vbSrc = live ? sourceBox(vb, live) : null;
    const dim = preview ? DIM_ALPHA : 1;
    const minSize = 0.35 / view.zoom; // menos de ~1/3 px: no se ve
    ctx.save();
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    let color = '';
    let curMoving: boolean | null = null;
    let scale = 1;
    ctx.globalAlpha = dim;
    // Al repintar una zona de un documento enorme, solo los objetos cuya caja la toca (en el orden del documento).
    const list: readonly DesignObject[] = o.region && objects.length >= INDEX_MIN_OBJECTS ? spatialIndexFor(objects).inBox(vb).map((i) => objects[i]!) : objects;
    for (const obj of list) {
      if (obj.hidden) continue;
      if (only && !only.has(obj.id)) continue;
      if (skip && skip.has(obj.id)) continue;
      const layer = layers.get(obj.layer) ?? layers.get(obj.layer.toLowerCase());
      if (layer && !layer.visible) continue;
      const g = getGeom(obj);
      if (!g.box) continue;
      const moving = live !== null && selectedIds.has(obj.id);
      if (moving) {
        if (vbSrc && !boxesIntersect(g.box, vbSrc)) continue;
      } else {
        if (!boxesIntersect(g.box, vb)) continue;
        if (g.box[2] - g.box[0] < minSize && g.box[3] - g.box[1] < minSize) continue;
      }
      if (obj.layer !== color) {
        color = obj.layer;
        ctx.strokeStyle = color;
        ctx.fillStyle = color;
      }
      // La matriz y el ancho de línea solo cambian al pasar de lo fijo a lo que se mueve (y al revés):
      // con 100 000 objetos, fijarlos en cada uno eran cientos de miles de llamadas al contexto.
      if (moving !== curMoving) {
        scale = worldTransform(ctx, view, moving ? live : null, dpr);
        ctx.lineWidth = STROKE_PX / (view.zoom * scale);
        curMoving = moving;
      }
      const cull = moving ? vbSrc : vb;

      if (g.image) {
        this.drawImage(ctx, obj, g, wireframe, dim, cull);
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
      strokeGeom(ctx, g, cull);
    }
    ctx.restore();
  }

  /** Imagen del documento con su transform; si aún no carga (o no hay URL), un marcador con su caja. */
  private drawImage(ctx: CanvasRenderingContext2D, obj: DesignObject, g: ObjectGeom, wireframe: boolean, dim: number, vb: Box | null): void {
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
    const obj = objectById(objects, overlay.hoverId);
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
    if (segmentsOf(hovered) > HEAVY_SEGMENTS) {
      // Un trayecto de cientos de miles de segmentos no se vuelve a trazar para resaltarlo: se marca su caja.
      const b = boundsOf(hovered);
      if (b) ctx.strokeRect(b[0][0], b[0][1], b[1][0] - b[0][0], b[1][1] - b[0][1]);
    } else {
      for (const o of hovered) strokeGeom(ctx, getGeom(o), null);
    }
    ctx.restore();
  }

  // ---- selección ------------------------------------------------------------

  /** Forma principal (A) de una booleana: trazo grueso y de otro color que la selección. */
  private drawPrimary(ctx: CanvasRenderingContext2D, scene: Scene): void {
    const { primaryId, objects, view, palette: p } = scene;
    if (!primaryId) return;
    const obj = objectById(objects, primaryId);
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
    // Con muchos objetos o un trazado enorme el contorno no se vuelve a trazar: queda el marco de la selección.
    if (selected.length <= 400 && segmentsOf(selected) <= HEAVY_SEGMENTS) for (const o of selected) strokeGeom(ctx, getGeom(o), null);
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
