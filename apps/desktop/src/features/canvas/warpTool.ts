import type { Point } from '@/api/types';
import type { SnapSet } from './snap';
import { designReason, runWarpEnvelope } from '@/features/arrange/designOps';
import { blockBounds, selectionUnits } from '@/features/arrange/units';
import { t } from '@/i18n';
import { clearGesture, patchInteraction, useInteraction } from './interaction';
import type { Mods, ToolDeps } from './toolDeps';
import {
  envelopeOfSession, guidePaths, handlePoints, hitHandle, isIdentity, isValid, movePoint, newSession, previewPaths, previewSource,
  resetSession, setMode, warpOf, type PreviewSource, type WarpMode, type WarpSession,
} from './warpEdit';
import { patchWarpEdit, resetWarpEdit, useWarpEdit } from './warpEditStore';

/** Radio de captura de una manija, en px de pantalla. */
export const WARP_HIT_PX = 9;

interface Drag {
  pointerId: number;
  index: number;
  /** Sesión al empezar el arrastre: Esc vuelve a ella. */
  before: WarpSession;
  /** Posición de la manija al empezar, para Mayús (mover en horizontal o vertical). */
  from: Point;
  /** Blancos del imán (nodos, bordes y guías de lo que no se deforma), calculados al agarrar la manija. */
  targets: SnapSet;
}

/**
 * Modo de edición de Deformar en el lienzo. Con la herramienta activa y una selección deformable
 * aparece una malla de control (4 esquinas o 4x4 puntos) sobre la caja de la selección; arrastrar sus
 * manijas deforma el diseño en vivo (la vista previa se pinta como borrador sobre el lienzo, sin
 * tocar el documento) y Aplicar (Intro) lo escribe en un solo paso de deshacer. Esc o cambiar de
 * herramienta lo descarta.
 */
