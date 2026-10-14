import type { DockTab } from '@/store/view';

/** Todas las pestañas del panel derecho, en su orden de fábrica. */
export const DOCK_TAB_IDS: readonly DockTab[] = ['layers', 'machine', 'console', 'history', 'camera', 'materials', 'notes'];

/** Borde de la ventana donde se acopla el panel de pestañas o la paleta de herramientas. */
export type DockSide = 'left' | 'right';

/** Posición y tamaño, en píxeles de la ventana, de un panel que se sacó a una ventana flotante. */
export interface FloatRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Un panel flotante: la pestaña que muestra y dónde está. */
export interface FloatingPanel extends FloatRect {
  id: DockTab;
}

/** Límites del tamaño de un panel flotante, en píxeles. */
export const FLOAT_MIN_W = 240;
export const FLOAT_MIN_H = 160;
export const FLOAT_MAX_W = 1200;
export const FLOAT_MAX_H = 1200;
/** Tamaño con que aparece un panel recién sacado. */
export const FLOAT_DEFAULT = { w: 340, h: 460 } as const;

/**
 * Distribución de la ventana (se guarda en las preferencias): orden y visibilidad de las pestañas
 * del panel acoplado, el borde donde se acopla, el de la paleta de herramientas y los paneles que
 * están sueltos como ventanas flotantes dentro de la app.
 */
export interface DockLayout {
  /** Todas las pestañas, en el orden en que se muestran. */
  order: DockTab[];
  /** Pestañas ocultas; siempre queda al menos una pestaña visible y acoplada. */
  hidden: DockTab[];
  /** Borde donde se acopla el panel de pestañas. */
  side: DockSide;
  /** Borde donde va la paleta de herramientas. */
  toolsSide: DockSide;
  /** Pestañas sacadas del panel a una ventana flotante (no se repiten ni están en el panel). */
  floating: FloatingPanel[];
}

export const DEFAULT_DOCK_LAYOUT: DockLayout = { order: [...DOCK_TAB_IDS], hidden: [], side: 'right', toolsSide: 'left', floating: [] };

const isTab = (v: unknown): v is DockTab => typeof v === 'string' && (DOCK_TAB_IDS as readonly string[]).includes(v);

const finite = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
const sideOf = (v: unknown, fallback: DockSide): DockSide => (v === 'left' || v === 'right' ? v : fallback);

/** Rectángulo flotante con medidas válidas (tamaño dentro de los límites, posición entera). */
export function sanitizeFloatRect(raw: unknown): FloatRect | null {
  const r = raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : null;
  if (!r || !finite(r.x) || !finite(r.y) || !finite(r.w) || !finite(r.h)) return null;
  return {
    x: Math.round(clamp(r.x, 0, 20000)),
    y: Math.round(clamp(r.y, 0, 20000)),
    w: Math.round(clamp(r.w, FLOAT_MIN_W, FLOAT_MAX_W)),
    h: Math.round(clamp(r.h, FLOAT_MIN_H, FLOAT_MAX_H)),
  };
}

/**
 * Diseño válido a partir de cualquier dato: descarta pestañas desconocidas o repetidas, agrega al
 * final las que falten (una versión nueva puede traer pestañas nuevas) y nunca deja el panel sin
 * una pestaña visible y acoplada (si todas estarían ocultas o flotando, vuelven al panel).
 */
export function normalizeDockLayout(raw: unknown): DockLayout {
  const src = raw && typeof raw === 'object' ? (raw as { order?: unknown; hidden?: unknown; side?: unknown; toolsSide?: unknown; floating?: unknown }) : {};
  const order: DockTab[] = [];
  for (const v of Array.isArray(src.order) ? src.order : []) if (isTab(v) && !order.includes(v)) order.push(v);
  for (const id of DOCK_TAB_IDS) if (!order.includes(id)) order.push(id);
  const hidden: DockTab[] = [];
  for (const v of Array.isArray(src.hidden) ? src.hidden : []) if (isTab(v) && !hidden.includes(v)) hidden.push(v);
  let floating: FloatingPanel[] = [];
  for (const v of Array.isArray(src.floating) ? src.floating : []) {
    const id = (v as { id?: unknown } | null)?.id;
    const rect = sanitizeFloatRect(v);
    if (isTab(id) && rect && !floating.some((f) => f.id === id)) floating.push({ id, ...rect });
  }
  const docked = (h: DockTab[], f: FloatingPanel[]) => order.filter((id) => !h.includes(id) && !f.some((p) => p.id === id));
  // Sin ninguna pestaña acoplada y visible: primero se devuelven los paneles flotantes, luego lo oculto.
  if (docked(hidden, floating).length === 0) floating = [];
  const keepHidden = docked(hidden, floating).length === 0 ? [] : hidden;
  return { order, hidden: keepHidden, side: sideOf(src.side, 'right'), toolsSide: sideOf(src.toolsSide, 'left'), floating };
}

/** Como `normalizeDockLayout`, pero devuelve el mismo objeto si ya era válido (no crea cambios de más en las preferencias). */
export function sanitizeDockLayout(raw: unknown): DockLayout {
  const out = normalizeDockLayout(raw);
  const same = raw !== null && typeof raw === 'object' && JSON.stringify(raw) === JSON.stringify(out);
  return same ? (raw as DockLayout) : out;
}

export const isFloating = (layout: DockLayout, id: DockTab): boolean => layout.floating.some((f) => f.id === id);

/** Pestañas que se ven en el panel acoplado: ni ocultas ni flotando. */
export const visibleTabs = (layout: DockLayout): DockTab[] => layout.order.filter((id) => !layout.hidden.includes(id) && !isFloating(layout, id));

