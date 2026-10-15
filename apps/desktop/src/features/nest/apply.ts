import type { Affine, DesignObject, Document } from '@/api/types';
import type { Unit } from '@/features/arrange/units';
import { polylineBez } from '@/lib/bezpath';
import { multiplyAffine } from '@/lib/geometry';
import { groupIdsOf, regroupCopies } from '@/lib/groups';
import { ensureLayerIn } from '@/lib/layers';
import { GUIDE_COLOR } from '@/lib/palette';
import { uniqueId } from '@/store/document';
import type { FlatPoly } from './preview';
import { offsetPlacements } from './sheets';
import type { NestPlacement } from './types';

/** Varias hojas: cuánto se traslada cada una y los contornos de referencia que se agregan. */
export interface SheetsLayout {
  /** Traslado de cada hoja (el índice es `NestPlacement.sheet`). */
  offsets: readonly (readonly [number, number])[];
  /** Contornos (ya trasladados) de las hojas que llevan una referencia nueva; `sheet` es su posición. */
  references: readonly { sheet: number; polys: readonly FlatPoly[] }[];
}

/** Redondea a 1 nm (y evita el -0) para que las posiciones no arrastren ruido de coma flotante. */
const round6 = (n: number) => Math.round(n * 1e6) / 1e6 + 0;
const tidy = (m: Affine): Affine => [m[0], m[1], m[2], m[3], round6(m[4]), round6(m[5])];

/** Transform nuevo de un objeto: el del acomodo POR ENCIMA del que ya tenía. */
const placed = (placement: Affine, o: DesignObject): Affine => tidy(multiplyAffine(placement, o.transform));

/**
 * Aplica el acomodo al documento. `copy` 0 mueve el objeto original; `copy` >= 1 agrega clones de
 * TODOS los objetos de la unidad (ids nuevos, grupos propios, sin bloqueo) encima de todo. Las
 * piezas que no están en `placements` quedan intactas; los lugares de piezas desconocidas o
 * repetidas se ignoran. Devuelve el documento nuevo y los ids de todo lo acomodado (originales
 * movidos y copias creadas), o el mismo documento si no había nada que hacer.
 *
 * Con `layout` (varias hojas o retazos) cada hoja se traslada a su zona del lienzo y se agrega, en
 * la capa de guías (que no se corta), un objeto `hoja-N` con su contorno: sirve de referencia y
 * para seleccionar la hoja con un recuadro y enviarla por separado. Esos objetos no entran en
 * `placed`.
 */
export function applyNest(
  doc: Document,
  units: readonly Unit[],
  placements: readonly NestPlacement[],
  layout?: SheetsLayout,
): { doc: Document; placed: string[] } {
  if (layout) placements = offsetPlacements(placements, layout.offsets);
  const byPiece = new Map(units.map((u) => [u.id, u]));
  const moved = new Map<string, Affine>();
  const taken = new Set(doc.objects.map((o) => o.id));
  const groups = groupIdsOf(doc.objects);
  const seen = new Set<string>();
  const created: DesignObject[] = [];

  for (const p of placements) {
    const unit = byPiece.get(p.piece);
    const key = `${p.piece}\u0000${p.copy}`;
    if (!unit || seen.has(key) || !Number.isInteger(p.copy) || p.copy < 0) continue;
    seen.add(key);
    if (p.copy === 0) {
      for (const o of unit.objects) moved.set(o.id, placed(p.transform, o));
    } else {
      const clones = unit.objects.map((o): DesignObject => {
        const id = uniqueId(o.id.replace(/-\d+$/, ''), taken);
        taken.add(id);
        return { ...o, id, locked: false, transform: placed(p.transform, o) };
      });
      created.push(...regroupCopies(clones, groups));
    }
  }
  if (moved.size === 0 && created.length === 0) return { doc, placed: [] };

  const objects = doc.objects.map((o) => {
    const transform = moved.get(o.id);
    return transform ? { ...o, transform } : o;
  });
  const refs: DesignObject[] = [];
  for (const r of layout?.references ?? []) {
    const closed = r.polys.filter((p) => p.points.length >= 3);
    if (closed.length === 0) continue;
    let id = `hoja-${r.sheet + 1}`;
    for (let i = 2; taken.has(id); i++) id = `hoja-${r.sheet + 1}-${i}`;
    taken.add(id);
    refs.push({
      id,
      layer: GUIDE_COLOR,
      transform: [1, 0, 0, 1, 0, 0],
      kind: { type: 'vector', paths: closed.map((p) => polylineBez(p.points, true)) },
      locked: false,
      hidden: false,
      group: null,
    });
  }
  const base = refs.length > 0 ? ensureLayerIn(doc, GUIDE_COLOR) : doc;
  return {
    doc: { ...base, objects: [...objects, ...created, ...refs] },
    placed: [...moved.keys(), ...created.map((o) => o.id)],
  };
}
