import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { patchInteraction } from './interaction';
import type { Mods, ToolDeps } from './toolDeps';

/**
 * Modo "Posicionar el láser": mientras `positioningLaser` está activo, cada clic en el lienzo manda
 * la cabeza a ese punto (mm del documento, Y arriba) con el láser apagado. No selecciona, no arrastra
 * y no cambia el documento; el almacén valida el punto y avisa si cae fuera de la cama.
 */
export function createPositionTool(d: ToolDeps) {
  const on = () => d.st().positioningLaser;

  return {
    active: on,

    /** Devuelve `true` si consumió el clic (el modo está activo). */
    down(_e: PointerEvent, sx: number, sy: number, _mods: Mods): boolean {
      if (!on()) return false;
      const [x, y] = d.world(sx, sy);
      void d.st().positionLaserAt(x, y);
      return true;
    },

    /** Mantiene el cursor de mira y muestra a qué punto iría la cabeza. */
    move(_e: PointerEvent, sx: number, sy: number, _mods: Mods): boolean {
      if (!on()) return false;
      const [x, y] = d.world(sx, sy);
      patchInteraction({ cursor: 'crosshair', hoverId: null });
      d.setTip(sx, sy, t('canvas.position.tip', { x: fmt(x, 1), y: fmt(y, 1) }));
      return true;
    },
  };
}
