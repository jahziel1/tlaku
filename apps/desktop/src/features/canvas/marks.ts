import type { DesignObject, GuideAxis, LayerSettings, Point } from '@/api/types';
import type { AppState } from '@/store';
import { objectsByIds } from '@/store/objectIndex';
import { nearestOnContour, polyPoints, spanAround, startMark } from './contour';
import { getGeom } from './geom';
import type { InteractionState } from './interaction';

/**
 * Marcas de diseño que se dibujan encima de los objetos: guías del documento, puentes manuales y
 * flechas de inicio y sentido. Se arman con los datos del documento y se cachean por objeto
 * (los objetos del store son inmutables), así que repintar no recalcula contornos.
 */

/** Tope de flechas de inicio a la vez (con "mostrar inicio" encendido en diseños enormes). */
export const MAX_START_MARKS = 300;
/** Ancho (mm) de un puente cuando la capa no lo dice (para dibujar el marcador). */
const DEFAULT_TAB_MM = 2;

export interface GuideMark {
  axis: GuideAxis;
  pos: number;
  /** Bajo el puntero. */
  hover: boolean;
  /** Se soltaría para borrarla. */
  remove: boolean;
}

export interface TabMark {
  /** Tramo del contorno que ocupa el puente (mm). */
  span: Point[];
  /** La capa del objeto tiene los puentes encendidos: si no, no saldrán en el trabajo. */
  active: boolean;
}

export interface StartArrow {
  point: Point;
  /** Sentido de avance, vector unitario (documento, Y hacia arriba). */
  dir: Point;
  /** Fijado por el usuario (si no, es el inicio del trazo tal como se dibujó). */
  fixed: boolean;
}

export interface SceneMarks {
  guides: GuideMark[];
  tabs: TabMark[];
  starts: StartArrow[];
  /** Punto del contorno bajo el puntero con las herramientas de puentes e inicio. */
  hover: Point | null;
}

const EMPTY: SceneMarks = { guides: [], tabs: [], starts: [], hover: null };

const spanCache = new WeakMap<DesignObject, { width: number; spans: Point[][] }>();

/** Tramos de contorno de los puentes manuales de `obj` con el ancho `width` (mm). */
export function tabSpans(obj: DesignObject, width: number): Point[][] {
  const hit = spanCache.get(obj);
  if (hit && hit.width === width) return hit.spans;
  const g = getGeom(obj);
  const spans: Point[][] = [];
  for (const p of obj.manual_tabs ?? []) {
    const near = nearestOnContour(obj, p, true);
    if (!near) continue;
    spans.push(spanAround(polyPoints(g.polys[near.poly]!), true, near.arc, width / 2));
  }
  spanCache.set(obj, { width, spans });
  return spans;
}

const arrowCache = new WeakMap<DesignObject, StartArrow | null>();

export function startArrow(obj: DesignObject): StartArrow | null {
  let a = arrowCache.get(obj);
  if (a === undefined) {
    const m = startMark(obj);
    a = m && { point: m.point, dir: m.dir, fixed: m.fixed };
    arrowCache.set(obj, a);
  }
  return a;
}

/** Puentes manuales y flechas de inicio de los objetos; se recalcula solo si cambia el documento, la selección o lo que decide qué flechas se ven. */
let scanMemo: { objects: DesignObject[]; layers: AppState['doc']['layers']; selected: string[]; start: boolean; tool: string; out: { tabs: TabMark[]; starts: StartArrow[] } } | null = null;

const withTabs = new WeakMap<readonly DesignObject[], boolean>();

function anyTabs(objects: readonly DesignObject[]): boolean {
  let any = withTabs.get(objects);
  if (any === undefined) {
    any = objects.some((o) => (o.manual_tabs?.length ?? 0) > 0);
    withTabs.set(objects, any);
  }
  return any;
}

function scanObjects(s: AppState): { tabs: TabMark[]; starts: StartArrow[] } {
  const m = scanMemo;
  if (m && m.objects === s.doc.objects && m.layers === s.doc.layers && m.selected === s.selectedIds && m.start === s.showStartMarks && m.tool === s.activeTool) return m.out;
  const layers = new Map<string, LayerSettings>(s.doc.layers.map((l) => [l.color.toLowerCase(), l]));
  const tabs: TabMark[] = [];
  const starts: StartArrow[] = [];
  const selected = new Set(s.selectedIds);
  const tool = s.activeTool;
  // Sin puentes manuales en el documento y sin flechas de todos los objetos, solo importan los seleccionados.
  const onlySelected = !s.showStartMarks && tool !== 'start' && !anyTabs(s.doc.objects);
  for (const o of onlySelected ? objectsByIds(s.doc.objects, s.selectedIds) : s.doc.objects) {
    if (o.hidden) continue;
    const layer = layers.get(o.layer.toLowerCase());
    if (layer && !layer.visible) continue;
    if (o.manual_tabs && o.manual_tabs.length > 0) {
      const width = layer?.tabs?.width && layer.tabs.width > 0 ? layer.tabs.width : DEFAULT_TAB_MM;
      const active = layer?.mode === 'line' && layer.tabs?.enabled === true;
      for (const span of tabSpans(o, width)) tabs.push({ span, active });
    }
    // Flecha de inicio: del objeto seleccionado, de los que tienen inicio fijado si "Ver" lo pide y,
    // con la herramienta de inicio, de todos los fijados.
    const wanted = selected.has(o.id) || ((s.showStartMarks || tool === 'start') && o.start != null);
    if (wanted && starts.length < MAX_START_MARKS && o.kind.type !== 'image') {
      const a = startArrow(o);
      if (a) starts.push(a);
    }
  }
  const out = { tabs, starts };
  scanMemo = { objects: s.doc.objects, layers: s.doc.layers, selected: s.selectedIds, start: s.showStartMarks, tool: s.activeTool, out };
  return out;
}

export function buildMarks(s: AppState, ix: Pick<InteractionState, 'guideDrag' | 'guideHover' | 'markHover'>): SceneMarks {
  const guides: GuideMark[] = [];
  if (s.showGuides) {
    const drag = ix.guideDrag;
    (s.doc.guides ?? []).forEach((g, i) => {
      if (drag && drag.index === i) {
        guides.push({ axis: drag.axis, pos: drag.pos, hover: true, remove: drag.remove });
      } else {
        guides.push({ axis: g.axis, pos: g.pos, hover: ix.guideHover === i, remove: false });
      }
    });
    if (drag && drag.index === null) guides.push({ axis: drag.axis, pos: drag.pos, hover: true, remove: drag.remove });
  }

  const { tabs, starts } = scanObjects(s);
  if (guides.length === 0 && tabs.length === 0 && starts.length === 0 && !ix.markHover) return EMPTY;
  return { guides, tabs, starts, hover: ix.markHover };
}
