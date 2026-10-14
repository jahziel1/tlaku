import type { ResumeView } from '@/api/types';

/** Colores del dibujo (se leen del CSS para que sigan el tema claro u oscuro). */
export interface ResumePalette {
  done: string;
  todo: string;
  travel: string;
  marker: string;
  bed: string;
}

export interface Fit {
  scale: number;
  ox: number;
  oy: number;
  height: number;
}

/** Ajusta la caja del trabajo (mm, Y arriba) a `w` × `h` píxeles con margen `pad`. */
export function fitBounds(bounds: [[number, number], [number, number]], w: number, h: number, pad = 16): Fit {
  const bw = Math.max(bounds[1][0] - bounds[0][0], 1e-6);
  const bh = Math.max(bounds[1][1] - bounds[0][1], 1e-6);
  const scale = Math.max(1e-6, Math.min((w - pad * 2) / bw, (h - pad * 2) / bh));
  const ox = (w - bw * scale) / 2 - bounds[0][0] * scale;
  const oy = (h - bh * scale) / 2 - bounds[0][1] * scale;
  return { scale, ox, oy, height: h };
}

/** Milímetros -> píxeles (la Y se invierte: en el documento crece hacia arriba). */
export function project(fit: Fit, x: number, y: number): [number, number] {
  return [fit.ox + x * fit.scale, fit.height - (fit.oy + y * fit.scale)];
}

/** Píxeles -> milímetros. */
export function unproject(fit: Fit, px: number, py: number): [number, number] {
  return [(px - fit.ox) / fit.scale, (fit.height - py - fit.oy) / fit.scale];
}

type Ctx = Pick<CanvasRenderingContext2D, 'save' | 'restore' | 'clearRect' | 'beginPath' | 'moveTo' | 'lineTo' | 'stroke' | 'setLineDash' | 'arc' | 'fill' | 'fillRect'> & {
  strokeStyle: CanvasRenderingContext2D['strokeStyle'];
  fillStyle: CanvasRenderingContext2D['fillStyle'];
  lineWidth: number;
};

/**
 * Dibuja el trabajo guardado: lo hecho (los movimientos anteriores al punto de reinicio) en trazo continuo, lo
 * que falta en trazo discontinuo, los desplazamientos muy tenues y el punto de reinicio marcado. Devuelve la
 * transformación usada (para elegir el punto con el ratón).
 */
export function drawResume(ctx: Ctx, w: number, h: number, view: ResumeView, restartMove: number, restartAt: [number, number] | null, palette: ResumePalette): Fit {
  const fit = fitBounds(view.bounds, w, h);
  ctx.save();
  ctx.clearRect(0, 0, w, h);
  ctx.fillStyle = palette.bed;
  ctx.fillRect(0, 0, w, h);
  const m = view.moves;
  const n = m.length / 4;
  const lines = (from: number, to: number, kind: 'cut' | 'travel') => {
    ctx.beginPath();
    for (let i = Math.max(1, from); i < to; i++) {
      const isCut = m[i * 4] === 1;
      if (isCut !== (kind === 'cut')) continue;
      const [x0, y0] = project(fit, m[(i - 1) * 4 + 1]!, m[(i - 1) * 4 + 2]!);
      const [x1, y1] = project(fit, m[i * 4 + 1]!, m[i * 4 + 2]!);
      ctx.moveTo(x0, y0);
      ctx.lineTo(x1, y1);
    }
    ctx.stroke();
  };
  const split = Math.min(Math.max(restartMove, 1), n);
  ctx.lineWidth = 0.75;
  ctx.setLineDash([]);
  ctx.strokeStyle = palette.travel;
  lines(1, n, 'travel');
  ctx.lineWidth = 2;
  ctx.strokeStyle = palette.done;
  lines(1, split, 'cut');
  ctx.setLineDash([5, 4]);
  ctx.strokeStyle = palette.todo;
  lines(split, n, 'cut');
  ctx.setLineDash([]);
  // Punto de reinicio: el inicio del movimiento elegido (o el punto a mitad de línea).
  const p = restartAt ?? [m[(split - 1) * 4 + 1] ?? 0, m[(split - 1) * 4 + 2] ?? 0];
  const [cx, cy] = project(fit, p[0]!, p[1]!);
  ctx.strokeStyle = palette.marker;
  ctx.fillStyle = palette.marker;
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.arc(cx, cy, 6, 0, Math.PI * 2);
  ctx.stroke();
  ctx.beginPath();
  ctx.arc(cx, cy, 2, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
  return fit;
}
