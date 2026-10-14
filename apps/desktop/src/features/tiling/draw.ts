import { worldToScreen } from '@/lib/viewport';
import type { Scene } from '@/features/canvas/renderer';
import type { SceneTile } from './overlay';

const BADGE_PX = 11;

/**
 * Dibuja los tramos numerados sobre el lienzo (Canvas 2D). Los pendientes van con borde
 * discontinuo, el actual resaltado, los enviados rellenos y los saltados tachados. Las franjas de
 * solape se notan porque los rellenos translúcidos se suman.
 */
export function drawTiles(ctx: CanvasRenderingContext2D, scene: Scene, dpr: number): void {
  const tiles = scene.tiles?.tiles;
  if (!tiles || tiles.length === 0) return;
  const { view, palette: p } = scene;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  for (const tile of tiles) drawTile(ctx, tile, view, p);
  ctx.restore();
}

function drawTile(ctx: CanvasRenderingContext2D, tile: SceneTile, view: Scene['view'], p: Scene['palette']): void {
  const [ax, ay] = worldToScreen(view, tile.rect[0]);
  const [bx, by] = worldToScreen(view, tile.rect[1]);
  const x = Math.min(ax, bx);
  const y = Math.min(ay, by);
  const w = Math.abs(bx - ax);
  const h = Math.abs(by - ay);
  const color = tile.current ? p.selection : tile.state === 'done' ? p.engrave : tile.state === 'skipped' ? p.travel : p.guide;

  ctx.globalAlpha = tile.state === 'done' ? 0.18 : 0.1;
  ctx.fillStyle = color;
  ctx.fillRect(x, y, w, h);
  ctx.globalAlpha = 1;
  ctx.strokeStyle = color;
  ctx.lineWidth = tile.current ? 2.5 : 1.25;
  ctx.setLineDash(tile.state === 'preview' || tile.state === 'pending' ? [6, 4] : []);
  ctx.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);
  ctx.setLineDash([]);
  if (tile.state === 'skipped') {
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(x + w, y + h);
    ctx.moveTo(x + w, y);
    ctx.lineTo(x, y + h);
    ctx.stroke();
  }

  // Número del tramo en un círculo, en la esquina superior izquierda.
  const cx = x + BADGE_PX + 4;
  const cy = y + BADGE_PX + 4;
  if (w < BADGE_PX * 3 || h < BADGE_PX * 3) return;
  ctx.fillStyle = color;
  ctx.beginPath();
  ctx.arc(cx, cy, BADGE_PX, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = p.workspace;
  ctx.font = '600 12px sans-serif';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(String(tile.label), cx, cy + 0.5);
}
