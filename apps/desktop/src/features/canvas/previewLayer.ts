import { worldToScreen } from '@/lib/viewport';
import type { Scene } from './renderer';

/**
 * Dibuja el recorrido de la vista previa sobre el lienzo. Este archivo es del área de vista
 * previa; el renderer solo lo llama al final de cada cuadro.
 */
export function drawPreview(ctx: CanvasRenderingContext2D, { view, preview, palette: p }: Scene): void {
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
