import { getApi, getApiKind } from '@/api';
import type { Point } from '@/api/types';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { screenToWorld } from '@/lib/viewport';
import { useStore } from '@/store';
import { useArtPanel } from './artStore';
import { artName, type ArtItem } from './catalog';
import { placeArt } from './place';

const state = () => useStore.getState();

/** Capa de destino: la activa si es de líneas o relleno; si no, la que elija el importador. */
function targetLayer(): string | null {
  const s = state();
  const layer = s.activeLayer ? s.doc.layers.find((l) => l.color.toLowerCase() === s.activeLayer!.toLowerCase()) : undefined;
  return layer && layer.mode !== 'image' ? layer.color : null;
}

/** Centro de lo que se ve del lienzo, en mm. */
export function viewCenter(): Point {
  const { zoom, panX, panY, viewport } = state();
  return screenToWorld({ zoom, panX, panY }, [viewport.width / 2, viewport.height / 2]);
}

/** Inserta el dibujo con su centro en `at` (o en el centro de la vista) y lo deja seleccionado. */
export async function insertArt(item: ArtItem, at?: Point): Promise<boolean> {
  try {
    const result = await getApi().artImportSvg({ svg: item.svg, target_layer: targetLayer() });
    const placed = placeArt(result, at ?? viewCenter(), state().doc.objects);
    const ids = state().applyImport(placed);
    if (ids.length > 0) {
      state().select(ids);
      state().notify('success', t('art.inserted', { name: artName(item) }));
    }
    if (result.warnings.length > 0) state().notify('warning', t('art.insertWarnings', { n: result.warnings.length }), result.warnings);
    void state().computePreview();
    return ids.length > 0;
  } catch (e) {
    state().notify('error', t('art.insertFailed', { error: errorMessage(e) }));
    return false;
  }
}

/** Carga la biblioteca del usuario desde la carpeta de configuración. */
export async function loadUserArt(): Promise<void> {
  try {
    useArtPanel.getState().setUser(await getApi().artListUser());
    useArtPanel.getState().setError(null);
  } catch (e) {
    useArtPanel.getState().setError(t('art.user.loadFailed', { error: errorMessage(e) }));
  }
}

/** Elige SVG con el diálogo del sistema y los copia a la biblioteca del usuario. */
export async function addUserArt(): Promise<void> {
  let paths: string[];
  if (getApiKind() !== 'tauri') {
    paths = ['ejemplos/mi-dibujo.svg'];
  } else {
    const dialog = await import('@tauri-apps/plugin-dialog');
    const picked = await dialog.open({ multiple: true, directory: false, filters: [{ name: 'SVG', extensions: ['svg'] }] });
    paths = Array.isArray(picked) ? picked : typeof picked === 'string' ? [picked] : [];
  }
  if (paths.length === 0) return;
  try {
    const r = await getApi().artAddUser({ paths });
    if (r.added.length > 0) state().notify('success', t('art.user.added', { n: r.added.length }));
    if (r.errors.length > 0) state().notify('warning', t('art.user.addErrors', { n: r.errors.length }), r.errors);
    await loadUserArt();
  } catch (e) {
    useArtPanel.getState().setError(t('art.user.addFailed', { error: errorMessage(e) }));
  }
}

export async function removeUserArt(name: string): Promise<void> {
  try {
    await getApi().artRemoveUser({ name });
    await loadUserArt();
  } catch (e) {
    useArtPanel.getState().setError(t('art.user.removeFailed', { error: errorMessage(e) }));
  }
}
