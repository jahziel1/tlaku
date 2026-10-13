import type { Point } from '@/api/types';
import { previewTrimAt, runTrimAt } from '@/features/arrange/shapeOps';
import { useShapeOps } from '@/features/arrange/shapeOpsUi';
import { t } from '@/i18n';
import { polylineBez } from '@/lib/bezpath';
import { clearGesture, patchInteraction } from './interaction';
import type { ToolDeps } from './toolDeps';

/** Espera (ms) tras el último movimiento antes de pedir al motor qué tramo se borraría. */
export const TRIM_PREVIEW_MS = 70;
/** Distancia (px de pantalla) a la que un clic todavía cuenta como sobre el trazo. */
export const TRIM_HIT_PX = 8;

/**
 * Herramienta Recortar formas: con los objetos que se cruzan seleccionados, un clic sobre un tramo
 * entre dos cruces lo borra (`runTrimAt`; Mayús lo conserva aparte). Mientras el puntero pasa por el
 * trazo se resalta el tramo que se borraría (`previewTrimAt`). El último clic queda en
 * `useShapeOps.lastPoint`, que usa también el comando «Recortar en el último clic».
 */
export function createTrimTool(d: ToolDeps) {
  let timer: ReturnType<typeof setTimeout> | null = null;
  /** Cada petición lleva su número: solo vale la última (el motor responde fuera de orden). */
  let token = 0;

  const stop = () => {
    token++;
    if (timer) clearTimeout(timer);
    timer = null;
  };

  const showTip = (sx: number, sy: number, text: string) => patchInteraction({ tip: { x: sx + 16, y: sy + 18, text } });

  return {
    hover(sx: number, sy: number): void {
      stop();
      const s = d.st();
      patchInteraction({ cursor: 'crosshair', hoverId: null });
      if (s.selectedIds.length === 0) {
        patchInteraction({ draft: null });
        showTip(sx, sy, t('canvas.trim.tipSelect'));
        return;
      }
      showTip(sx, sy, t('canvas.trim.tip'));
      const mine = token;
      const p: Point = d.world(sx, sy);
      const maxDist = TRIM_HIT_PX / d.st().zoom;
      timer = setTimeout(() => {
        timer = null;
        void previewTrimAt(d.st(), p, maxDist).then((points) => {
          if (mine !== token) return;
          patchInteraction({ draft: points && points.length >= 2 ? { paths: [polylineBez(points, false)] } : null });
        });
      }, TRIM_PREVIEW_MS);
    },

    down(sx: number, sy: number, shift: boolean): void {
      stop();
      const s = d.st();
      if (s.selectedIds.length === 0) {
        s.notify('info', t('canvas.trim.tipSelect'));
        return;
      }
      const p = d.world(sx, sy);
      useShapeOps.getState().setLastPoint(p);
      patchInteraction({ draft: null });
      void runTrimAt(s, p, { maxDist: TRIM_HIT_PX / s.zoom, ...(shift ? { keepRemoved: true } : {}) });
    },

    leave(): void {
      stop();
      patchInteraction({ draft: null, tip: null });
    },

    cancel(): void {
      stop();
      clearGesture();
    },
  };
}
