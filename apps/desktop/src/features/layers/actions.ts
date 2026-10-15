import type { Document, LayerMode, LayerSettings } from '@/api/types';
import { pinTab, type AppState } from '@/store';
import { askChoice } from '@/features/history/prompt';
import { t } from '@/i18n';
import { applyTemplate, assignObjectsToLayer, ensureLayerIn, layerTemplate, MAX_SUB_LAYERS, newSubLayer, normalizeLayer } from '@/lib/layers';
import { colorName, paletteLabel, type PaletteEntry } from '@/lib/palette';
import { useLayerUi } from './layerUi';
import { moveLayerInDoc } from './order';

/**
 * Elegir una muestra de la paleta. Con objetos seleccionados los pasa a esa capa (un solo paso
 * de deshacer, creando la capa si hace falta); sin selección, la vuelve la capa activa.
 */
export function pickPaletteEntry(s: AppState, entry: PaletteEntry): void {
  const ids = s.selectedIds;
  if (ids.length > 0) {
    const locked = new Set(s.doc.objects.filter((o) => o.locked).map((o) => o.id));
    if (ids.every((id) => locked.has(id))) {
      s.notify('warning', t('reason.locked'));
      return;
    }
    s.applyDocChange('history.changeLayer', (doc) => assignObjectsToLayer(doc, ids, entry.color));
  } else if (entry.guide) {
    // Las guías se crean ya (vacías y sin salida): así lo que se dibuje encima no queda con salida activa.
    s.applyDocChange('history.addLayer', (doc) => ensureLayerIn(doc, entry.color));
  }
  s.setActiveLayer(entry.color);
}

/** Texto para anunciar el resultado de elegir una muestra (tooltip). */
export function pickHint(entry: PaletteEntry, selectedCount: number): string {
  const name = paletteLabel(entry);
  if (selectedCount > 0) return t('layers.palette.moveHint', { name, n: selectedCount });
  return t('layers.palette.pickHint', { name });
}

/** Mueve una capa a otra posición de la ejecución, en un solo paso de deshacer. */
export function moveLayer(s: AppState, color: string, toIndex: number): void {
  const mode = s.jobOptions.layer_order;
  s.applyDocChange('history.reorderLayers', (doc) => moveLayerInDoc(doc, mode, color, toIndex));
}

export function selectLayerObjects(s: AppState, color: string): void {
  s.setActiveLayer(color);
  s.selectLayerObjects(color, false);
}

export function openEditorFor(s: AppState, color: string): void {
  s.setDockTab('layers');
  s.setActiveLayer(color);
  useLayerUi.getState().openEditor(color);
}

// ---- copiar y pegar ajustes (C06) ----------------------------------------------------------------

const sameColor = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** Guarda los ajustes de la capa (sin color ni nombre) para pegarlos en otra. */
export function copyLayerSettings(s: AppState, color: string): void {
  const layer = s.doc.layers.find((l) => l.color === color);
  if (!layer) return;
  const name = layer.name || colorName(color);
  useLayerUi.getState().setCopied(layerTemplate(layer), name);
  s.notify('success', t('layers.notice.copied', { name }));
}

/** Pega los ajustes copiados en la capa, en un solo paso de deshacer. Conserva su color, nombre y posición. */
export function pasteLayerSettings(s: AppState, color: string): void {
  const copied = useLayerUi.getState().copied;
  const layer = s.doc.layers.find((l) => l.color === color);
  if (!copied || !layer) return;
  s.applyDocChange('layers.history.paste', (doc) => ({
    ...doc,
    layers: doc.layers.map((l) => (l.color === color ? applyTemplate(normalizeLayer(l), copied.template) : l)),
  }));
  s.notify('success', t('layers.notice.pasted', { name: layer.name || colorName(color) }));
}

// ---- borrar una capa con sus objetos --------------------------------------------------------------

/** El documento sin la capa ni los objetos que están en ella. */
export function removeLayerFrom(doc: Document, color: string): Document {
  return {
    ...doc,
    layers: doc.layers.filter((l) => !sameColor(l.color, color)),
    objects: doc.objects.filter((o) => !sameColor(o.layer, color)),
  };
}

/** Objetos de la capa que están bloqueados (impiden borrarla). */
const lockedMemo = new WeakMap<object, Map<string, number>>();

