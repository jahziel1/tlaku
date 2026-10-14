/** Capas de pintura Canvas 2D sin estado: nodos, selección en curso, guías, medidas y cabezal. */
import type { Affine } from '@/api/types';
import { worldToScreen, type ViewTransform } from '@/lib/viewport';
import { boxesIntersect, chunkPath, getGeom, type Box, type ObjectGeom } from './geom';
import type { Scene } from './renderer';

/** Contexto en coordenadas de documento (mm) con la vista y, opcionalmente, una matriz viva encima. */
export function worldTransform(ctx: CanvasRenderingContext2D, view: ViewTransform, live: Affine | null, dpr: number): number {
  const k = dpr * view.zoom;
  ctx.setTransform(k, 0, 0, -k, dpr * view.panX, dpr * view.panY);
  if (!live) return 1;
  ctx.transform(live[0], live[1], live[2], live[3], live[4], live[5]);
  return Math.sqrt(Math.abs(live[0] * live[3] - live[1] * live[2])) || 1;
}

export function strokeGeom(ctx: CanvasRenderingContext2D, g: ObjectGeom, vb: Box | null): void {
  for (const ch of g.chunks) {
    if (vb && !boxesIntersect(ch.box, vb)) continue;
    ctx.stroke(chunkPath(g, ch));
  }
}

// ---- nodos ---------------------------------------------------------------------------

