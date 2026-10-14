import type { Document } from '@/api/types';
import { t } from '@/i18n';
import { factoryLayer } from '@/lib/layers';
import { MARK_ID_PREFIX, MARK_LAYER_COLOR, markObjects, type MarkSegment } from './plan';

/**
 * Copia del documento con las cruces de alineación en su propia capa (línea suave, para que se
 * vean sin atravesar el material). No toca el documento del usuario: solo viaja al motor con el
 * tramo. Si la capa ya existe se respeta tal como esté; se quitan marcas anteriores.
 */
export function withAlignmentMarks(doc: Document, segments: MarkSegment[]): Document {
  if (segments.length === 0) return doc;
  const layers = doc.layers.some((l) => l.color === MARK_LAYER_COLOR)
    ? doc.layers
    : [...doc.layers, factoryLayer(MARK_LAYER_COLOR, t('tiling.markLayer'), { speed: 3000, power: 12 })];
  const objects = doc.objects.filter((o) => !o.id.startsWith(MARK_ID_PREFIX));
  return { ...doc, layers, objects: [...objects, ...markObjects(segments)] };
}

/** Ids de las marcas (para añadirlos a «Solo la selección»). */
export function markIds(segments: MarkSegment[]): string[] {
  return markObjects(segments).map((o) => o.id);
}
