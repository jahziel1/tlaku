import type { DesignObject, Document, LayerSettings, Point } from '@/api/types';
import type { GenerateResult } from '@/bindings/GenerateResult';
import type { GeneratorRecord } from '@/bindings/GeneratorRecord';
import { polylineBez } from '@/lib/bezpath';
import { objectBounds, unionBounds, type Bounds } from '@/lib/geometry';
import { groupChain, groupIdsOf, groupMembers } from '@/lib/groups';
import { makeLayer } from '@/lib/layers';
import { colorName, LAYER_PALETTE } from '@/lib/palette';

/** Margen (mm) entre las piezas y el borde de la cama, y separación entre piezas. */
export const BED_MARGIN_MM = 5;
export const PART_GAP_MM = 5;

/** Potencia y velocidad de la capa de rótulos que se crea sola: grabar sin atravesar el material. */
const ENGRAVE_POWER = 15;
const ENGRAVE_SPEED = 3000;

const IDENTITY: DesignObject['transform'] = [1, 0, 0, 1, 0, 0];

/** Id libre `<base>-<n>`. */
function freshId(base: string, taken: Set<string>): string {
  let i = 1;
  while (taken.has(`${base}-${i}`)) i++;
  const id = `${base}-${i}`;
  taken.add(id);
  return id;
}

/** Id de grupo libre con la convención `grupo-N` del resto de la app. */
export function freshGroupId(objects: readonly DesignObject[]): string {
  const taken = groupIdsOf(objects);
  let i = 1;
  while (taken.has(`grupo-${i}`)) i++;
  return `grupo-${i}`;
}

export interface PartObjectsOptions {
  generator: string;
  cutLayer: string;
  /** `null`: los rótulos no se insertan. */
  engraveLayer: string | null;
  /** Valor de `DesignObject.group` de todas las piezas. */
  group: string;
  /** Esquina inferior izquierda del conjunto en el documento. */
  origin: Point;
  /** Ids ya usados en el documento (se completa con los nuevos). */
  taken: Set<string>;
}

/** Un objeto vectorial de corte por pieza (y uno de grabado si hay rótulos), con las trayectorias ya en el documento. */
export function partObjects(result: GenerateResult, o: PartObjectsOptions): DesignObject[] {
  const [ox, oy] = o.origin;
  const place = (paths: GenerateResult['parts'][number]['cuts']) =>
    paths.map((c) => polylineBez(c.points.map(([x, y]): Point => [x + ox, y + oy]), c.closed));
  const make = (id: string, layer: string, paths: ReturnType<typeof place>): DesignObject => ({
    id,
    layer,
    transform: [...IDENTITY],
    kind: { type: 'vector', paths },
    locked: false,
    hidden: false,
    group: o.group,
  });
  const out: DesignObject[] = [];
  for (const part of result.parts) {
    if (part.cuts.length > 0) out.push(make(freshId(`${o.generator}-${part.key}`, o.taken), o.cutLayer, place(part.cuts)));
    if (o.engraveLayer && part.engraves.length > 0) {
      out.push(make(freshId(`${o.generator}-${part.key}-label`, o.taken), o.engraveLayer, place(part.engraves)));
    }
  }
  return out;
}

const overlaps = (a: Bounds, b: Bounds, gap: number) =>
  a[0][0] < b[1][0] + gap && b[0][0] < a[1][0] + gap && a[0][1] < b[1][1] + gap && b[0][1] < a[1][1] + gap;

/**
 * Esquina inferior izquierda de un hueco libre de `w × h` dentro de la cama (con `margin` al borde) que no toque
 * ningún `occupied` (con `gap` de separación): el primero de abajo hacia arriba y de izquierda a derecha.
 * `null` si no hay.
 */
