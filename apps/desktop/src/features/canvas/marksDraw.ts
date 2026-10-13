import type { Point } from '@/api/types';
import { worldToScreen } from '@/lib/viewport';
import type { SceneMarks } from './marks';
import type { CanvasPalette, Scene } from './renderer';

const TAB_PX = 4;
const ARROW_PX = 24;
const HEAD_PX = 8;

/** Dibuja las marcas de diseño (guías, puentes manuales, flechas de inicio) sobre el lienzo. */
export function drawMarks(ctx: CanvasRenderingContext2D, scene: Scene, dpr: number): void {
  const marks = scene.marks;
  if (!marks) return;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  drawGuideLines(ctx, scene, marks);
  drawTabs(ctx, scene, marks);
  drawStarts(ctx, scene, marks);
  drawHover(ctx, scene, marks);
  ctx.restore();
}

function drawGuideLines(ctx: CanvasRenderingContext2D, { view, width, height, palette: p }: Scene, marks: SceneMarks): void {
  const color = p.docGuide ?? p.engrave;
  for (const g of marks.guides) {
    const [sx, sy] = worldToScreen(view, g.axis === 'x' ? [g.pos, 0] : [0, g.pos]);
    ctx.strokeStyle = g.remove ? p.travel : color;
    ctx.globalAlpha = g.remove ? 0.6 : 1;
    ctx.lineWidth = g.hover ? 2 : 1;
    ctx.setLineDash(g.remove ? [4, 4] : []);
    ctx.beginPath();
    if (g.axis === 'x') {
      const x = Math.round(sx) + 0.5;
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
    } else {
      const y = Math.round(sy) + 0.5;
      ctx.moveTo(0, y);
      ctx.lineTo(width, y);
    }
    ctx.stroke();
  }
  ctx.setLineDash([]);
  ctx.globalAlpha = 1;
}

function drawTabs(ctx: CanvasRenderingContext2D, { view, palette: p }: Scene, marks: SceneMarks): void {
  if (marks.tabs.length === 0) return;
  const color = p.tab ?? p.cut;
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  for (const pass of [0, 1]) {
    // Primero un trazo ancho del color del fondo para separar el puente del contorno, luego el color.
    ctx.strokeStyle = pass === 0 ? p.bed : color;
    ctx.lineWidth = pass === 0 ? TAB_PX + 3 : TAB_PX;
    for (const tab of marks.tabs) {
      ctx.globalAlpha = tab.active ? 1 : 0.45;
      ctx.beginPath();
      tab.span.forEach((pt, i) => {
        const [x, y] = worldToScreen(view, pt);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      });
      ctx.stroke();
    }
  }
  ctx.globalAlpha = 1;
  ctx.lineCap = 'butt';
}

function drawStarts(ctx: CanvasRenderingContext2D, { view, palette: p }: Scene, marks: SceneMarks): void {
  for (const a of marks.starts) {
    const [x, y] = worldToScreen(view, a.point);
    // Y de pantalla va hacia abajo.
    const dx = a.dir[0];
    const dy = -a.dir[1];
    const ex = x + dx * ARROW_PX;
    const ey = y + dy * ARROW_PX;
    ctx.fillStyle = a.fixed ? p.selection : p.travel;
    for (const outline of [true, false]) {
      ctx.strokeStyle = outline ? p.bed : a.fixed ? p.selection : p.travel;
      ctx.lineWidth = outline ? 4 : 1.75;
      ctx.setLineDash(a.fixed || outline ? [] : [3, 2]);
      ctx.beginPath();
      ctx.moveTo(x, y);
      ctx.lineTo(ex, ey);
      ctx.stroke();
    }
    ctx.setLineDash([]);
    // Punta de flecha.
    const nx = -dy;
    const ny = dx;
    ctx.beginPath();
    ctx.moveTo(ex + dx * 3, ey + dy * 3);
    ctx.lineTo(ex - dx * HEAD_PX + nx * (HEAD_PX * 0.55), ey - dy * HEAD_PX + ny * (HEAD_PX * 0.55));
    ctx.lineTo(ex - dx * HEAD_PX - nx * (HEAD_PX * 0.55), ey - dy * HEAD_PX - ny * (HEAD_PX * 0.55));
    ctx.closePath();
    ctx.fill();
    // Disco en el punto de inicio.
    ctx.beginPath();
    ctx.arc(x, y, 3.5, 0, Math.PI * 2);
    ctx.fillStyle = p.bed;
    ctx.fill();
    ctx.strokeStyle = a.fixed ? p.selection : p.travel;
    ctx.lineWidth = 1.75;
    ctx.stroke();
  }
}

function drawHover(ctx: CanvasRenderingContext2D, { view, palette }: Scene, marks: SceneMarks): void {
  const h: Point | null = marks.hover;
  if (!h) return;
  const [x, y] = worldToScreen(view, h);
  ctx.strokeStyle = (palette as CanvasPalette).tab ?? palette.selection;
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.arc(x, y, 6, 0, Math.PI * 2);
  ctx.stroke();
}
