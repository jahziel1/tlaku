import type { ViewTransform } from '@/lib/viewport';
import type { LiveLayers } from './layers';
import type { LiveFrame } from './service';

/** Colores de la vista en vivo (se leen de los tokens CSS para seguir el tema). */
export interface LivePalette {
  /** El material sobre el que se «quema» (la cama de la simulación). */
  sheet: string;
  /** Fondo de la ventana de la vista en vivo, fuera de la cama. */
  workspace: string;
  reticle: string;
  laser: string;
}

export const FALLBACK_PALETTE: LivePalette = { sheet: '#d6be96', workspace: '#0d0f14', reticle: '#1b1b1b', laser: '#ff2d2d' };

export function readLivePalette(el: Element = document.documentElement): LivePalette {
  const css = getComputedStyle(el);
  const get = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
  return {
    sheet: get('--live-sheet', FALLBACK_PALETTE.sheet),
    workspace: get('--cv-workspace', FALLBACK_PALETTE.workspace),
    reticle: get('--live-reticle', FALLBACK_PALETTE.reticle),
    laser: get('--live-laser', FALLBACK_PALETTE.laser),
  };
}

/** Radio de la retícula en px de pantalla. */
export const RETICLE_PX = 8;

type HeadCtx = Pick<CanvasRenderingContext2D, 'save' | 'restore' | 'beginPath' | 'arc' | 'moveTo' | 'lineTo' | 'stroke' | 'fill' | 'createRadialGradient'> & {
  strokeStyle: CanvasRenderingContext2D['strokeStyle'];
  fillStyle: CanvasRenderingContext2D['fillStyle'];
  lineWidth: number;
  globalAlpha: number;
};

/** El cabezal: retícula; con el láser emitiendo, un punto rojo con brillo. En px de pantalla (no cambia con el zoom). */
export function drawHead(ctx: HeadCtx, sx: number, sy: number, emitting: boolean, palette: LivePalette): void {
  ctx.save();
  if (emitting) {
    const glow = ctx.createRadialGradient(sx, sy, 0, sx, sy, 16);
    glow.addColorStop(0, palette.laser);
    glow.addColorStop(1, 'rgba(255, 45, 45, 0)');
    ctx.globalAlpha = 0.65;
    ctx.fillStyle = glow;
    ctx.beginPath();
    ctx.arc(sx, sy, 16, 0, Math.PI * 2);
    ctx.fill();
    ctx.globalAlpha = 1;
  }
  // Contorno claro bajo la retícula para que se vea sobre el material y sobre lo quemado.
  for (const [color, width] of [['rgba(255, 255, 255, 0.85)', 3.5], [palette.reticle, 1.5]] as const) {
    ctx.strokeStyle = color;
    ctx.lineWidth = width;
    ctx.beginPath();
    ctx.arc(sx, sy, RETICLE_PX, 0, Math.PI * 2);
    ctx.moveTo(sx - RETICLE_PX - 6, sy);
    ctx.lineTo(sx - RETICLE_PX + 3, sy);
    ctx.moveTo(sx + RETICLE_PX - 3, sy);
    ctx.lineTo(sx + RETICLE_PX + 6, sy);
    ctx.moveTo(sx, sy - RETICLE_PX - 6);
    ctx.lineTo(sx, sy - RETICLE_PX + 3);
    ctx.moveTo(sx, sy + RETICLE_PX - 3);
    ctx.lineTo(sx, sy + RETICLE_PX + 6);
    ctx.stroke();
  }
  ctx.fillStyle = emitting ? palette.laser : palette.reticle;
  ctx.beginPath();
  ctx.arc(sx, sy, emitting ? 4 : 2, 0, Math.PI * 2);
  ctx.fill();
  if (emitting) {
    ctx.fillStyle = '#ffffff';
    ctx.beginPath();
    ctx.arc(sx, sy, 1.5, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.restore();
}

export interface LiveDrawArgs {
  ctx: CanvasRenderingContext2D;
  /** Tamaño en px CSS del lienzo. */
  width: number;
  height: number;
  view: ViewTransform;
  bed: readonly [number, number];
  layers: LiveLayers | null;
  frame: LiveFrame | null;
  palette: LivePalette;
}

/**
 * Un cuadro: el fondo, la cama (el material), lo pendiente y lo quemado (dos `drawImage`) y el cabezal.
 * Se dibuja en el lienzo PROPIO de la ventana de la vista en vivo, con su propia vista (`view`): no
 * toca ni tapa el lienzo del diseño.
 */
export function drawLive(a: LiveDrawArgs): void {
  const { ctx, view, bed } = a;
  ctx.clearRect(0, 0, a.width, a.height);
  ctx.fillStyle = a.palette.workspace;
  ctx.fillRect(0, 0, a.width, a.height);
  ctx.fillStyle = a.palette.sheet;
  ctx.fillRect(view.panX, view.panY - bed[1] * view.zoom, bed[0] * view.zoom, bed[1] * view.zoom);
  a.layers?.blit(ctx, view);
  if (a.frame) drawHead(ctx, view.panX + a.frame.x * view.zoom, view.panY - a.frame.y * view.zoom, a.frame.emitting, a.palette);
}