/** Se guarda por lista de objetos: el panel lo pide en cada actualización del store y no debe recorrer el documento cada vez. */
export function lockedInLayer(doc: Document, color: string): number {
  let byColor = lockedMemo.get(doc.objects);
  if (!byColor) {
    byColor = new Map();
    lockedMemo.set(doc.objects, byColor);
  }
  let n = byColor.get(color);
  if (n === undefined) {
    n = doc.objects.filter((o) => o.locked && sameColor(o.layer, color)).length;
    byColor.set(color, n);
  }
  return n;
}

/**
 * Pide confirmación y borra la capa con sus objetos (un paso de deshacer). Devuelve si se borró.
 * Recibe `get` y no una foto del estado porque el estado puede cambiar mientras espera la respuesta.
 */
export async function deleteLayerWithObjects(get: () => AppState, color: string): Promise<boolean> {
  const layer = get().doc.layers.find((l) => l.color === color);
  if (!layer) return false;
  const name = layer.name || colorName(color);
  if (lockedInLayer(get().doc, color) > 0) {
    get().notify('warning', t('layers.reason.lockedObjects'));
    return false;
  }
  const n = get().doc.objects.filter((o) => sameColor(o.layer, color)).length;
  const inTab = pinTab(get());
  const answer = await askChoice({
    title: t('layers.delete.title'),
    message: n > 0 ? t('layers.delete.ask', { name, n }) : t('layers.delete.askEmpty', { name }),
    choices: [
      { id: 'delete', label: t('layers.delete.confirm'), tone: 'danger' },
      { id: 'cancel', label: t('layers.delete.cancel') },
    ],
    cancelId: 'cancel',
  });
  if (answer !== 'delete') return false;
  // Se borra en la pestaña donde se pidió (la pregunta es modal, pero un atajo podría haber cambiado de pestaña).
  const done = inTab(() => {
    const st = get();
    if (!st.doc.layers.some((l) => l.color === color) || lockedInLayer(st.doc, color) > 0) return false;
    st.applyDocChange('layers.history.delete', (doc) => removeLayerFrom(doc, color));
    const after = get();
    if (after.activeLayer && sameColor(after.activeLayer, color)) after.setActiveLayer(null);
    return true;
  });
  if (!done) return false;
  get().notify('success', t('layers.notice.deleted', { name }));
  return true;
}

// ---- operaciones adicionales (sub-capas) ------------------------------------------------------------

/** Aplica un cambio a la lista de operaciones adicionales de la capa; el mismo documento si no cambia nada. */
function mapSubs(doc: Document, color: string, fn: (subs: LayerSettings[], parent: LayerSettings) => LayerSettings[]): Document {
  let changed = false;
  const layers = doc.layers.map((l) => {
    if (l.color !== color) return l;
    const parent = normalizeLayer(l);
    const subs = fn(parent.sub_layers, parent);
    if (subs === parent.sub_layers) return l;
    changed = true;
    return { ...parent, sub_layers: subs };
  });
  return changed ? { ...doc, layers } : doc;
}

export function addSubLayerTo(doc: Document, color: string, mode: LayerMode): Document {
  return mapSubs(doc, color, (subs, parent) => (subs.length >= MAX_SUB_LAYERS ? subs : [...subs, newSubLayer(parent, mode)]));
}

export function removeSubLayerFrom(doc: Document, color: string, index: number): Document {
  return mapSubs(doc, color, (subs) => (index >= 0 && index < subs.length ? subs.filter((_, i) => i !== index) : subs));
}

export function moveSubLayerIn(doc: Document, color: string, from: number, to: number): Document {
  return mapSubs(doc, color, (subs) => {
    if (from === to || from < 0 || to < 0 || from >= subs.length || to >= subs.length) return subs;
    const next = [...subs];
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item!);
    return next;
  });
}

export function addSubLayer(s: AppState, color: string, mode: LayerMode): void {
  s.applyDocChange('layers.history.subAdd', (doc) => addSubLayerTo(doc, color, mode));
}

export function removeSubLayer(s: AppState, color: string, index: number): void {
  s.applyDocChange('layers.history.subDelete', (doc) => removeSubLayerFrom(doc, color, index));
}

export function moveSubLayer(s: AppState, color: string, from: number, to: number): void {
  s.applyDocChange('layers.history.subMove', (doc) => moveSubLayerIn(doc, color, from, to));
}
