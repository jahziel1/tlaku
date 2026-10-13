import type { DeviceOrigin, Point } from '@/api/types';
import { applyAffine, objectBounds, unionBounds } from '@/lib/geometry';
import { gridSteps, worldToScreen, type ViewTransform } from '@/lib/viewport';
import type { CanvasPalette, Renderer, Scene } from './renderer';

/** Renderer de referencia en Canvas 2D: cama, cuadrícula, objetos, selección y vista previa. */
export class Canvas2DRenderer implements Renderer {
  readonly kind = 'canvas2d' as const;
  private canvas: HTMLCanvasElement | null = null;
  private ctx: CanvasRenderingContext2D | null = null;
  private w = 0;
  private h = 0;
  private dpr = 1;

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
    this.canvas = null;
    this.ctx = null;
  }

  render(scene: Scene): void {
    const ctx = this.ctx;
    if (!ctx || this.w === 0) return;
    const { palette: p, view } = scene;
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    ctx.fillStyle = p.workspace;
    ctx.fillRect(0, 0, this.w, this.h);

    this.drawBed(ctx, scene);
    this.drawObjects(ctx, scene);
    if (scene.preview) this.drawPreview(ctx, scene);
    else this.drawSelection(ctx, scene);
    this.drawOrigin(ctx, view, scene.bed, p);
  }

  // ---- cama y cuadrícula -------------------------------------------------

  private drawBed(ctx: CanvasRenderingContext2D, { view, bed, palette: p, width, height }: Scene): void {
    const [x0, y0] = worldToScreen(view, [0, 0]);
    const [x1, y1] = worldToScreen(view, [bed.width, bed.height]);
    const left = Math.min(x0, x1);
    const right = Math.max(x0, x1);
    const top = Math.min(y0, y1);
    const bottom = Math.max(y0, y1);

    ctx.fillStyle = p.bed;
    ctx.fillRect(left, top, right - left, bottom - top);

    // Cuadrícula recortada a la cama; líneas de 1 px físico (alineadas a la mitad del píxel).
    ctx.save();
    ctx.beginPath();
    ctx.rect(left, top, right - left, bottom - top);
    ctx.clip();
    const { minor, major } = gridSteps(view.zoom);
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

  private drawObjects(ctx: CanvasRenderingContext2D, scene: Scene): void {
    const { view, objects, layers, wireframe, preview } = scene;
    ctx.save();
    if (preview) ctx.globalAlpha = 0.22;
    for (const obj of objects) {
      if (obj.hidden) continue;
      const layer = layers.get(obj.layer);
      if (layer && !layer.visible) continue;
      const color = obj.layer;
      ctx.strokeStyle = color;
      ctx.fillStyle = color;
      ctx.lineWidth = 1.4;
      ctx.lineJoin = 'round';

      if (obj.kind.type === 'image') {
        const [w, h] = obj.kind.size_mm;
        const a = worldToScreen(view, applyAffine(obj.transform, [0, 0]));
        const b = worldToScreen(view, applyAffine(obj.transform, [w, h]));
        const x = Math.min(a[0], b[0]);
        const y = Math.min(a[1], b[1]);
        const rw = Math.abs(b[0] - a[0]);
        const rh = Math.abs(b[1] - a[1]);
        ctx.globalAlpha = (preview ? 0.22 : 1) * (wireframe ? 0 : 0.16);
        ctx.fillRect(x, y, rw, rh);
        ctx.globalAlpha = preview ? 0.22 : 1;
        ctx.strokeRect(x, y, rw, rh);
        ctx.beginPath();
        ctx.moveTo(x, y);
        ctx.lineTo(x + rw, y + rh);
        ctx.moveTo(x + rw, y);
        ctx.lineTo(x, y + rh);
        ctx.globalAlpha = (preview ? 0.22 : 1) * 0.35;
        ctx.stroke();
        ctx.globalAlpha = preview ? 0.22 : 1;
        continue;
      }

      ctx.beginPath();
      for (const path of obj.kind.paths) {
        path.points.forEach((pt, i) => {
          const [sx, sy] = worldToScreen(view, applyAffine(obj.transform, pt));
          if (i === 0) ctx.moveTo(sx, sy);
          else ctx.lineTo(sx, sy);
        });
        if (path.closed) ctx.closePath();
      }
      if (layer?.mode === 'fill' && !wireframe) {
        const a = ctx.globalAlpha;
        ctx.globalAlpha = a * 0.3;
        ctx.fill('evenodd');
        ctx.globalAlpha = a;
      }
      ctx.stroke();
    }
    ctx.restore();
  }

  private drawSelection(ctx: CanvasRenderingContext2D, { view, objects, selectedIds, palette: p }: Scene): void {
    const picked = objects.filter((o) => selectedIds.has(o.id));
    const b = unionBounds(picked.map(objectBounds));
    if (!b) return;
    const [x0, y0] = worldToScreen(view, [b[0][0], b[1][1]]);
    const [x1, y1] = worldToScreen(view, [b[1][0], b[0][1]]);
    const pad = 6;
    const x = x0 - pad;
    const y = y0 - pad;
    const w = x1 - x0 + pad * 2;
    const h = y1 - y0 + pad * 2;
    ctx.save();
    ctx.fillStyle = p.selectionFill;
    ctx.fillRect(x, y, w, h);
    ctx.strokeStyle = p.selection;
    ctx.lineWidth = 1;
    ctx.setLineDash([5, 4]);
    ctx.strokeRect(Math.round(x) + 0.5, Math.round(y) + 0.5, w, h);
    ctx.setLineDash([]);
    ctx.fillStyle = p.workspace;
    for (const [hx, hy] of [
      [x, y], [x + w / 2, y], [x + w, y],
      [x, y + h / 2], [x + w, y + h / 2],
      [x, y + h], [x + w / 2, y + h], [x + w, y + h],
    ] as const) {
      ctx.fillRect(hx - 3.5, hy - 3.5, 7, 7);
      ctx.strokeRect(hx - 3.5, hy - 3.5, 7, 7);
    }
    ctx.restore();
  }

  // ---- vista previa del recorrido ----------------------------------------

  private drawPreview(ctx: CanvasRenderingContext2D, { view, preview, palette: p }: Scene): void {
    if (!preview) return;
    const m = preview.data.moves;
    const count = Math.floor(m.length / 4);
    const upto = Math.floor(count * preview.scrub);
    const travel = new Path2D();
    const cut = new Path2D();
    const engrave = new Path2D();
    const over = new Path2D();
    let [sx0, sy0] = worldToScreen(view, [0, 0]);
    for (let i = 0; i < upto; i++) {
      const kind = m[i * 4]!;
      const x = m[i * 4 + 1]!;
      const y = m[i * 4 + 2]!;
      const power = m[i * 4 + 3]!;
      const [sx, sy] = worldToScreen(view, [x, y]);
      // Potencia >= 60 %: corte; menos: grabado; 0: overscan (el cabezal acelera sin disparar).
      const target = kind === 0 ? travel : power >= 60 ? cut : power > 0 ? engrave : over;
      target.moveTo(sx0, sy0);
      target.lineTo(sx, sy);
      sx0 = sx;
      sy0 = sy;
    }
    ctx.save();
    ctx.lineJoin = 'round';
    ctx.lineWidth = 1;
    ctx.strokeStyle = p.travel;
    ctx.setLineDash([3, 4]);
    ctx.stroke(travel);
    ctx.setLineDash([]);
    ctx.strokeStyle = p.overscan;
    ctx.stroke(over);
    ctx.strokeStyle = p.engrave;
    ctx.stroke(engrave);
    ctx.strokeStyle = p.cut;
    ctx.lineWidth = 1.6;
    ctx.stroke(cut);
    if (upto > 0 && upto < count) {
      ctx.fillStyle = p.head;
      ctx.beginPath();
      ctx.arc(sx0, sy0, 5, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = p.head;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.arc(sx0, sy0, 10, 0, Math.PI * 2);
      ctx.stroke();
    }
    ctx.restore();
  }
}
