import type { DesignObject, ImportResult, Point } from '@/api/types';
import { multiplyAffine, objectBounds, translation, unionBounds } from '@/lib/geometry';
import { groupIdsOf } from '@/lib/groups';

/**
 * El dibujo importado, con su centro en `center` (mm). Si trae varios objetos se agrupan, para que
 * se muevan juntos. Devuelve el mismo resultado si no hay nada con área.
 */
export function placeArt(result: ImportResult, center: Point, existing: readonly DesignObject[]): ImportResult {
  const box = unionBounds(result.objects.map(objectBounds));
  if (!box) return result;
  const mid: Point = [(box[0][0] + box[1][0]) / 2, (box[0][1] + box[1][1]) / 2];
  const shift = translation(center[0] - mid[0], center[1] - mid[1]);
  let group: string | null = null;
  if (result.objects.length > 1) {
    const taken = groupIdsOf(existing);
    let i = 1;
    while (taken.has(`grupo-${i}`)) i++;
    group = `grupo-${i}`;
  }
  const objects = result.objects.map((o) => ({
    ...o,
    transform: multiplyAffine(shift, o.transform),
    group: o.group ?? group,
  }));
  return { ...result, objects };
}