export function freeSpot(occupied: readonly Bounds[], w: number, h: number, bed: readonly [number, number], margin = BED_MARGIN_MM, gap = PART_GAP_MM): Point | null {
  const xs = [margin, ...occupied.map((b) => b[1][0] + gap)];
  const ys = [margin, ...occupied.map((b) => b[1][1] + gap)];
  const candidates: Point[] = [];
  for (const y of new Set(ys)) for (const x of new Set(xs)) candidates.push([x, y]);
  candidates.sort((a, b) => a[1] - b[1] || a[0] - b[0]);
  for (const [x, y] of candidates) {
    if (x + w > bed[0] - margin + 1e-9 || y + h > bed[1] - margin + 1e-9) continue;
    const box: Bounds = [[x, y], [x + w, y + h]];
    if (!occupied.some((b) => overlaps(box, b, gap))) return [x, y];
  }
  return null;
}

/** Caja de todos los objetos del documento. */
export function occupiedBounds(doc: Document): Bounds[] {
  return doc.objects.map((o) => objectBounds(o)).filter((b): b is Bounds => b !== null);
}

/** Esquina inferior izquierda de las piezas de un grupo, o `null` si ya no existe. */
export function groupOrigin(doc: Document, group: string): Point | null {
  const ids = new Set(groupMembers(doc.objects, group));
  const box = unionBounds(doc.objects.filter((o) => ids.has(o.id)).map((o) => objectBounds(o)));
  return box ? [box[0][0], box[0][1]] : null;
}

/** El registro de generador al que pertenecen todos los objetos de `ids`, si lo hay. */
export function recordFor(doc: Document, ids: readonly string[]): GeneratorRecord | null {
  const records = doc.generators ?? [];
  if (records.length === 0 || ids.length === 0) return null;
  const wanted = new Set(ids);
  const picked = doc.objects.filter((o) => wanted.has(o.id));
  if (picked.length === 0) return null;
  return records.find((r) => picked.every((o) => groupChain(o.group).includes(r.group))) ?? null;
}

/** ¿El grupo conserva solo piezas con el mismo número de objetos que generó? (si no, se editó o se borró algo) */
export function groupIntact(doc: Document, record: GeneratorRecord, expected: number): boolean {
  return groupMembers(doc.objects, record.group).length === expected;
}

const sameColor = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** Primer color de la paleta que ninguna capa del documento ni los `also` usan. */
export function unusedColor(layers: readonly LayerSettings[], also: readonly string[] = []): string {
  const used = (c: string) => layers.some((l) => sameColor(l.color, c)) || also.some((a) => sameColor(a, c));
  return (LAYER_PALETTE.find((p) => !p.guide && !used(p.color)) ?? LAYER_PALETTE[0]!).color;
}

/** Capa de rótulos nueva: de líneas, con poca potencia. */
export function engraveLayerFor(color: string, name: string): LayerSettings {
  return { ...makeLayer(color, name), mode: 'line', power: ENGRAVE_POWER, speed: ENGRAVE_SPEED, passes: 1 };
}

export interface InsertPlan {
  objects: DesignObject[];
  record: GeneratorRecord;
  /** Capa de rótulos que hay que crear (si `engraveLayer` no existía). */
  newLayers: LayerSettings[];
}

/**
 * Documento con las piezas insertadas. Con `replaceGroup` se quitan antes los objetos de ese grupo (regenerar) y el
 * registro anterior se reemplaza. Crea las capas que falten. Devuelve el mismo documento si no hay objetos.
 */
export function applyInsert(doc: Document, plan: InsertPlan, replaceGroup: string | null): Document {
  if (plan.objects.length === 0) return doc;
  const drop = new Set(replaceGroup ? groupMembers(doc.objects, replaceGroup) : []);
  const have = new Set(doc.layers.map((l) => l.color.toLowerCase()));
  const layers = [...doc.layers];
  for (const color of new Set(plan.objects.map((o) => o.layer))) {
    if (have.has(color.toLowerCase())) continue;
    layers.push(plan.newLayers.find((l) => sameColor(l.color, color)) ?? makeLayer(color, colorName(color)));
  }
  const records = (doc.generators ?? []).filter((r) => r.group !== plan.record.group && r.group !== replaceGroup);
  return {
    ...doc,
    layers,
    objects: [...doc.objects.filter((o) => !drop.has(o.id)), ...plan.objects],
    generators: [...records, plan.record],
  };
}
