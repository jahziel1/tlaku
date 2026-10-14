import { useEffect, type RefObject } from 'react';
import type { Point } from '@/api/types';
import { expandGroups } from '@/lib/groups';
import { t } from '@/i18n';
import { isEditableTarget } from '@/commands/shortcuts';
import { screenToWorld, type ViewTransform } from '@/lib/viewport';
import { useStore } from '@/store';
import { cursorForHandle, handleSpots, hitHandle } from './handles';
import { boxModeFor, HIT_PX, layerLookup, normalizeBox, pickObject, selectByBox, type PickContext } from './hit';
import { clearGesture, patchInteraction, registerGestureHooks, useInteraction } from './interaction';
import { constrainAngle } from './shapes';
import { openContextMenu } from '@/features/contextmenu/uiStore';
import { createGuideTool } from './guideTool';
import { createMarksTool } from './marksTool';
import { createMeasureTool } from './measureTool';
import { createNodeTool } from './nodeTool';
import { smoothNode } from './pen';
import { createPickTool } from './pickTool';
import { createPositionTool } from './positionTool';
import { createTrimTool } from './trimTool';
import { createFilletTool } from './filletTool';
import { createWarpTool } from './warpTool';
import { registerWarpActions } from './warpEditStore';
import { createTextAt, editText, prefetchFonts } from './textTool';
import { DRAG_PX, type Mods, type ToolDeps } from './toolDeps';
import { boundsOf, canRotate, isIdentity } from './transform';
import { dragKey, fmtMM, modsOf, type Gesture, type GestureCtx, type GestureState, type PointerInfo } from './gestureState';
import { createPolylineGesture } from './polylineGesture';
import { createShapeGesture } from './shapeGesture';
import { createSnapping } from './toolSnapping';
import { createTransformGesture } from './transformGesture';

/**
 * Interacción del lienzo con el ratón: selección, mover, escalar, rotar y herramientas de dibujo.
 * Cada gesto edita una copia viva (`useInteraction.live`) y escribe en el documento UNA sola vez al
 * soltar, así que un gesto es un solo paso de deshacer y Esc lo cancela sin dejar rastro.
 */