export function createWarpTool(d: ToolDeps) {
  let source: PreviewSource | null = null;
  let drag: Drag | null = null;
  /** Documento y selección para los que se armó la sesión: si cambian, se rehace. */
  let builtFor: { objects: unknown; ids: unknown } | null = null;
  /** Aplicando: el documento cambia por Aplicar y no hay que rehacer la sesión. */
  let applying = false;

  const session = (): WarpSession | null => useWarpEdit.getState().session;
  const setSession = (next: WarpSession) => patchWarpEdit({ session: next });

  /** Dibuja la deformación viva y la malla de control. */
  const show = () => {
    const s = session();
    if (!s || !source) {
      patchInteraction({ draft: null });
      return;
    }
    const warp = warpOf(s);
    const hot = useWarpEdit.getState().hot;
    const pts = handlePoints(s);
    patchInteraction({
      draft: {
        paths: warp ? [...previewPaths(source, warp), ...guidePaths(s, warp)] : [],
        grips: pts,
        hotGrip: hot,
      },
    });
  };

  /** (Re)arma la sesión a partir de la selección. */
  const begin = () => {
    drag = null;
    const state = d.st();
    builtFor = { objects: state.doc.objects, ids: state.selectedIds };
    const reason = designReason(state);
    if (reason !== true) {
      source = null;
      patchWarpEdit({ session: null, reason, hot: -1 });
      patchInteraction({ draft: null, tip: null, cursor: 'default', gesture: null });
      return;
    }
    const units = selectionUnits(state.doc, state.selectedIds);
    const bounds = blockBounds(units);
    if (!bounds || !(bounds[1][0] - bounds[0][0] > 1e-6) || !(bounds[1][1] - bounds[0][1] > 1e-6)) {
      source = null;
      patchWarpEdit({ session: null, reason: 'canvas.warp.flat', hot: -1 });
      patchInteraction({ draft: null, tip: null, cursor: 'default', gesture: null });
      return;
    }
    const prev = session();
    source = previewSource(units.flatMap((u) => u.objects), bounds);
    // Al volver a armarse (cambió el documento) se conserva el modo elegido.
    patchWarpEdit({ session: newSession(bounds, prev?.mode ?? 'bilinear'), reason: null, hot: -1 });
    patchInteraction({ cursor: 'default', gesture: null });
    show();
  };

  /** Si la selección o el documento cambiaron desde que se armó la sesión, la rehace. */
  const sync = () => {
    const state = d.st();
    if (!builtFor || builtFor.objects !== state.doc.objects || builtFor.ids !== state.selectedIds) begin();
  };

  const toolActive = () => d.st().activeTool === 'warp';

  const tipFor = (sx: number, sy: number, index: number) => {
    const s = session();
    if (!s) return;
    const p = handlePoints(s)[index];
    if (p) d.setTip(sx, sy, t('canvas.warp.tipPoint', { x: p[0].toFixed(2), y: p[1].toFixed(2) }));
  };

  /**
   * Mueve la manija agarrada a la posición del puntero. Se pega a la rejilla, a los nodos de los
   * demás objetos y a las guías con el mismo imán de las otras herramientas (Alt lo desactiva).
   * Mayús = solo en horizontal o vertical: el imán solo ajusta el eje libre.
   */
  const dragTo = (sx: number, sy: number, mods: Mods) => {
    const s = session();
    if (!drag || !s) return;
    let p = d.world(sx, sy);
    let axis: 0 | 1 | null = null;
    if (mods.shift) {
      const dx = Math.abs(p[0] - drag.from[0]);
      const dy = Math.abs(p[1] - drag.from[1]);
      axis = dx >= dy ? 1 : 0;
      p = axis === 1 ? [p[0], drag.from[1]] : [drag.from[0], p[1]];
    }
    const snapped = d.snapTo(p, drag.targets, mods);
    if (axis === null) p = snapped;
    else p = axis === 1 ? [snapped[0], drag.from[1]] : [drag.from[0], snapped[1]];
    setSession(movePoint(s, drag.index, p));
    show();
    tipFor(sx, sy, drag.index);
  };

  /** ¿Hay algo que aplicar? */
  const canApply = (): boolean => {
    const s = session();
    return !!s && isValid(s) && !isIdentity(s) && !useWarpEdit.getState().busy;
  };

  const hitAt = (sx: number, sy: number): number => {
    const s = session();
    return s ? hitHandle(s, d.world(sx, sy), WARP_HIT_PX / d.st().zoom) : -1;
  };

  return {
    /** Hay un arrastre de manija en curso. */
    active: () => drag !== null,

    /** Activar la herramienta o cambiar la selección: arma la sesión. */
    begin,

    hover(sx: number, sy: number): void {
      sync();
      const s = session();
      if (!s) {
        patchInteraction({ cursor: 'default', hoverId: null });
        return;
      }
      const index = hitAt(sx, sy);
      if (useWarpEdit.getState().hot !== index) {
        patchWarpEdit({ hot: index });
        show();
      }
      patchInteraction({ cursor: index >= 0 ? 'pointer' : 'default', hoverId: null });
      if (index >= 0) tipFor(sx, sy, index);
      else patchInteraction({ tip: null });
    },

    down(e: PointerEvent, sx: number, sy: number): void {
      sync();
      const s = session();
      if (!s) {
        d.st().notify('info', t(useWarpEdit.getState().reason ?? 'canvas.warp.tipSelect'));
        return;
      }
      const index = hitAt(sx, sy);
      if (index < 0) return;
      const from = handlePoints(s)[index]!;
      // Lo que se deforma no atrae: las manijas se pegan a lo demás del documento.
      const state = d.st();
      const moving = selectionUnits(state.doc, state.selectedIds).flatMap((u) => u.objects.map((o) => o.id));
      const targets = d.targetsFor(new Set(moving));
      drag = { pointerId: e.pointerId, index, before: s, from, targets };
      d.capture(e);
      patchWarpEdit({ hot: index });
      patchInteraction({ gesture: 'warp', cursor: 'move' });
      show();
    },

    move(e: PointerEvent, sx: number, sy: number, mods: Mods): boolean {
      if (!drag || e.pointerId !== drag.pointerId) return false;
      dragTo(sx, sy, mods);
      return true;
    },

    /** Repite el último dibujo (tras limpiar el borrador desde fuera). */
    redraw: show,

    /** Cambió el documento o la selección: rehace la sesión, salvo mientras se aplica. */
    refresh(): void {
      if (!applying) sync();
    },

    up(e: PointerEvent): boolean {
      if (!drag || e.pointerId !== drag.pointerId) return false;
      d.release(drag.pointerId);
      drag = null;
      patchInteraction({ gesture: null, cursor: 'pointer', tip: null, guides: [], snapMark: null });
      show();
      return true;
    },

    /** Esc a mitad de un arrastre: la manija vuelve a donde estaba. Sin arrastre no hace nada (la herramienta se deja con Esc → Seleccionar). */
    cancel(): void {
      if (drag) {
        d.release(drag.pointerId);
        setSession(drag.before);
        drag = null;
        patchInteraction({ guides: [], snapMark: null });
        show();
      }
      if (useInteraction.getState().gesture === 'warp') clearGesture();
    },

    /** Cambia de modo conservando la forma. */
    setMode(mode: WarpMode): void {
      const s = session();
      if (!s) return;
      setSession(setMode(s, mode));
      show();
    },

    reset(): void {
      const s = session();
      if (!s) return;
      setSession(resetSession(s));
      show();
    },

    canApply,

    /** Escribe la deformación en el documento (un solo paso de deshacer) y vuelve a Seleccionar. */
    async apply(): Promise<boolean> {
      const s = session();
      if (!s || !canApply()) return false;
      applying = true;
      patchWarpEdit({ busy: true });
      try {
        const ok = await runWarpEnvelope(d.st(), envelopeOfSession(s));
        if (ok) d.st().setTool('select');
        return ok;
      } finally {
        applying = false;
        patchWarpEdit({ busy: false });
      }
    },

    /** La herramienta dejó de estar activa o se cierra el lienzo. */
    end(): void {
      drag = null;
      source = null;
      builtFor = null;
      resetWarpEdit();
      if (!toolActive()) patchInteraction({ draft: null, tip: null });
    },
  };
}

export type WarpTool = ReturnType<typeof createWarpTool>;