/** Paneles flotantes que se ven (los ocultos desde el menú no se pintan). */
export const shownFloating = (layout: DockLayout): FloatingPanel[] => layout.floating.filter((f) => !layout.hidden.includes(f.id));

/** Mueve `id` justo antes de `before` (o al final si `before` es `null`). */
export function moveDockTab(layout: DockLayout, id: DockTab, before: DockTab | null): DockLayout {
  if (id === before) return layout;
  const rest = layout.order.filter((x) => x !== id);
  const at = before === null ? rest.length : rest.indexOf(before);
  if (at < 0) return layout;
  rest.splice(at, 0, id);
  return rest.every((x, i) => x === layout.order[i]) ? layout : { ...layout, order: rest };
}

/** Mueve `id` una posición hacia el inicio (`-1`) o el final (`1`) entre las pestañas visibles. */
export function nudgeDockTab(layout: DockLayout, id: DockTab, dir: -1 | 1): DockLayout {
  const shown = visibleTabs(layout);
  const i = shown.indexOf(id);
  const next = shown[i + dir];
  if (i < 0 || next === undefined) return layout;
  // Hacia el inicio: justo antes de la vecina. Hacia el final: justo antes de la que sigue a la vecina.
  return moveDockTab(layout, id, dir < 0 ? next : (layout.order[layout.order.indexOf(next) + 1] ?? null));
}

/** Muestra u oculta `id`. La última pestaña visible no se puede ocultar. */
export function setDockTabHidden(layout: DockLayout, id: DockTab, hide: boolean): DockLayout {
  const isHidden = layout.hidden.includes(id);
  if (hide === isHidden) return layout;
  if (hide) {
    // Una pestaña flotante se puede ocultar sin más; la última acoplada y visible, no.
    if (!isFloating(layout, id) && visibleTabs(layout).length <= 1) return layout;
    return { ...layout, hidden: [...layout.hidden, id] };
  }
  return { ...layout, hidden: layout.hidden.filter((x) => x !== id) };
}

/** Posición de un panel recién sacado: en cascada, para que no tape al anterior. */
function cascadeRect(layout: DockLayout): FloatRect {
  const n = layout.floating.length % 8;
  return { x: 160 + n * 28, y: 96 + n * 28, ...FLOAT_DEFAULT };
}

/** Saca `id` del panel a una ventana flotante. La última pestaña acoplada no se puede sacar. */
export function floatDockTab(layout: DockLayout, id: DockTab, rect?: Partial<FloatRect>): DockLayout {
  if (isFloating(layout, id)) return layout;
  const shown = visibleTabs(layout);
  if (shown.includes(id) && shown.length <= 1) return layout;
  const next = sanitizeFloatRect({ ...cascadeRect(layout), ...rect });
  if (!next) return layout;
  return { ...layout, floating: [...layout.floating, { id, ...next }] };
}

/** Devuelve `id` al panel acoplado (vuelve a su sitio en el orden). */
export function dockFloatingTab(layout: DockLayout, id: DockTab): DockLayout {
  if (!isFloating(layout, id)) return layout;
  return { ...layout, floating: layout.floating.filter((f) => f.id !== id) };
}

/** Cambia la posición o el tamaño de un panel flotante. */
export function setFloatRect(layout: DockLayout, id: DockTab, rect: FloatRect): DockLayout {
  const next = sanitizeFloatRect(rect);
  const cur = layout.floating.find((f) => f.id === id);
  if (!next || !cur || (cur.x === next.x && cur.y === next.y && cur.w === next.w && cur.h === next.h)) return layout;
  return { ...layout, floating: layout.floating.map((f) => (f.id === id ? { id, ...next } : f)) };
}

/** Acopla el panel de pestañas al borde `side`. */
export function setDockSide(layout: DockLayout, side: DockSide): DockLayout {
  return layout.side === side ? layout : { ...layout, side };
}

/** Pone la paleta de herramientas en el borde `side`. */
export function setToolsSide(layout: DockLayout, toolsSide: DockSide): DockLayout {
  return layout.toolsSide === toolsSide ? layout : { ...layout, toolsSide };
}

/**
 * Rectángulo que cabe en una ventana de `bounds` píxeles: nunca más grande que ella y con la barra
 * de título siempre al alcance (si el usuario achica la ventana de la app, el panel no se pierde).
 */
export function fitFloatRect(rect: FloatRect, bounds: { w: number; h: number }): FloatRect {
  const w = Math.min(rect.w, Math.max(FLOAT_MIN_W, bounds.w));
  const h = Math.min(rect.h, Math.max(FLOAT_MIN_H, bounds.h));
  return { w, h, x: Math.round(clamp(rect.x, 0, Math.max(0, bounds.w - w))), y: Math.round(clamp(rect.y, 0, Math.max(0, bounds.h - Math.min(h, 40)))) };
}

/** Columnas del cuerpo de la ventana, de izquierda a derecha, según los bordes elegidos. */
export function shellColumns(layout: Pick<DockLayout, 'side' | 'toolsSide'>): ('tools' | 'main' | 'dock')[] {
  const left: ('tools' | 'dock')[] = [];
  const right: ('tools' | 'dock')[] = [];
  (layout.toolsSide === 'left' ? left : right).push('tools');
  // El panel queda del lado del lienzo cuando comparte borde con las herramientas.
  if (layout.side === 'left') left.push('dock');
  else right.unshift('dock');
  return [...left, 'main', ...right];
}
