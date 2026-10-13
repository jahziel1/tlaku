import type { GuideAxis } from '@/api/types';
import { t } from '@/i18n';
import { formatLength } from '@/lib/units';
import { getPref, useStore } from '@/store';
import {
  guideAt, guideAxisForRuler, guideValueFromScreen, outsideCanvas, roundGuide, snapGuidePos, useGuideUi,
} from './guides';
import { clearGesture, patchInteraction, useInteraction } from './interaction';
import { DRAG_PX, type Mods, type ToolDeps } from './toolDeps';

const posText = (axis: GuideAxis, pos: number): string =>
  t(axis === 'x' ? 'canvas.guide.tipX' : 'canvas.guide.tipY', { pos: formatLength(pos, getPref('units'), { digits: 2, trim: true }) });

interface Drag {
  pointerId: number;
  index: number;
  axis: GuideAxis;
  startScreen: [number, number];
  active: boolean;
}

/**
 * Guías ya creadas, sobre el lienzo (herramienta Seleccionar): pasar el puntero las resalta,
 * arrastrarlas las mueve (con ajuste magnético; Alt lo suspende), soltarlas fuera del lienzo o
 * sobre una regla las borra y el doble clic abre el diálogo de posición exacta.
 */
export function createGuideTool(d: ToolDeps) {
  let drag: Drag | null = null;

  const visible = () => {
    const s = d.st();
    return s.showGuides && s.activeTool === 'select';
  };

  const indexAt = (sx: number, sy: number): number => {
    const s = d.st();
    return guideAt(s.doc.guides ?? [], d.viewOf(), sx, sy);
  };

  const live = (g: Drag, sx: number, sy: number, mods: Mods) => {
    const s = d.st();
    const raw = guideValueFromScreen(g.axis, d.viewOf(), sx, sy);
    const remove = outsideCanvas(s.viewport.width, s.viewport.height, sx, sy);
    const pos = snapGuidePos(s, g.axis, raw, g.index, mods.alt || mods.ctrl);
    patchInteraction({ guideDrag: { index: g.index, axis: g.axis, pos, remove } });
    d.setTip(sx, sy, remove ? t('canvas.guide.tipRemove') : posText(g.axis, pos));
    return { pos, remove };
  };

  return {
    active: () => drag !== null,

    /** Resalta la guía bajo el puntero y cambia el cursor. `true` si hay una. */
    hover(sx: number, sy: number): boolean {
      const i = visible() ? indexAt(sx, sy) : -1;
      const g = i >= 0 ? d.st().doc.guides?.[i] : undefined;
      if (g) {
        patchInteraction({ guideHover: i, cursor: g.axis === 'x' ? 'ew-resize' : 'ns-resize', hoverId: null });
        return true;
      }
      if (useInteraction.getState().guideHover !== null) patchInteraction({ guideHover: null });
      return false;
    },

    down(e: PointerEvent, sx: number, sy: number): boolean {
      if (!visible()) return false;
      const i = indexAt(sx, sy);
      const g = d.st().doc.guides?.[i];
      if (!g) return false;
      drag = { pointerId: e.pointerId, index: i, axis: g.axis, startScreen: [sx, sy], active: false };
      d.capture(e);
      return true;
    },

    move(e: PointerEvent, sx: number, sy: number, mods: Mods): boolean {
      if (!drag || e.pointerId !== drag.pointerId) return false;
      if (!drag.active) {
        if (Math.hypot(sx - drag.startScreen[0], sy - drag.startScreen[1]) < DRAG_PX) return true;
        drag.active = true;
        patchInteraction({ gesture: 'guide', hoverId: null });
      }
      live(drag, sx, sy, mods);
      return true;
    },

    up(e: PointerEvent, sx: number, sy: number, mods: Mods): boolean {
      if (!drag || e.pointerId !== drag.pointerId) return false;
      const g = drag;
      drag = null;
      d.release(g.pointerId);
      if (g.active) {
        const { pos, remove } = live(g, sx, sy, mods);
        const s = d.st();
        if (remove) s.removeGuide(g.index);
        else s.updateGuide(g.index, { pos }, `guide:${g.index}:${Date.now()}`);
      }
      clearGesture();
      patchInteraction({ guideHover: null });
      return true;
    },

    doubleClick(sx: number, sy: number): boolean {
      if (!visible()) return false;
      const i = indexAt(sx, sy);
      if (i < 0) return false;
      useGuideUi.getState().show(i);
      return true;
    },

    cancel() {
      if (drag) d.release(drag.pointerId);
      drag = null;
      patchInteraction({ guideDrag: null });
    },
  };
}

// ---- crear guías desde las reglas ---------------------------------------------------------------

/** Posición de pantalla, relativa al área del lienzo, de un evento sobre una regla. */
export function rulerToArea(ruler: 'x' | 'y', rect: DOMRect, e: { clientX: number; clientY: number }): [number, number] {
  return ruler === 'x' ? [e.clientX - rect.left, e.clientY - rect.bottom] : [e.clientX - rect.right, e.clientY - rect.top];
}

/**
 * Arrastre que nace en una regla: al soltar dentro del lienzo crea la guía; si se suelta sobre la
 * regla o fuera del lienzo, se cancela y no queda nada.
 */
export function createRulerDrag(ruler: 'x' | 'y') {
  const axis = guideAxisForRuler(ruler);
  /** Cuánto hay que alejarse de la regla (px) para que cuente como "dentro del lienzo". */
  const INSIDE_PX = 4;

  const state = (rect: DOMRect, e: { clientX: number; clientY: number }, suspend: boolean) => {
    const s = useStore.getState();
    const [sx, sy] = rulerToArea(ruler, rect, e);
    const perpendicular = ruler === 'x' ? sy : sx;
    const along = ruler === 'x' ? sx : sy;
    const limit = ruler === 'x' ? s.viewport.width : s.viewport.height;
    const across = ruler === 'x' ? s.viewport.height : s.viewport.width;
    const cancel = perpendicular < INSIDE_PX || perpendicular > across || along < 0 || along > limit;
    const view = { zoom: s.zoom, panX: s.panX, panY: s.panY };
    const pos = snapGuidePos(s, axis, guideValueFromScreen(axis, view, sx, sy), null, suspend);
    return { pos, cancel };
  };

  return {
    move(rect: DOMRect, e: { clientX: number; clientY: number; altKey: boolean }) {
      const { pos, cancel } = state(rect, e, e.altKey);
      patchInteraction({ gesture: 'guide', guideDrag: { index: null, axis, pos, remove: cancel } });
      return { pos, cancel };
    },
    finish(rect: DOMRect, e: { clientX: number; clientY: number; altKey: boolean }) {
      const { pos, cancel } = state(rect, e, e.altKey);
      clearGesture();
      if (cancel) return;
      useStore.getState().addGuide({ axis, pos: roundGuide(pos) });
    },
    cancel() {
      clearGesture();
    },
  };
}
