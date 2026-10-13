import type { AppState } from '@/store';
import { t } from '@/i18n';
import { assignObjectsToLayer, ensureLayerIn } from '@/lib/layers';
import { paletteLabel, type PaletteEntry } from '@/lib/palette';
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
