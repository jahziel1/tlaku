import type { DesignObject, Point } from '@/api/types';
import { t } from '@/i18n';
import { isSelectable, HIT_PX, layerLookup, pickObject } from './hit';
import { nearestOnContour, polyPoints } from './contour';
import { getGeom } from './geom';
import { clearGesture, patchInteraction } from './interaction';
import { clearStart, reverseDirection, setStartPoint, tabIndexNear, toggleTab } from './marksOps';
import type { Mods, ToolDeps } from './toolDeps';

/** Puntos de un objeto a los que se pega el inicio: sus vértices. */
function nearestVertex(obj: DesignObject, p: Point, tol: number): Point | null {
  let best: Point | null = null;
  let bestD = tol;
  for (const poly of getGeom(obj).polys) {
    if (!poly.closed) continue;
    for (const v of polyPoints(poly)) {
      const d = Math.hypot(v[0] - p[0], v[1] - p[1]);
      if (d <= bestD) {
        best = v;
        bestD = d;
      }
    }
  }
  return best;
}

/**
 * Herramientas de un clic sobre el contorno:
 *  - Puentes (`tabs`): un clic sobre el contorno de un objeto pone un puente; un clic sobre un
 *    puente ya puesto lo quita. Se guardan en `manual_tabs`.
 *  - Inicio (`start`): un clic sobre un contorno cerrado fija ahí su punto de inicio (se pega al
 *    vértice más cercano; Alt lo suspende). Mayús+clic invierte el sentido del objeto y Ctrl/Cmd+clic
 *    quita lo fijado. Se guarda en `start`.
 */
export function createMarksTool(d: ToolDeps) {
  /** Objeto con contorno cerrado bajo el puntero y el punto del contorno al que apunta. */
  const target = (sx: number, sy: number): { obj: DesignObject; point: Point; tol: number } | null => {
    const s = d.st();
    const tol = HIT_PX / s.zoom;
    const p = d.world(sx, sy);
    const layer = layerLookup(s.doc.layers);
    // El de más arriba cuyo contorno esté a la vista del puntero (un relleno no cuenta: hay que apuntar al trazo).
    for (let i = s.doc.objects.length - 1; i >= 0; i--) {
      const obj = s.doc.objects[i]!;
      if (obj.kind.type === 'image' || !isSelectable(obj, layer(obj.layer))) continue;
      const hit = nearestOnContour(obj, p, true);
      if (hit && hit.dist <= tol) return { obj, point: hit.point, tol };
    }
    // Un objeto bajo el puntero pero sin contorno cerrado cerca: no hay dónde poner nada.
    return null;
  };

  const tool = () => d.st().activeTool;

  return {
    hover(sx: number, sy: number): void {
      const hit = target(sx, sy);
      if (!hit) {
        const under = pickObject({ objects: d.st().doc.objects, layer: layerLookup(d.st().doc.layers) }, d.world(sx, sy), HIT_PX / d.st().zoom);
        patchInteraction({ cursor: 'crosshair', markHover: null, hoverId: null, tip: under ? { x: sx + 16, y: sy + 18, text: t('canvas.marks.notClosed') } : null });
        return;
      }
      const onTab = tool() === 'tabs' && tabIndexNear(hit.obj.manual_tabs ?? [], hit.point, markReach(d, hit.obj)) >= 0;
      const text =
        tool() === 'tabs' ? t(onTab ? 'canvas.tabs.tipRemove' : 'canvas.tabs.tipAdd') : t('canvas.start.tipSet');
      patchInteraction({ cursor: 'crosshair', markHover: hit.point, hoverId: hit.obj.id, tip: { x: sx + 16, y: sy + 18, text } });
    },

    down(sx: number, sy: number, mods: Mods): void {
      const s = d.st();
      const hit = target(sx, sy);
      if (!hit) return;
      if (tool() === 'tabs') {
        const done = { action: 'none' as 'add' | 'remove' | 'none' };
        s.applyDocChange('canvas.history.tabs', (doc) => {
          const r = toggleTab(doc, hit.obj.id, hit.point, markReach(d, hit.obj));
          done.action = r.action;
          return r.doc;
        });
        if (done.action === 'add') warnIfLayerOff(d, hit.obj);
        return;
      }
      s.select([hit.obj.id]);
      if (mods.ctrl) {
        s.applyDocChange('canvas.history.clearStart', (doc) => clearStart(doc, [hit.obj.id]));
        return;
      }
      if (mods.shift) {
        s.applyDocChange('canvas.history.reverse', (doc) => reverseDirection(doc, [hit.obj.id]));
        return;
      }
      const vertex = mods.alt ? null : nearestVertex(hit.obj, hit.point, hit.tol * 1.5);
      s.applyDocChange('canvas.history.start', (doc) => setStartPoint(doc, hit.obj.id, vertex ?? hit.point));
    },

    leave(): void {
      patchInteraction({ markHover: null, tip: null, hoverId: null });
    },

    cancel(): void {
      patchInteraction({ markHover: null });
      clearGesture();
    },
  };
}

/** Radio (mm) en el que un clic cuenta como "sobre este puente". */
function markReach(d: ToolDeps, obj: DesignObject): number {
  const s = d.st();
  const layer = layerLookup(s.doc.layers)(obj.layer);
  const width = layer?.tabs?.width && layer.tabs.width > 0 ? layer.tabs.width : 2;
  return Math.max(width / 2, (HIT_PX * 1.5) / s.zoom);
}

/** Avisa, una vez por clic, si el puente recién puesto no saldrá porque la capa no lo admite. */
function warnIfLayerOff(d: ToolDeps, obj: DesignObject): void {
  const s = d.st();
  const layer = layerLookup(s.doc.layers)(obj.layer);
  if (!layer) return;
  if (layer.mode !== 'line') s.notify('info', t('canvas.tabs.layerNotLine', { layer: layer.name || layer.color }));
  else if (!layer.tabs?.enabled) s.notify('info', t('canvas.tabs.layerOff', { layer: layer.name || layer.color }));
}