/** Nodos (esquina = cuadrado, suave = círculo, simétrico = rombo) y manejadores de la herramienta de nodos. */
export function drawNodes(ctx: CanvasRenderingContext2D, { view, overlay, palette: p, width, height }: Scene, dpr: number): void {
  const nodes = overlay.nodes;
  if (!nodes || (nodes.nodes.length === 0 && nodes.handles.length === 0)) return;
  const hover = nodes.hover;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.lineWidth = 1;
  ctx.strokeStyle = p.selection;
  ctx.fillStyle = p.workspace;
  ctx.beginPath();
  for (const h of nodes.handles) {
    const [nx, ny] = worldToScreen(view, [h.nx, h.ny]);
    const [hx, hy] = worldToScreen(view, [h.hx, h.hy]);
    ctx.moveTo(nx, ny);
    ctx.lineTo(hx, hy);
  }
  ctx.stroke();
  for (const h of nodes.handles) {
    const [hx, hy] = worldToScreen(view, [h.hx, h.hy]);
    const hot = hover?.id === h.id && hover.k === h.k && hover.handle === h.which;
    ctx.beginPath();
    ctx.arc(hx, hy, hot ? 5 : 3.5, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
  }
  // Primero los sueltos y luego los elegidos, para que queden encima.
  for (const pass of [false, true]) {
    for (const n of nodes.nodes) {
      if (n.selected !== pass) continue;
      const [sx, sy] = worldToScreen(view, [n.x, n.y]);
      if (sx < -10 || sy < -10 || sx > width + 10 || sy > height + 10) continue;
      const hot = hover?.id === n.id && hover.k === n.k && hover.handle === null;
      const r = hot ? 5.5 : 4.2;
      ctx.fillStyle = n.selected ? p.selection : p.workspace;
      ctx.strokeStyle = n.selected ? p.workspace : p.selection;
      ctx.lineWidth = 1.4;
      ctx.beginPath();
      if (n.type === 'smooth') ctx.arc(sx, sy, r, 0, Math.PI * 2);
      else if (n.type === 'symmetric') {
        ctx.moveTo(sx, sy - r - 1);
        ctx.lineTo(sx + r + 1, sy);
        ctx.lineTo(sx, sy + r + 1);
        ctx.lineTo(sx - r - 1, sy);
        ctx.closePath();
      } else ctx.rect(sx - r, sy - r, r * 2, r * 2);
      ctx.fill();
      ctx.stroke();
    }
  }
  ctx.restore();
}

// ---- superposiciones ------------------------------------------------------------

export function drawMarquee(ctx: CanvasRenderingContext2D, { view, overlay, palette: p }: Scene, dpr: number): void {
  const m = overlay.marquee;
  if (!m) return;
  const [ax, ay] = worldToScreen(view, m.a);
  const [bx, by] = worldToScreen(view, m.b);
  const x = Math.min(ax, bx);
  const y = Math.min(ay, by);
  const w = Math.abs(bx - ax);
  const h = Math.abs(by - ay);
  const crossing = m.mode === 'crossing';
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.globalAlpha = 0.1;
  ctx.fillStyle = crossing ? p.crossing : p.selection;
  ctx.fillRect(x, y, w, h);
  ctx.globalAlpha = 1;
  ctx.strokeStyle = crossing ? p.crossing : p.selection;
  ctx.lineWidth = 1;
  ctx.setLineDash(crossing ? [6, 4] : []); // continua = "dentro"; discontinua = "toca"
  ctx.strokeRect(Math.round(x) + 0.5, Math.round(y) + 0.5, w, h);
  ctx.restore();
}

export function drawGuides(ctx: CanvasRenderingContext2D, { view, overlay, palette: p }: Scene, dpr: number): void {
  if (overlay.guides.length === 0) return;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.strokeStyle = p.guide;
  ctx.lineWidth = 1;
  ctx.setLineDash([3, 3]);
  ctx.beginPath();
  for (const g of overlay.guides) {
    if (g.axis === 'x') {
      const [sx, sy0] = worldToScreen(view, [g.v, g.lo]);
      const [, sy1] = worldToScreen(view, [g.v, g.hi]);
      const x = Math.round(sx) + 0.5;
      ctx.moveTo(x, sy0 + 4);
      ctx.lineTo(x, sy1 - 4);
    } else {
      const [sx0, sy] = worldToScreen(view, [g.lo, g.v]);
      const [sx1] = worldToScreen(view, [g.hi, g.v]);
      const y = Math.round(sy) + 0.5;
      ctx.moveTo(sx0 - 4, y);
      ctx.lineTo(sx1 + 4, y);
    }
  }
  ctx.stroke();
  ctx.restore();
}

export function drawDraft(ctx: CanvasRenderingContext2D, { view, overlay, palette: p }: Scene, dpr: number): void {
  const d = overlay.draft;
  if (!d) return;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.strokeStyle = p.selection;
  ctx.fillStyle = p.workspace;
  ctx.lineWidth = 1.5;
  ctx.lineJoin = 'round';
  for (const path of d.paths) {
    // La vista es una afín: llevar los puntos de control a pantalla conserva las curvas.
    ctx.beginPath();
    for (const seg of path) {
      switch (seg.type) {
        case 'move_to': {
          const [x, y] = worldToScreen(view, seg.to);
          ctx.moveTo(x, y);
          break;
        }
        case 'line_to': {
          const [x, y] = worldToScreen(view, seg.to);
          ctx.lineTo(x, y);
          break;
        }
        case 'quad_to': {
          const [cx, cy] = worldToScreen(view, seg.ctrl);
          const [x, y] = worldToScreen(view, seg.to);
          ctx.quadraticCurveTo(cx, cy, x, y);
          break;
        }
        case 'cubic_to': {
          const [c1x, c1y] = worldToScreen(view, seg.ctrl1);
          const [c2x, c2y] = worldToScreen(view, seg.ctrl2);
          const [x, y] = worldToScreen(view, seg.to);
          ctx.bezierCurveTo(c1x, c1y, c2x, c2y, x, y);
          break;
        }
        case 'close':
          ctx.closePath();
          break;
      }
    }
    ctx.stroke();
  }
  if (d.rubber) {
    if (d.nodes) {
      // Curva en curso: un cuadrito en cada nodo puesto.
      for (const pt of d.nodes) {
        const [sx, sy] = worldToScreen(view, pt);
        ctx.fillRect(sx - 3, sy - 3, 6, 6);
        ctx.strokeRect(sx - 3, sy - 3, 6, 6);
      }
    } else {
      for (const path of d.paths) {
        // El último punto es el cursor: los vértices fijos llevan un cuadrito.
        const verts = path.flatMap((seg) => (seg.type === 'move_to' || seg.type === 'line_to' ? [seg.to] : []));
        verts.slice(0, -1).forEach((pt) => {
          const [sx, sy] = worldToScreen(view, pt);
          ctx.fillRect(sx - 3, sy - 3, 6, 6);
          ctx.strokeRect(sx - 3, sy - 3, 6, 6);
        });
      }
    }
  }
  if (d.grips) {
    // Agarraderas: cuadros llenos con el color de selección; la resaltada, más grande.
    ctx.lineWidth = 1.5;
    ctx.fillStyle = p.workspace;
    d.grips.forEach((pt, i) => {
      const [gx, gy] = worldToScreen(view, pt);
      const half = i === d.hotGrip ? 6 : 4.5;
      ctx.fillStyle = i === d.hotGrip ? p.selection : p.workspace;
      ctx.fillRect(gx - half, gy - half, half * 2, half * 2);
      ctx.strokeRect(gx - half, gy - half, half * 2, half * 2);
    });
  }
  if (d.handles) {
    // Manijas de Bézier: una línea fina del nodo a la manija y un punto en su extremo.
    ctx.lineWidth = 1;
    for (const [node, handle] of d.handles) {
      const [nx, ny] = worldToScreen(view, node);
      const [hx, hy] = worldToScreen(view, handle);
      ctx.beginPath();
      ctx.moveTo(nx, ny);
      ctx.lineTo(hx, hy);
      ctx.stroke();
      ctx.beginPath();
      ctx.arc(hx, hy, 3, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    }
  }
  ctx.restore();
}

/** Indicador del punto de ajuste: cuadro = nodo, triángulo = punto medio, círculo = centro, aspa = intersección. */
export function drawSnapMark(ctx: CanvasRenderingContext2D, { view, overlay, palette: p }: Scene, dpr: number): void {
  const m = overlay.snapMark;
  if (!m) return;
  const [x, y] = worldToScreen(view, m.p);
  const r = 6;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.strokeStyle = p.guide;
  ctx.fillStyle = p.workspace;
  ctx.lineWidth = 2;
  ctx.beginPath();
  switch (m.kind) {
    case 'node':
      ctx.rect(x - r, y - r, r * 2, r * 2);
      break;
    case 'mid':
      ctx.moveTo(x, y - r - 1);
      ctx.lineTo(x + r + 1, y + r);
      ctx.lineTo(x - r - 1, y + r);
      ctx.closePath();
      break;
    case 'center':
      ctx.arc(x, y, r, 0, Math.PI * 2);
      break;
    case 'inter':
      ctx.moveTo(x - r, y - r);
      ctx.lineTo(x + r, y + r);
      ctx.moveTo(x + r, y - r);
      ctx.lineTo(x - r, y + r);
      break;
    default:
      ctx.arc(x, y, 3, 0, Math.PI * 2);
  }
  ctx.stroke();
  ctx.font = '11px sans-serif';
  ctx.textBaseline = 'middle';
  ctx.fillStyle = p.guide;
  if (m.label) ctx.fillText(m.label, x + r + 6, y - r - 4);
  ctx.restore();
}

/** Línea de medida entre A y B con los catetos Δx y Δy, o el contorno de la forma medida. */
export function drawMeasure(ctx: CanvasRenderingContext2D, scene: Scene, dpr: number): void {
  const { view, overlay, palette: p, objects } = scene;
  const m = overlay.measure;
  if (!m) return;
  if (m.objectId) {
    const obj = objects.find((o) => o.id === m.objectId);
    if (obj) {
      ctx.save();
      ctx.lineJoin = 'round';
      ctx.lineCap = 'round';
      worldTransform(ctx, view, null, dpr);
      ctx.strokeStyle = p.selection;
      ctx.lineWidth = 3 / view.zoom;
      strokeGeom(ctx, getGeom(obj), null);
      ctx.restore();
    }
    return;
  }
  const [ax, ay] = worldToScreen(view, m.a);
  const [bx, by] = worldToScreen(view, m.b);
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.strokeStyle = p.selection;
  ctx.fillStyle = p.selection;
  ctx.lineWidth = 1;
  ctx.setLineDash([4, 4]);
  ctx.globalAlpha = 0.7;
  ctx.beginPath();
  ctx.moveTo(ax, ay);
  ctx.lineTo(bx, ay);
  ctx.lineTo(bx, by);
  ctx.stroke();
  ctx.setLineDash([]);
  ctx.globalAlpha = 1;
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.moveTo(ax, ay);
  ctx.lineTo(bx, by);
  ctx.stroke();
  for (const [x, y] of [[ax, ay], [bx, by]]) {
    ctx.beginPath();
    ctx.arc(x!, y!, 3.5, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.restore();
}

/** Rectángulo de encuadre de la vista previa, con overscan incluido. */
export function drawPreviewBounds(ctx: CanvasRenderingContext2D, { view, preview, palette: p }: Scene, dpr: number): void {
  if (!preview) return;
  const [[x0, y0], [x1, y1]] = preview.data.bounds;
  const [ax, ay] = worldToScreen(view, [x0, y1]);
  const [bx, by] = worldToScreen(view, [x1, y0]);
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.strokeStyle = p.overscan;
  ctx.lineWidth = 1;
  ctx.setLineDash([8, 5]);
  ctx.strokeRect(Math.round(ax) + 0.5, Math.round(ay) + 0.5, bx - ax, by - ay);
  ctx.restore();
}

/** Marcador de la posición del cabezal: mira y círculo, solo lectura. */
export function drawHead(ctx: CanvasRenderingContext2D, { view, head, palette: p }: Scene, dpr: number): void {
  if (!head) return;
  const [sx, sy] = worldToScreen(view, head);
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.strokeStyle = p.head;
  ctx.fillStyle = p.head;
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.arc(sx, sy, 7, 0, Math.PI * 2);
  ctx.moveTo(sx - 12, sy);
  ctx.lineTo(sx - 3, sy);
  ctx.moveTo(sx + 3, sy);
  ctx.lineTo(sx + 12, sy);
  ctx.moveTo(sx, sy - 12);
  ctx.lineTo(sx, sy - 3);
  ctx.moveTo(sx, sy + 3);
  ctx.lineTo(sx, sy + 12);
  ctx.stroke();
  ctx.beginPath();
  ctx.arc(sx, sy, 1.8, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}
