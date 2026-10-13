import type { DockTab } from '@/store/view';

/** Todas las pestañas del panel derecho, en su orden de fábrica. */
export const DOCK_TAB_IDS: readonly DockTab[] = ['layers', 'machine', 'console', 'history', 'camera', 'materials', 'notes'];

/** Orden y visibilidad de las pestañas del panel derecho (se guarda en las preferencias). */
export interface DockLayout {
  /** Todas las pestañas, en el orden en que se muestran. */
  order: DockTab[];
  /** Pestañas ocultas; siempre queda al menos una visible. */
  hidden: DockTab[];
}

export const DEFAULT_DOCK_LAYOUT: DockLayout = { order: [...DOCK_TAB_IDS], hidden: [] };

const isTab = (v: unknown): v is DockTab => typeof v === 'string' && (DOCK_TAB_IDS as readonly string[]).includes(v);

/**
 * Diseño válido a partir de cualquier dato: descarta pestañas desconocidas o repetidas, agrega al
 * final las que falten (una versión nueva puede traer pestañas nuevas) y nunca oculta todas.
 */
export function normalizeDockLayout(raw: unknown): DockLayout {
  const src = raw && typeof raw === 'object' ? (raw as { order?: unknown; hidden?: unknown }) : {};
  const order: DockTab[] = [];
  for (const v of Array.isArray(src.order) ? src.order : []) if (isTab(v) && !order.includes(v)) order.push(v);
  for (const id of DOCK_TAB_IDS) if (!order.includes(id)) order.push(id);
  const hidden: DockTab[] = [];
  for (const v of Array.isArray(src.hidden) ? src.hidden : []) if (isTab(v) && !hidden.includes(v)) hidden.push(v);
  return { order, hidden: hidden.length >= order.length ? [] : hidden };
}

/** Como `normalizeDockLayout`, pero devuelve el mismo objeto si ya era válido (no crea cambios de más en las preferencias). */
export function sanitizeDockLayout(raw: unknown): DockLayout {
  const out = normalizeDockLayout(raw);
  const same = raw !== null && typeof raw === 'object' && JSON.stringify(raw) === JSON.stringify(out);
  return same ? (raw as DockLayout) : out;
}

export const visibleTabs = (layout: DockLayout): DockTab[] => layout.order.filter((id) => !layout.hidden.includes(id));

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
    if (visibleTabs(layout).length <= 1) return layout;
    return { ...layout, hidden: [...layout.hidden, id] };
  }
  return { ...layout, hidden: layout.hidden.filter((x) => x !== id) };
}
