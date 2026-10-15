import type { NestRemnantInput } from './types';

/**
 * Retazos: lo que sobra de una hoja, guardado con su contorno real, el material y el grosor para
 * usarlo como hoja en otro acomodo. Los nombres de campo van en snake_case porque viajan tal cual
 * a Rust (`edit/remnants.rs`).
 */
export interface Remnant {
  id: string;
  name: string;
  /** Nombre del material (se compara sin distinguir mayúsculas ni acentos). */
  material: string;
  /** Grosor en mm (0 = no se indicó). */
  thickness_mm: number;
  width_mm: number;
  height_mm: number;
  /** Área neta en mm² (exterior menos huecos). */
  area_mm2: number;
  /** El contorno es (casi) un rectángulo. */
  rect: boolean;
  /** Contornos cerrados con el mínimo de la caja en (0, 0): el exterior y sus huecos. */
  paths: [number, number][][];
  /** Milisegundos desde 1970. */
  created_ms: number;
}

export interface RemnantLibrary {
  version: number;
  remnants: Remnant[];
}

/** Igual que `VERSION` y `MAX_REMNANTS` de `edit/remnants.rs`. */
export const REMNANT_LIBRARY_VERSION = 1;
export const MAX_REMNANTS = 500;
export const MAX_REMNANT_NAME = 80;
/** Los grosores se comparan con esta tolerancia (mm). */
export const THICKNESS_TOLERANCE_MM = 0.05;
/** Ancho mínimo (mm) de un retazo aprovechable que se ofrece por omisión. */
export const DEFAULT_REMNANT_MIN_MM = 20;

export const emptyRemnants = (): RemnantLibrary => ({ version: REMNANT_LIBRARY_VERSION, remnants: [] });

/** Material sin mayúsculas, acentos ni espacios de más (para compararlo). */
export const normalizeMaterial = (s: string): string =>
  s
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .trim()
    .replace(/\s+/g, ' ')
    .toLowerCase();

/** Mismo material (y no vacío) y mismo grosor (si algún lado no lo indica, 0, solo cuenta el material). */
export function sameStock(r: Pick<Remnant, 'material' | 'thickness_mm'>, material: string, thicknessMm: number): boolean {
  const m = normalizeMaterial(material);
  if (m === '' || normalizeMaterial(r.material) !== m) return false;
  if (r.thickness_mm > 0 && thicknessMm > 0) return Math.abs(r.thickness_mm - thicknessMm) <= THICKNESS_TOLERANCE_MM;
  return true;
}

/** Retazos que sirven para el material y grosor: los más chicos primero (se gastan antes que una hoja nueva). */
export function matchingRemnants(list: readonly Remnant[], material: string, thicknessMm: number): Remnant[] {
  return list
    .filter((r) => sameStock(r, material, thicknessMm))
    .sort((a, b) => a.area_mm2 - b.area_mm2 || a.id.localeCompare(b.id));
}

/** Lo que `nest_start` recibe de cada retazo. */
export const remnantInputs = (list: readonly Remnant[]): NestRemnantInput[] => list.map((r) => ({ id: r.id, paths: r.paths }));

const ringArea = (ring: readonly (readonly [number, number])[]): number => {
  let s = 0;
  for (let i = 0; i < ring.length; i++) {
    const [x0, y0] = ring[i]!;
    const [x1, y1] = ring[(i + 1) % ring.length]!;
    s += x0 * y1 - x1 * y0;
  }
  return Math.abs(s) / 2;
};

/** Área neta de un retazo: el primer contorno es el exterior y los demás, huecos (como los entrega el motor). */
export function netArea(paths: readonly (readonly [number, number][])[]): number {
  const [outer, ...holes] = paths;
  if (!outer) return 0;
  return Math.max(0, ringArea(outer) - holes.reduce((a, h) => a + ringArea(h), 0));
}

export interface LeftoverForm {
  material: string;
  thicknessMm: number;
  /** Nombre; si falta se arma con el material, el grosor y las medidas. */
  name?: string;
  id: string;
  now: number;
}

const round1 = (n: number) => Math.round(n * 10) / 10;

/** Nombre automático: «Triplay 3 mm · 420×180». */
export function autoName(material: string, thicknessMm: number, w: number, h: number): string {
  const base = material.trim() || 'Retazo';
  const thick = thicknessMm > 0 ? ` ${round1(thicknessMm)} mm` : '';
  return `${base}${thick} · ${Math.round(w)}×${Math.round(h)}`.slice(0, MAX_REMNANT_NAME);
}

/**
 * Un retazo a partir de un sobrante del motor (contornos cerrados en las coordenadas de la hoja):
 * se lleva al origen y se miden su caja y su área. `null` si no tiene un contorno válido.
 */
export function remnantFromLeftover(rings: readonly [number, number][][] | readonly (readonly [number, number][])[], form: LeftoverForm): Remnant | null {
  const valid = rings.filter((r) => r.length >= 3 && r.every(([x, y]) => Number.isFinite(x) && Number.isFinite(y)));
  if (valid.length === 0) return null;
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  for (const ring of valid) {
    for (const [x, y] of ring) {
      x0 = Math.min(x0, x);
      y0 = Math.min(y0, y);
      x1 = Math.max(x1, x);
      y1 = Math.max(y1, y);
    }
  }
  const [w, h] = [x1 - x0, y1 - y0];
  if (!(w > 0 && h > 0)) return null;
  const paths = valid.map((ring) => ring.map(([x, y]): [number, number] => [Math.round((x - x0) * 1e4) / 1e4, Math.round((y - y0) * 1e4) / 1e4]));
  const area = netArea(paths);
  if (!(area > 0)) return null;
  const material = form.material.trim();
  return {
    id: form.id,
    name: (form.name?.trim() || autoName(material, form.thicknessMm, w, h)).slice(0, MAX_REMNANT_NAME),
    material,
    thickness_mm: Math.max(0, Number.isFinite(form.thicknessMm) ? form.thicknessMm : 0),
    width_mm: Math.round(w * 100) / 100,
    height_mm: Math.round(h * 100) / 100,
    area_mm2: Math.round(area * 100) / 100,
    rect: paths.length === 1 && area >= w * h * 0.999,
    paths,
    created_ms: form.now,
  };
}

/** Id libre `retazo-<n>`. */
export function freshRemnantId(taken: ReadonlySet<string>, now: number): string {
  let i = 1;
  const stamp = now.toString(36);
  while (taken.has(`retazo-${stamp}-${i}`)) i++;
  return `retazo-${stamp}-${i}`;
}
