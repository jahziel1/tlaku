import { getApi, getApiKind } from '@/api';
import type { DesignObject, ImageObject } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { baseName } from '@/features/history/paths';
import { selectSelectedObjects, useStore, type AppState } from '@/store';

const state = () => useStore.getState();

type ImageObj = DesignObject & { kind: ImageObject & { type: 'image' } };

function singleImage(s: AppState): ImageObj | null {
  const picked = selectSelectedObjects(s);
  const o = picked.length === 1 ? picked[0]! : null;
  return o && o.kind.type === 'image' ? (o as ImageObj) : null;
}

/** Solo con exactamente una imagen seleccionada. */
export function imageOpEnabled(s: AppState): true | I18nKey {
  return singleImage(s) ? true : 'trace.reason.oneImage';
}

/** Nombre sugerido para la imagen procesada: el de la imagen con `-procesada.png`. */
export function processedName(sourcePath: string): string {
  return `${baseName(sourcePath).replace(/\.[^.]+$/, '')}-procesada.png`;
}

/**
 * "Guardar imagen procesada": escribe un PNG con lo que se grabaría de la imagen seleccionada
 * (ajustes de brillo, contraste y tramado de su capa, y la máscara). `true` si se guardó.
 */
export async function saveProcessedImage(): Promise<boolean> {
  const obj = singleImage(state());
  if (!obj) return false;
  const layer = state().doc.layers.find((l) => l.color.toLowerCase() === obj.layer.toLowerCase());
  if (!layer || layer.mode !== 'image') {
    state().notify('warning', t('trace.processed.notImageLayer'));
    return false;
  }
  let path: string | null;
  if (getApiKind() !== 'tauri') {
    path = `exportes/${processedName(obj.kind.source_path)}`;
  } else {
    const dialog = await import('@tauri-apps/plugin-dialog');
    path = await dialog.save({ filters: [{ name: 'PNG', extensions: ['png'] }], defaultPath: processedName(obj.kind.source_path) });
  }
  if (!path) return false;
  if (!/\.png$/i.test(path)) path += '.png';
  try {
    const r = await getApi().saveProcessedImage({ image: obj.kind, layer, path });
    state().notify('success', t('trace.processed.done', { path: r.path, w: r.width_px, h: r.height_px }));
    return true;
  } catch (e) {
    state().notify('error', t('trace.processed.failed', { error: errorMessage(e) }));
    return false;
  }
}

/** Nuevo `ImageObject` al cambiar el archivo: conserva el ancho y la posición, y sigue la proporción del nuevo. */
export function replacedImage(old: ImageObject, next: Pick<ImageObject, 'source_path' | 'width_px' | 'height_px'>): ImageObject {
  const w = old.size_mm[0];
  const h = next.width_px > 0 ? (w * next.height_px) / next.width_px : old.size_mm[1];
  // La máscara era de la forma de la imagen anterior: se descarta.
  const { mask: _mask, ...rest } = old;
  return { ...rest, source_path: next.source_path, width_px: next.width_px, height_px: next.height_px, size_mm: [w, h] };
}

/** "Reemplazar imagen": cambia el archivo de la imagen seleccionada sin mover ni cambiar de capa. */
export async function replaceImage(): Promise<boolean> {
  const obj = singleImage(state());
  if (!obj) return false;
  let path: string | null;
  if (getApiKind() !== 'tauri') {
    path = 'ejemplos/nueva.png';
  } else {
    const dialog = await import('@tauri-apps/plugin-dialog');
    const picked = await dialog.open({
      multiple: false,
      directory: false,
      filters: [{ name: t('trace.batch.filter'), extensions: ['png', 'jpg', 'jpeg', 'bmp', 'webp', 'tif', 'tiff', 'gif', 'tga'] }],
    });
    path = typeof picked === 'string' ? picked : null;
  }
  if (!path) return false;
  try {
    const r = await getApi().importFile({ path });
    const next = r.objects.map((o) => o.kind).find((k) => k.type === 'image');
    if (next?.type !== 'image') throw new Error(t('trace.batch.notImage', { file: baseName(path) }));
    const id = obj.id;
    state().applyDocChange('trace.replace.history', (doc) => ({
      ...doc,
      objects: doc.objects.map((o) => (o.id === id && o.kind.type === 'image' ? { ...o, kind: { type: 'image' as const, ...replacedImage(o.kind, next) } } : o)),
    }));
    state().notify('success', t('trace.replace.done', { name: baseName(path) }));
    void state().computePreview();
    return true;
  } catch (e) {
    state().notify('error', t('trace.replace.failed', { error: errorMessage(e) }));
    return false;
  }
}