export function useCanvasTools(areaRef: RefObject<HTMLElement | null>, canvasRef: RefObject<HTMLCanvasElement | null>): void {
  useEffect(() => {
    const el = areaRef.current;
    const canvas = canvasRef.current;
    if (!el || !canvas) return;

    const st = () => useStore.getState();
    const viewOf = (): ViewTransform => {
      const s = st();
      return { zoom: s.zoom, panX: s.panX, panY: s.panY };
    };
    const local = (e: { clientX: number; clientY: number }): [number, number] => {
      const r = el.getBoundingClientRect();
      return [e.clientX - r.left, e.clientY - r.top];
    };
    const world = (sx: number, sy: number): Point => screenToWorld(viewOf(), [sx, sy]);
    const pickCtx = (): PickContext => ({ objects: st().doc.objects, layer: layerLookup(st().doc.layers) });

    const state: GestureState = { g: null, poly: null };
    let last: PointerInfo | null = null;
    let hoverFrame = 0;

    const snapping = createSnapping(st, viewOf);
    const { snapOpts, targetsFor, snapTo } = snapping;

    const setTip = (sx: number, sy: number, text: string) => patchInteraction({ tip: { x: sx + 16, y: sy + 18, text } });

    const capture = (e: PointerEvent) => {
      try {
        el.setPointerCapture(e.pointerId);
      } catch {
        /* el puntero ya no existe */
      }
    };
    const release = (id: number) => {
      try {
        if (el.hasPointerCapture(id)) el.releasePointerCapture(id);
      } catch {
        /* nada que liberar */
      }
    };

    // La herramienta de nodos lleva su propio gesto (ver nodeTool.ts); aquí solo se le pasan los eventos.
    const nodeTool = createNodeTool({
      el, st, viewOf, world, capture, release, snapOpts, targetsFor, setTip,
      pick: (p, tol) => pickObject(pickCtx(), p, tol),
    });

    const cancel = () => {
      if (state.g) release(state.g.pointerId);
      state.g = null;
      state.poly = null;
      nodeTool.cancel();
      measureTool.cancel();
      pickTool.cancel();
      guideTool.cancel();
      marksTool.cancel();
      trimTool.cancel();
      filletTool.cancel();
      warpTool.cancel();
      clearGesture();
      // Deformar guarda su malla entre gestos: se vuelve a dibujar tras limpiar el borrador.
      if (st().activeTool === 'warp') warpTool.redraw();
    };

    const toolDeps: ToolDeps = { el, st, viewOf, world, capture, release, snapOpts, targetsFor, setTip, snapTo };
    const ctx: GestureCtx = { ...toolDeps, markOf: snapping.markOf, KIND_LABEL: snapping.KIND_LABEL, applyPointSnap: snapping.applyPointSnap, pickCtx, state };
    const { selectedObjects, groupIds, isSelectableId, downSelect, updateMove, updateScale, updateRotate, startActive, commitMove } = createTransformGesture(ctx);
    const shape = createShapeGesture(ctx);
    const { downDraw, updateDraw, finishDraw } = shape;
    const { polyCursor, showPoly, finishPoly, downLine } = createPolylineGesture(ctx, shape);
    const measureTool = createMeasureTool(toolDeps);
    const pickTool = createPickTool(toolDeps);
    const positionTool = createPositionTool(toolDeps);
    const guideTool = createGuideTool(toolDeps);
    const marksTool = createMarksTool(toolDeps);
    const trimTool = createTrimTool(toolDeps);
    const filletTool = createFilletTool({ ...toolDeps, pick: (p, tol) => pickObject(pickCtx(), p, tol) });
    const warpTool = createWarpTool(toolDeps);
    const unregisterWarp = registerWarpActions({
      apply: () => warpTool.apply(),
      reset: () => warpTool.reset(),
      setMode: (m) => warpTool.setMode(m),
      canApply: () => warpTool.canApply(),
    });

    // ---- eventos del puntero -------------------------------------------------------------------------------------

    /** Herramienta de texto: un clic sobre un texto lo edita; sobre otra cosa, crea uno nuevo ahí. */
    const downText = (sx: number, sy: number) => {
      const s = st();
      const p = world(sx, sy);
      const hit = pickObject(pickCtx(), p, HIT_PX / s.zoom);
      if (hit?.kind.type === 'text') editText(s, hit.id);
      else createTextAt(s, p);
    };

    const onDown = (e: PointerEvent) => {
      if (e.target !== canvas) return;
      if (e.button === 2 && state.poly) {
        finishPoly(false);
        return;
      }
      // Clic derecho con un modo de elegir puntos abierto: lo cancela.
      if (e.button === 2 && useInteraction.getState().pick) {
        cancel();
        return;
      }
      if (e.button !== 0) return;
      const s = st();
      if (el.dataset.space === 'true' || s.activeTool === 'pan') return;
      const [sx, sy] = local(e);
      const mods = modsOf(e);
      last = { sx, sy, mods };
      // "Posicionar el láser": el clic solo manda la cabeza allí; no selecciona ni arrastra.
      if (positionTool.down(e, sx, sy, mods)) return;
      if (useInteraction.getState().pick) {
        pickTool.down(e, sx, sy, mods);
        return;
      }
      // Una guía del documento se agarra antes que cualquier objeto.
      if (guideTool.down(e, sx, sy)) return;
      switch (s.activeTool) {
        case 'tabs':
        case 'start':
          marksTool.down(sx, sy, mods);
          break;
        case 'trim':
          trimTool.down(sx, sy, mods.shift);
          break;
        case 'warp':
          warpTool.down(e, sx, sy);
          break;
        case 'fillet':
          filletTool.down(sx, sy, mods);
          break;
        case 'rect':
        case 'ellipse':
        case 'polygon':
        case 'star':
          downDraw(e, s.activeTool, sx, sy, mods);
          break;
        case 'line':
          downLine(e, sx, sy, mods);
          break;
        case 'measure':
          measureTool.down(e, sx, sy, mods);
          break;
        case 'nodes':
          nodeTool.down(e, sx, sy, mods);
          break;
        case 'text':
          downText(sx, sy);
          break;
        default:
          downSelect(e, sx, sy, mods);
      }
    };

    const update = (sx: number, sy: number, mods: Mods) => {
      if (!state.g) return;
      if (!state.g.active) {
        if (Math.hypot(sx - state.g.startScreen[0], sy - state.g.startScreen[1]) < DRAG_PX) return;
        startActive(state.g);
      }
      const gs = state.g as Gesture | null;
      if (!gs) return;
      switch (gs.kind) {
        case 'move': updateMove(gs, sx, sy, mods); break;
        case 'scale': updateScale(gs, sx, sy, mods); break;
        case 'rotate': updateRotate(gs, sx, sy, mods); break;
        case 'marquee': {
          const mode = boxModeFor(gs.startScreen[0], sx);
          patchInteraction({ marquee: { a: gs.startWorld, b: world(sx, sy), mode } });
          break;
        }
        case 'draw': updateDraw(gs, sx, sy, mods); break;
        case 'pen': {
          if (!state.poly) break;
          const node = state.poly.nodes[gs.index];
          if (!node) break;
          let ptr = world(sx, sy);
          if (mods.shift) ptr = constrainAngle(node.p, ptr);
          state.poly.nodes[gs.index] = smoothNode(node.p, ptr);
          showPoly(null);
          setTip(sx, sy, t('canvas.tip.handle', { len: fmtMM(Math.hypot(ptr[0] - node.p[0], ptr[1] - node.p[1])) }));
          break;
        }
        default: break;
      }
    };

    const hoverAt = (sx: number, sy: number) => {
      const s = st();
      const tool = s.activeTool;
      if (tool === 'pan') {
        patchInteraction({ cursor: 'grab', hoverId: null });
        return;
      }
      if (tool === 'rect' || tool === 'ellipse' || tool === 'line' || tool === 'polygon' || tool === 'star') {
        patchInteraction({ cursor: 'crosshair', hoverId: null });
        return;
      }
      if (tool === 'measure') {
        // Resalta la forma bajo el cursor: un clic muestra sus datos.
        const hit = pickObject(pickCtx(), world(sx, sy), HIT_PX / s.zoom);
        patchInteraction({ cursor: 'crosshair', hoverId: hit?.id ?? null });
        return;
      }
      if (tool === 'text') {
        patchInteraction({ cursor: 'text', hoverId: null });
        return;
      }
      if (tool === 'tabs' || tool === 'start') {
        marksTool.hover(sx, sy);
        return;
      }
      if (tool === 'trim') {
        trimTool.hover(sx, sy);
        return;
      }
      if (tool === 'warp') {
        warpTool.hover(sx, sy);
        return;
      }
      if (tool === 'fillet') {
        filletTool.hover(sx, sy, last?.mods ?? { shift: false, alt: false, ctrl: false });
        return;
      }
      if (guideTool.hover(sx, sy)) return;
      if (tool === 'nodes') {
        patchInteraction({ hoverId: null });
        nodeTool.hover(sx, sy);
        return;
      }
      const inter = useInteraction.getState();
      const objs = selectedObjects();
      const frame = boundsOf(objs);
      if (frame) {
        const spot = hitHandle(handleSpots(frame, viewOf(), { rotateMode: inter.rotateMode, canRotate: canRotate(objs) }), sx, sy);
        if (spot) {
          patchInteraction({ cursor: cursorForHandle(spot), hoverId: null });
          return;
        }
      }
      const hit = pickObject(pickCtx(), world(sx, sy), HIT_PX / s.zoom);
      patchInteraction({ hoverId: hit?.id ?? null, cursor: hit ? (s.selectedIds.includes(hit.id) ? 'move' : 'pointer') : 'default' });
    };

    const onMove = (e: PointerEvent) => {
      const [sx, sy] = local(e);
      const mods = modsOf(e);
      last = { sx, sy, mods };
      if (positionTool.move(e, sx, sy, mods)) return;
      if (nodeTool.move(e, sx, sy, mods)) return;
      if (warpTool.move(e, sx, sy, mods)) return;
      if (pickTool.move(e, sx, sy, mods)) return;
      if (measureTool.move(e, sx, sy, mods)) return;
      if (guideTool.move(e, sx, sy, mods)) return;
      if (state.g) {
        if (e.pointerId === state.g.pointerId) update(sx, sy, mods);
        return;
      }
      if (state.poly) {
        const p = polyCursor(sx, sy, mods);
        showPoly(p, sx, sy);
        return;
      }
      if (e.buttons !== 0) return;
      // El hover cuesta un hit-test: como máximo uno por cuadro.
      if (!hoverFrame) {
        hoverFrame = requestAnimationFrame(() => {
          hoverFrame = 0;
          if (last && !state.g && !nodeTool.active() && !guideTool.active() && !warpTool.active()) hoverAt(last.sx, last.sy);
        });
      }
    };

    const onUp = (e: PointerEvent) => {
      if (warpTool.up(e)) return;
      if (nodeTool.active()) {
        const [nx, ny] = local(e);
        nodeTool.up(e, nx, ny, modsOf(e));
        return;
      }
      if (pickTool.up(e, ...local(e), modsOf(e))) return;
      if (measureTool.up(e, ...local(e), modsOf(e))) return;
      if (guideTool.up(e, ...local(e), modsOf(e))) return;
      if (!state.g || e.pointerId !== state.g.pointerId) return;
      const gs = state.g;
      release(gs.pointerId);
      const [sx, sy] = local(e);
      const mods = modsOf(e);
      if (gs.active) update(sx, sy, mods); // posición final exacta
      state.g = null;
      const s = st();
      const cur = gs as Gesture;
      switch (cur.kind) {
        case 'press': {
          if (!cur.active) {
            // Clic sin arrastre.
            const picked = mods.alt ? [cur.hit.id] : groupIds(cur.hit.id);
            if (cur.shift && cur.wasSelected) s.select(s.selectedIds.filter((id) => !picked.includes(id)));
            else if (!cur.shift && cur.wasSelected) {
              if (s.selectedIds.length === 1) patchInteraction({ rotateMode: !useInteraction.getState().rotateMode });
              else s.select(picked);
            }
          }
          break;
        }
        case 'move': commitMove(cur); break;
        case 'scale':
          if (cur.active && !isIdentity(cur.matrix)) s.transformObjects(cur.ids, cur.matrix, dragKey());
          break;
        case 'rotate':
          if (cur.active && !isIdentity(cur.matrix)) s.transformObjects(cur.ids, cur.matrix, dragKey());
          break;
        case 'marquee':
          if (!cur.active) {
            if (!cur.shift) {
              s.clearSelection();
              patchInteraction({ rotateMode: false });
            }
          } else {
            const mode = boxModeFor(cur.startScreen[0], sx);
            const found = selectByBox(pickCtx(), normalizeBox(cur.startWorld, world(sx, sy)), mode);
            const ids = expandGroups(s.doc.objects, found, isSelectableId);
            s.select(cur.shift ? [...s.selectedIds, ...ids] : ids);
          }
          break;
        case 'draw': finishDraw(cur, mods); return;
        case 'pen':
          // El nodo ya está puesto (con sus manijas si se arrastró): la curva sigue abierta.
          if (state.poly) showPoly(polyCursor(sx, sy, mods), sx, sy);
          return;
        default: break;
      }
      clearGesture();
    };

    const onLeave = () => {
      if (hoverFrame) {
        cancelAnimationFrame(hoverFrame);
        hoverFrame = 0;
      }
      if (!state.g) patchInteraction({ hoverId: null });
      if (st().activeTool === 'tabs' || st().activeTool === 'start') marksTool.leave();
      if (st().activeTool === 'trim') trimTool.leave();
      if (st().activeTool === 'fillet') filletTool.leave();
      if (positionTool.active()) patchInteraction({ tip: null });
    };

    // ---- teclado ------------------------------------------------------------------------------------------------------

    // Mientras hay una polilínea, Enter la termina y Retroceso quita el último punto, sin importar
    // dónde esté el foco. Va en fase de captura para anticiparse al comando "Eliminar".
    const onKeyCapture = (e: KeyboardEvent) => {
      if (!state.poly || isEditableTarget(e.target)) return;
      if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        finishPoly(false);
      } else if (e.key === 'Backspace') {
        e.preventDefault();
        state.poly.nodes.pop();
        if (state.poly.nodes.length === 0) cancel();
        else if (last) showPoly(polyCursor(last.sx, last.sy, last.mods), last.sx, last.sy);
      }
    };

    // Intro aplica la deformación del lienzo. No es un atajo del registro: atraparía Intro en cualquier botón.
    const onWarpKey = (e: KeyboardEvent) => {
      if (e.key !== 'Enter' || e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey || st().activeTool !== 'warp') return;
      const target = e.target as HTMLElement | null;
      if (isEditableTarget(e.target) || target?.closest?.('button, a, select, summary, [role="button"]')) return;
      e.preventDefault();
      void warpTool.apply();
    };

    // Cambiar Mayús / Alt / Ctrl a mitad de un gesto actualiza el resultado sin mover el ratón.
    const onModifier = (e: KeyboardEvent) => {
      if (!['Shift', 'Alt', 'Control', 'Meta'].includes(e.key) || !last) return;
      const mods = modsOf(e);
      last = { ...last, mods };
      if (st().activeTool === 'fillet') filletTool.hover(last.sx, last.sy, mods);
      if (nodeTool.active()) nodeTool.refresh(last.sx, last.sy, mods);
      else if (state.g?.active) update(last.sx, last.sy, mods);
      else if (state.poly) showPoly(polyCursor(last.sx, last.sy, mods), last.sx, last.sy);
    };

    // Clic derecho: la polilínea en curso se termina (lo hace pointerdown); un modo de elegir puntos se
    // cancela; en cualquier otro caso se abre el menú contextual, con el objeto de abajo seleccionado.
    const onContextMenu = (e: MouseEvent) => {
      e.preventDefault();
      if (state.poly || state.g || useInteraction.getState().pick || nodeTool.active()) return;
      const [sx, sy] = local(e);
      const s = st();
      const at = world(sx, sy);
      const hit = pickObject(pickCtx(), at, HIT_PX / s.zoom);
      if (hit && !s.selectedIds.includes(hit.id)) s.select(groupIds(hit.id));
      openContextMenu({ x: e.clientX, y: e.clientY, at, onObject: hit !== null || s.selectedIds.length > 0 });
    };

    // Doble clic: con nodos, agrega un nodo en el segmento; con Seleccionar, sobre un texto pasa a editarlo.
    const onDoubleClick = (e: MouseEvent) => {
      // Tras capturar el puntero, el navegador dirige el doble clic al área y no al lienzo.
      if (e.target !== canvas && e.target !== el) return;
      const s = st();
      const [sx, sy] = local(e);
      if (guideTool.doubleClick(sx, sy)) return;
      if (s.activeTool === 'nodes') {
        nodeTool.doubleClick(sx, sy);
      } else if (s.activeTool === 'select') {
        const hit = pickObject(pickCtx(), world(sx, sy), HIT_PX / s.zoom);
        if (hit?.kind.type === 'text') {
          editText(s, hit.id);
          s.setTool('text');
        }
      }
    };

    el.addEventListener('pointerdown', onDown);
    el.addEventListener('pointermove', onMove);
    el.addEventListener('pointerup', onUp);
    el.addEventListener('pointercancel', cancel);
    el.addEventListener('pointerleave', onLeave);
    el.addEventListener('contextmenu', onContextMenu);
    el.addEventListener('dblclick', onDoubleClick);
    window.addEventListener('keydown', onKeyCapture, true);
    window.addEventListener('keydown', onWarpKey);
    window.addEventListener('keydown', onModifier);
    window.addEventListener('keyup', onModifier);
    window.addEventListener('blur', cancel);

    const unhook = registerGestureHooks({ cancel, closePolyline: () => state.poly && finishPoly(true) });

    // Cambiar de herramienta o de selección deshace lo pendiente.
    const unsub = useStore.subscribe((s, prev) => {
      if (s.activeTool === 'warp' && s.activeTool === prev.activeTool && (s.selectedIds !== prev.selectedIds || s.doc.objects !== prev.doc.objects)) {
        warpTool.refresh();
      }
      if (s.activeTool !== prev.activeTool) {
        cancel();
        if (prev.activeTool === 'warp') warpTool.end();
        if (s.activeTool === 'warp') warpTool.begin();
        patchInteraction({ hoverId: null, nodeHover: null, measure: null, markHover: null, guideHover: null, tip: null });
        if (s.activeTool === 'text') prefetchFonts();
      }
      if (s.selectedIds !== prev.selectedIds && useInteraction.getState().rotateMode) {
        const same = s.selectedIds.length === prev.selectedIds.length && s.selectedIds.every((id) => prev.selectedIds.includes(id));
        if (!same) patchInteraction({ rotateMode: false });
      }
    });

    // Al activar o apagar "Posicionar el láser" cambia el cursor (mira) y se limpia la etiqueta.
    const unsubPosition = useStore.subscribe((s, prev) => {
      if (s.positioningLaser === prev.positioningLaser) return;
      patchInteraction({ cursor: s.positioningLaser ? 'crosshair' : 'default', hoverId: null, tip: null });
    });
    if (st().positioningLaser) patchInteraction({ cursor: 'crosshair' });

    // El cursor CSS sigue al estado de la interacción.
    const syncCursor = (c: string) => {
      el.dataset.cursor = c;
    };
    syncCursor(useInteraction.getState().cursor);
    const unsubCursor = useInteraction.subscribe((s, prev) => {
      if (s.cursor !== prev.cursor) syncCursor(s.cursor);
    });

    return () => {
      el.removeEventListener('pointerdown', onDown);
      el.removeEventListener('pointermove', onMove);
      el.removeEventListener('pointerup', onUp);
      el.removeEventListener('pointercancel', cancel);
      el.removeEventListener('pointerleave', onLeave);
      el.removeEventListener('contextmenu', onContextMenu);
      el.removeEventListener('dblclick', onDoubleClick);
      window.removeEventListener('keydown', onKeyCapture, true);
      window.removeEventListener('keydown', onWarpKey);
      window.removeEventListener('keydown', onModifier);
      window.removeEventListener('keyup', onModifier);
      window.removeEventListener('blur', cancel);
      if (hoverFrame) cancelAnimationFrame(hoverFrame);
      unhook();
      unsub();
      unsubCursor();
      unsubPosition();
      unregisterWarp();
      cancel();
      warpTool.end();
    };
  }, [areaRef, canvasRef]);
}
