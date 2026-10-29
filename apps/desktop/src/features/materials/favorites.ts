import type { DesignObject, Document, MaterialPreset } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { assignObjectsToLayer, countByLayer, ensureLayerIn } from '@/lib/layers';
import { colorName, isGuideColor, LAYER_PALETTE } from '@/lib/palette';
import { getPref, useStore, type AppState } from '@/store';
import { MAX_FAVORITES } from '@/store/prefs';
import { materialName, titleName } from './labels';
import { applyPresetChange, layerMatchesPreset } from './library';
import { useMaterials } from './state';

/**
 * Materiales favoritos (estrella). Solo se guardan los ids en la preferencia `favoriteMaterials`
 * (`preferences.json`): así viajan con el paquete `.tlakuprefs` y con el respaldo a carpeta, y no
 * tocan el formato de la biblioteca. Un id que ya no existe (se borró el material) simplemente no se muestra.
 */

const app = () => useStore.getState();

export const favoriteIds = (): readonly string[] => getPref('favoriteMaterials');

/** ¿El material es favorito? (con `ids` explícito sirve en componentes reactivos). */
export const isFavorite = (id: string, ids: readonly string[] = favoriteIds()): boolean => ids.includes(id);

/** Marca o desmarca. Devuelve `false` si no se pudo marcar por llegar al tope. */
export function setFavorite(id: string, on: boolean): boolean {
  const ids = favoriteIds();
  if (on === ids.includes(id)) return true;
  if (on && ids.length >= MAX_FAVORITES) return false;
  app().setPref('favoriteMaterials', on ? [...ids, id] : ids.filter((x) => x !== id));
  return true;
}

/** Quita un id de los favoritos (al borrar el material). */
export function dropFavorite(id: string): void {
  setFavorite(id, false);
}

/** Los materiales favoritos que existen, en el orden en que se marcaron. */
export function favoritePresets(user: readonly MaterialPreset[], factory: readonly MaterialPreset[], ids: readonly string[]): MaterialPreset[] {
  const all = new Map<string, MaterialPreset>();
  for (const p of [...factory, ...user]) all.set(p.id, p);
  return ids.map((id) => all.get(id)).filter((p): p is MaterialPreset => p !== undefined);
}

/** Favoritos de ahora, leyendo el estado de la biblioteca. */
export function currentFavorites(): MaterialPreset[] {
  const ui = useMaterials.getState();
  return favoritePresets(ui.library.presets, ui.factory.presets, favoriteIds());
}

export const presetLabel = (p: Pick<MaterialPreset, 'material' | 'title' | 'material_key' | 'title_key'>): string => `${materialName(p)} · ${titleName(p)}`;

// ---------------------------------------------------------------------------
// Aplicar a la selección
// ---------------------------------------------------------------------------

/** Capa donde irá la selección: una que ya tiene esos ajustes, o un color libre al que se le aplican. */
export interface FavoriteTarget {
  color: string;
  /** `true` = la capa ya existía con esos ajustes (no hay que cambiar nada en ella). */
  existing: boolean;
}

/** Objetos seleccionados que se pueden mover (no bloqueados). */
function movable(s: Pick<AppState, 'doc' | 'selectedIds'>): DesignObject[] {
  const ids = new Set(s.selectedIds);
  return s.doc.objects.filter((o) => ids.has(o.id) && !o.locked);
}

/** Una capa de imagen solo admite imágenes y las demás, vectores (lo mismo que decide qué sale a la máquina). */
const accepts = (mode: string, o: DesignObject): boolean => (mode === 'image') === (o.kind.type === 'image');

/**
 * Capa de destino de `preset`: primero una con esos ajustes y con salida; si no hay, el primer color de
 * la paleta sin capa; y si todos están en uso, uno cuya capa esté vacía (se le aplica el material).
 * `null` si no queda ninguno.
 */
export function pickFavoriteTarget(doc: Document, preset: MaterialPreset): FavoriteTarget | null {
  const hit = doc.layers.find((l) => !isGuideColor(l.color) && l.output && layerMatchesPreset(l, preset));
  if (hit) return { color: hit.color, existing: true };
  const has = new Set(doc.layers.map((l) => l.color.toLowerCase()));
  const free = LAYER_PALETTE.find((p) => !p.guide && !has.has(p.color));
  if (free) return { color: free.color, existing: false };
  const counts = countByLayer(doc);
  const empty = LAYER_PALETTE.find((p) => !p.guide && (counts.get(p.color)?.total ?? 0) === 0);
  return empty ? { color: empty.color, existing: false } : null;
}

/** Por qué no se puede mover la selección a `preset`, o `null` si se puede. */
export function favoriteApplyReason(s: Pick<AppState, 'doc' | 'selectedIds'>, preset: MaterialPreset): { key: I18nKey; params?: Record<string, string> } | null {
  if (s.selectedIds.length === 0) return { key: 'materials.fav.applyReason.noSelection' };
  const objs = movable(s);
  if (objs.length === 0) return { key: 'materials.fav.applyReason.locked' };
  if (!objs.every((o) => accepts(preset.settings.mode, o))) return { key: 'materials.fav.applyReason.mode', params: { mode: t(`materials.mode.${preset.settings.mode}`) } };
  if (!pickFavoriteTarget(s.doc, preset)) return { key: 'materials.fav.applyReason.noFreeLayer' };
  return null;
}

/**
 * Mueve la selección a una capa con los ajustes del material: la misma operación de «aplicar un material a
 * una capa» (`applyPresetChange`) más la de la paleta de colores (`assignObjectsToLayer`), en un solo paso de deshacer.
 */
export function applyFavoriteToSelection(preset: MaterialPreset): boolean {
  const s = app();
  const why = favoriteApplyReason(s, preset);
  if (why) {
    s.notify('warning', t(why.key, why.params));
    return false;
  }
  const target = pickFavoriteTarget(s.doc, preset)!;
  const ids = movable(s).map((o) => o.id);
  const { color } = target;
  s.applyDocChange('materials.fav.history', (doc) => {
    let d = doc;
    if (!target.existing) {
      d = applyPresetChange(ensureLayerIn(d, color), color, preset);
      // Una capa vacía reutilizada pudo quedar sin salida: aquí hace falta que salga.
      d = { ...d, layers: d.layers.map((l) => (l.color.toLowerCase() === color.toLowerCase() ? { ...l, output: true, visible: true } : l)) };
    }
    return assignObjectsToLayer(d, ids, color);
  });
  s.setActiveLayer(color);
  const layer = app().doc.layers.find((l) => l.color.toLowerCase() === color.toLowerCase());
  s.notify('success', t(target.existing ? 'materials.fav.moved' : 'materials.fav.movedNew', { n: String(ids.length), layer: layer?.name || colorName(color), name: presetLabel(preset) }));
  return true;
}
