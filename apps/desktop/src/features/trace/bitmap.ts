import type { DesignObject, Document } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { baseName } from '@/features/history/paths';
import { makeLayer } from '@/lib/layers';
import { uniqueId } from '@/store/document';
import type { BitmapOptions, BitmapResult } from './types';

export const DEFAULT_BITMAP_OPTIONS: BitmapOptions = { dpi: 300, filled: true, stroke_mm: 0.3, invert: false };

/** Resoluciones habituales; cualquier valor entre 10 y 2400 se acepta. */
export const BITMAP_DPI_PRESETS = [150, 254, 300, 600, 1000] as const;
export const BITMAP_LIMITS = { dpi: { min: 10, max: 2400 }, stroke_mm: { min: 0.05, max: 5 } } as const;

/** Mismo color que la capa de imágenes del motor (`LayerSettings::IMAGE_COLOR`). */
const IMAGE_COLOR = '#808080';

/** Carpeta del proyecto (donde se guarda el PNG), o `null` si todavía no tiene archivo. */
export function projectDir(filePath: string | null): string | null {
  if (!filePath) return null;
  const cut = Math.max(filePath.lastIndexOf('/'), filePath.lastIndexOf('\\'));
  return cut > 0 ? filePath.slice(0, cut) : null;
}

/** Nombre del proyecto sin extensión, base del nombre del PNG. */
export function projectStem(filePath: string | null, fallback: string): string {
  if (!filePath) return fallback;
  return baseName(filePath).replace(/\.[^.]+$/, '') || fallback;
}

const convertible = (o: DesignObject) => o.kind.type === 'vector' || (o.kind.type === 'text' && o.kind.outline.length > 0);

/** Motivo por el que no se puede convertir la selección a mapa de bits; `true` si se puede. */
export function bitmapEnabled(picked: readonly DesignObject[]): true | I18nKey {
  if (picked.length === 0) return 'trace.bitmap.reason.needShapes';
  return picked.some(convertible) ? true : 'trace.bitmap.reason.needShapes';
}

/** Ids de lo que se convierte: los vectores y textos con contorno de la selección. */
export function bitmapSourceIds(picked: readonly DesignObject[]): string[] {
  return picked.filter(convertible).map((o) => o.id);
}

/** Capa de imagen de destino: la primera capa de imagen del documento o la de fábrica. */
export function imageLayerColor(doc: Document): string {
  return doc.layers.find((l) => l.mode === 'image')?.color ?? IMAGE_COLOR;
}

export interface ApplyBitmapOptions {
  /** Id de los objetos que se convirtieron. */
  sourceIds: readonly string[];
  /** Quita los vectores originales (los bloqueados se quedan). */
  replace: boolean;
}

/**
 * Documento con la imagen nueva en la posición de las formas. Con `replace` los vectores se
 * quitan, así la imagen "reemplaza" a los vectores y se graba como imagen. Es un solo cambio.
 */
export function applyBitmapToDoc(doc: Document, result: BitmapResult, opts: ApplyBitmapOptions): { doc: Document; id: string } {
  const color = imageLayerColor(doc);
  const withLayer = doc.layers.some((l) => l.color.toLowerCase() === color.toLowerCase())
    ? doc
    : { ...doc, layers: [...doc.layers, makeLayer(color, 'Imagen', { mode: 'image', speed: 3000 })] };
  const id = uniqueId('bitmap', new Set(doc.objects.map((o) => o.id)));
  const image: DesignObject = {
    id,
    layer: color,
    transform: [1, 0, 0, 1, result.origin_mm[0], result.origin_mm[1]],
    kind: {
      type: 'image',
      source_path: result.path,
      width_px: result.width_px,
      height_px: result.height_px,
      size_mm: result.size_mm,
    },
    locked: false,
    hidden: false,
    group: null,
  };
  const gone = new Set(opts.replace ? opts.sourceIds : []);
  const kept = withLayer.objects.filter((o) => !gone.has(o.id) || o.locked);
  // La imagen queda justo encima de lo último que se convirtió (o al final si se quitó).
  const last = Math.max(...opts.sourceIds.map((sid) => kept.findIndex((o) => o.id === sid)));
  const at = last >= 0 ? last + 1 : kept.length;
  return { doc: { ...withLayer, objects: [...kept.slice(0, at), image, ...kept.slice(at)] }, id };
}
