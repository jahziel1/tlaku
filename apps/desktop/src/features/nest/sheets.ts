import type { Affine, Point } from '@/api/types';
import type { Remnant } from './remnants';
import type { FlatPoly } from './preview';
import type { NestContainer, NestPlacement, NestSheet } from './types';

/** Separación (mm) entre hojas contiguas en el lienzo. */
export const SHEET_GAP_MM = 20;

type Box = readonly [number, number, number, number];

/**
 * Cuánto se traslada cada hoja para dejarlas en fila hacia la derecha, alineadas por abajo con la
 * primera: la primera se queda donde está (la cama o el material del documento) y cada hoja
 * siguiente va a `gap` mm de la anterior. Así cada hoja es una zona aparte del lienzo que se puede
 * seleccionar con un recuadro y mandar por separado.
 */
export function sheetOffsets(sheets: readonly Pick<NestSheet, 'bounds'>[], gap = SHEET_GAP_MM): [number, number][] {
  const out: [number, number][] = [];
  let right = 0;
  let bottom = 0;
  sheets.forEach((s, k) => {
    const [x0, y0, x1] = s.bounds;
    if (k === 0) {
      out.push([0, 0]);
      right = x1;
      bottom = y0;
      return;
    }
    const dx = right + gap - x0;
    out.push([dx, bottom - y0]);
    right = x1 + dx;
  });
  return out;
}

/** El acomodo de cada hoja, ya trasladado a su lugar en el lienzo (el `transform` de cada colocación incluye el traslado). */
export function offsetPlacements(placements: readonly NestPlacement[], offsets: readonly (readonly [number, number])[]): NestPlacement[] {
  return placements.map((p) => {
    const [dx, dy] = offsets[p.sheet] ?? [0, 0];
    if (dx === 0 && dy === 0) return p;
    const t: Affine = [p.transform[0], p.transform[1], p.transform[2], p.transform[3], p.transform[4] + dx, p.transform[5] + dy];
    return { ...p, transform: t };
  });
}

const rectRing = ([x0, y0, x1, y1]: Box): Point[] => [
  [x0, y0],
  [x1, y0],
  [x1, y1],
  [x0, y1],
];

/**
 * Contorno de una hoja en las coordenadas de sus colocaciones (sin trasladar): un rectángulo para
 * la cama, el contorno del retazo o el del objeto material.
 */
export function sheetOutline(
  sheet: NestSheet,
  container: NestContainer | null,
  remnants: ReadonlyMap<string, Pick<Remnant, 'paths'>>,
  materialPolys: readonly FlatPoly[] | null,
): FlatPoly[] {
  if (sheet.source === 'remnant') {
    const r = sheet.remnant_id ? remnants.get(sheet.remnant_id) : undefined;
    if (r) return r.paths.map((ring) => ({ points: ring.map(([x, y]): Point => [x, y]), closed: true }));
  } else if (container?.type === 'object' && materialPolys) {
    return materialPolys.filter((p) => p.closed).map((p) => ({ points: [...p.points], closed: true }));
  }
  return [{ points: rectRing(sheet.bounds), closed: true }];
}

/** Caja total de todas las hojas ya colocadas en fila (para encuadrar la vista previa). */
export function sheetsBounds(sheets: readonly Pick<NestSheet, 'bounds'>[], offsets: readonly (readonly [number, number])[]): [Point, Point] | null {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  sheets.forEach((s, k) => {
    const [dx, dy] = offsets[k] ?? [0, 0];
    minX = Math.min(minX, s.bounds[0] + dx);
    minY = Math.min(minY, s.bounds[1] + dy);
    maxX = Math.max(maxX, s.bounds[2] + dx);
    maxY = Math.max(maxY, s.bounds[3] + dy);
  });
  return minX <= maxX ? [[minX, minY], [maxX, maxY]] : null;
}

/**
 * Qué hojas llevan un contorno de referencia nuevo en el lienzo: todas cuando hay varias, salvo la
 * primera si ya es un objeto del documento (el material hace de referencia); con una sola hoja,
 * solo si es un retazo (la cama no necesita dibujarse).
 */
export function referenceSheets(sheets: readonly NestSheet[], container: NestContainer | null): number[] {
  const out: number[] = [];
  sheets.forEach((s, k) => {
    const isMaterialObject = s.source === 'container' && container?.type === 'object';
    if (k === 0 && isMaterialObject) return;
    if (sheets.length === 1 && s.source === 'container') return;
    out.push(k);
  });
  return out;
}

/** Contornos de referencia ya trasladados a su lugar en el lienzo, en el orden de `referenceSheets`. */
export function sheetReferences(
  sheets: readonly NestSheet[],
  offsets: readonly (readonly [number, number])[],
  container: NestContainer | null,
  remnants: ReadonlyMap<string, Pick<Remnant, 'paths'>>,
  materialPolys: readonly FlatPoly[] | null,
): { sheet: number; polys: FlatPoly[] }[] {
  return referenceSheets(sheets, container).map((k) => {
    const [dx, dy] = offsets[k] ?? [0, 0];
    const polys = sheetOutline(sheets[k]!, container, remnants, materialPolys).map(
      (p): FlatPoly => ({ closed: p.closed, points: p.points.map(([x, y]): Point => [x + dx, y + dy]) }),
    );
    return { sheet: k, polys };
  });
}
