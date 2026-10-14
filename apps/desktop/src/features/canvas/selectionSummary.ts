import type { DesignObject } from '@/api/types';
import { t } from '@/i18n';
import { unionBounds } from '@/lib/geometry';
import { formatLength, unitSymbol, type LengthUnit } from '@/lib/units';
import { objectBoundsWithCopies } from '@/lib/virtualArray';
import { objectsByIds } from '@/store/objectIndex';
import { boundsOf } from './transform';

export interface SelectionBox {
  count: number;
  /** Esquina inferior izquierda y tamaño, en mm del documento. */
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Caja de la selección (cantidad, esquina inferior izquierda y tamaño), o `null` si no hay nada seleccionado. */
export function selectionBox(objects: readonly DesignObject[], ids: readonly string[]): SelectionBox | null {
  if (ids.length === 0) return null;
  const picked = objectsByIds(objects as DesignObject[], ids as string[]);
  const plain = picked.filter((o) => !o.array);
  const b = unionBounds([boundsOf(plain), ...picked.filter((o) => o.array).map(objectBoundsWithCopies)]);
  if (!b) return null;
  return { count: ids.length, x: b[0][0], y: b[0][1], w: b[1][0] - b[0][0], h: b[1][1] - b[0][1] };
}

/** Frase para lectores de pantalla: cuántos objetos, tamaño y posición. */
export function describeSelection(box: SelectionBox | null, units: LengthUnit): string {
  if (!box) return t('a11y.sel.none');
  const d = units === 'in' ? 3 : 2;
  const n = (mm: number) => formatLength(mm, units, { digits: d, symbol: false });
  return t(box.count === 1 ? 'a11y.sel.one' : 'a11y.sel.many', {
    n: box.count, w: n(box.w), h: n(box.h), x: n(box.x), y: n(box.y), unit: unitSymbol(units),
  });
}
