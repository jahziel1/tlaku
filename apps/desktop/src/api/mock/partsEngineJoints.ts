/**
 * Uniones y caja redondeada SIMULADAS para el modo mock (`crates/tlaku-parts/src/joints.rs` y
 * `rounded_box.rs` son los reales). La cola de milano y la pestaña-ranura siguen la misma
 * geometría; la caja redondeada es solo una aproximación (contorno liso sin dientes y un lateral
 * del largo correcto con cortes rectos en las esquinas) y lo avisa. Los patrones ladrillo y rombo
 * de la bisagra tampoco se simulan: salen como rectas y el motor real los dibuja bien.
 */
import type { PathDto } from '@/bindings/PathDto';
import { Invalid, type Generated, type Params } from './partsEngine';

type P = [number, number];

const num = (p: Params, k: string, d: number) => (typeof p[k] === 'number' ? (p[k] as number) : d);
const str = (p: Params, k: string, d: string) => (typeof p[k] === 'string' ? (p[k] as string) : d);
const bool = (p: Params, k: string, d: boolean) => (typeof p[k] === 'boolean' ? (p[k] as boolean) : d);

const closed = (points: P[]): PathDto => ({ points, closed: true });

/** Lleva los contornos a su origen y devuelve la caja que ocupan. */
function normalize(paths: PathDto[]): { cuts: PathDto[]; width: number; height: number } {
  const xs = paths.flatMap((c) => c.points.map((q) => q[0]));
  const ys = paths.flatMap((c) => c.points.map((q) => q[1]));
  const [x0, y0] = [Math.min(...xs), Math.min(...ys)];
  return {
    cuts: paths.map((c) => ({ ...c, points: c.points.map(([x, y]): P => [x - x0, y - y0]) })),
    width: Math.max(...xs) - x0,
    height: Math.max(...ys) - y0,
  };
}

const MIN_LAND = 1.5;

export function jointsParts(p: Params): Generated {
  const kind = str(p, 'kind', 'dovetail');
  const length = num(p, 'length_mm', 60);
  const plate = num(p, 'plate_mm', 40);
  const count = Math.round(num(p, 'count', 3));
  const fit = num(p, 'fit_mm', 0);
  const pitch = length / count;
  const center = (i: number) => (i + 0.5) * pitch;
  const warnings: string[] = [];
  if (fit !== 0) warnings.push('Ya hay holgura de encaje: no la sumes a un kerf de capa que compense lo mismo. Usa solo uno de los dos y calibra con la pieza de prueba de encaje.');
  if (kind === 'tab_slot') {
    const tab = num(p, 'tab_mm', 10);
    const t = num(p, 'thickness_mm', 3);
    const offset = num(p, 'offset_mm', 20);
    const slotW = tab + fit;
    const slotH = t + fit;
    if (slotW > pitch - MIN_LAND) throw new Invalid('tab_mm', `La ranura de cada pestaña debe dejar material entre una y otra: la pestaña no puede medir más de ${Math.max(pitch - MIN_LAND - fit, 0).toFixed(1)} mm con ese número de pestañas.`);
    if (offset - slotH / 2 < MIN_LAND || offset + slotH / 2 > plate - MIN_LAND) throw new Invalid('offset_mm', 'La ranura debe quedar lejos del borde de la placa para dejar material a los lados.');
    const tabs: P[] = [[0, 0]];
    for (let i = 0; i < count; i++) tabs.push([center(i) - tab / 2, 0], [center(i) - tab / 2, -t], [center(i) + tab / 2, -t], [center(i) + tab / 2, 0]);
    tabs.push([length, 0], [length, plate], [0, plate]);
    const slots: PathDto[] = [closed([[0, 0], [length, 0], [length, plate], [0, plate]])];
    for (let i = 0; i < count; i++) {
      const [x0, x1] = [center(i) - slotW / 2, center(i) + slotW / 2];
      const [y0, y1] = [offset - slotH / 2, offset + slotH / 2];
      slots.push(closed([[x0, y0], [x0, y1], [x1, y1], [x1, y0]]));
    }
    const a = normalize([closed(tabs)]);
    const b = normalize(slots);
    if (tab < t) warnings.push(`Las pestañas miden ${tab} mm, menos que el grosor (${t} mm): son frágiles. Sube su ancho.`);
    return {
      parts: [
        { key: 'tab_plate', name: 'Pestaña-ranura: placa con pestañas', engraves: [], ...a },
        { key: 'slot_plate', name: 'Pestaña-ranura: placa con ranuras', engraves: [], ...b },
      ],
      warnings,
    };
  }
  const neck = num(p, 'neck_mm', 8);
  const depth = num(p, 'depth_mm', 8);
  const alpha = (num(p, 'angle_deg', 12) * Math.PI) / 180;
  const e = fit / 2;
  const nh = neck / 2;
  const th = nh + depth * Math.tan(alpha);
  const snh = nh + e / Math.cos(alpha);
  const sd = depth + e;
  const sth = snh + sd * Math.tan(alpha);
  if (2 * sth > pitch - MIN_LAND) throw new Invalid('count', `Las colas no caben: cada una (con su ranura) debe medir como máximo ${Math.max(pitch - MIN_LAND, 0).toFixed(1)} mm de ancho en la punta para dejar material entre ranuras. Baja el cuello, el ángulo o la profundidad, o el número de colas.`);
  if (sd + MIN_LAND + 0.5 > plate) throw new Invalid('depth_mm', `La ranura es demasiado profunda para la placa: la profundidad no puede pasar de ${(plate - 2).toFixed(1)} mm.`);
  const a: P[] = [[-plate, 0], [0, 0]];
  for (let i = 0; i < count; i++) a.push([0, center(i) - nh], [depth, center(i) - th], [depth, center(i) + th], [0, center(i) + nh]);
  a.push([0, length], [-plate, length]);
  const b: P[] = [[0, 0], [plate, 0], [plate, length], [0, length]];
  for (let i = count - 1; i >= 0; i--) b.push([0, center(i) + snh], [sd, center(i) + sth], [sd, center(i) - sth], [0, center(i) - snh]);
  if (neck < 4) warnings.push(`El cuello de ${neck} mm es muy fino: la cola puede romperse. Sube el ancho del cuello.`);
  warnings.unshift('Las dos placas se ensamblan deslizando una sobre la otra a lo largo de la arista; la forma de cola impide separarlas tirando en perpendicular, pero no al deslizar. Para fijarlas, una gota de pegamento o un tope.');
  return {
    parts: [
      { key: 'dovetail_tails', name: 'Cola de milano: placa con colas', engraves: [], ...normalize([closed(a)]) },
      { key: 'dovetail_sockets', name: 'Cola de milano: placa con ranuras', engraves: [], ...normalize([closed(b)]) },
    ],
    warnings,
  };
}

/** Rectángulo `w x h` con esquinas de radio `r`, antihorario, con el origen en su esquina inferior izquierda. */
function roundedRect(w: number, h: number, r: number): PathDto {
  const pts: P[] = [];
  const corners: [number, number, number][] = [[w - r, r, -Math.PI / 2], [w - r, h - r, 0], [r, h - r, Math.PI / 2], [r, r, Math.PI]];
  for (const [cx, cy, a0] of corners) {
    for (let k = 0; k <= 8; k++) {
      const a = a0 + (Math.PI / 2) * (k / 8);
      pts.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
    }
  }
  return closed(pts);
}

export function roundedBoxParts(p: Params): Generated {
  const w = num(p, 'width_mm', 100);
  const d = num(p, 'depth_mm', 70);
  const h = num(p, 'height_mm', 40);
  const r = num(p, 'radius_mm', 15);
  const t = num(p, 'thickness_mm', 3);
  const lid = bool(p, 'lid', true);
  if (r < t + 2) throw new Invalid('radius_mm', `El radio de las esquinas debe ser de al menos ${(t + 2).toFixed(1)} mm (el grosor más 2 mm) para que el lateral pueda curvarse por dentro.`);
  if (r > Math.min(w, d) / 2) throw new Invalid('radius_mm', 'El radio de las esquinas no puede pasar de la mitad del lado menor.');
  const rm = r - t / 2;
  // Perímetro de la línea media (la que no se estira al curvar el lateral).
  const per = 2 * (w - t + (d - t)) - 8 * rm + 2 * Math.PI * rm;
  const straightHalf = (w - t) / 2 - rm;
  const sideLen = d - t - 2 * rm;
  const quarter = (Math.PI / 2) * rm;
  // Inicio (a lo largo de la línea media) de cada una de las cuatro esquinas.
  const a1 = straightHalf;
  const a2 = a1 + quarter + sideLen;
  const a3 = a2 + quarter + 2 * straightHalf;
  const a4 = a3 + quarter + sideLen;
  const starts = [a1, a2, a3, a4];
  const margin = num(p, 'margin_mm', 3);
  const gap = Math.max(num(p, 'row_gap_mm', 2.5), 0.4);
  const y0 = t + margin;
  const y1 = h - (lid ? t : 0) - margin;
  const cuts: PathDto[] = [closed([[0, 0], [per, 0], [per, h], [0, h]])];
  if (y1 - y0 > 1) {
    for (const s of starts) {
      const rows = Math.max(1, Math.floor((quarter - 2 * margin) / gap) + 1);
      for (let i = 0; i < rows; i++) {
        const x = s + quarter / 2 + (i - (rows - 1) / 2) * gap;
        // Filas alternas con cortes más cortos (escalonadas, como las rectas del motor).
        const off = i % 2 === 0 ? 0 : (y1 - y0) / 6;
        cuts.push({ points: [[x, y0 + off], [x, y1 - off]], closed: false });
      }
    }
  }
  const wall = normalize(cuts);
  const plate = normalize([roundedRect(w, d, r)]);
  const parts = [{ key: 'base', name: 'Base', engraves: [], ...plate }];
  if (lid) parts.push({ key: 'lid', name: 'Tapa', engraves: [], ...plate });
  parts.push({ key: 'wall', name: 'Lateral con bisagra', engraves: [], ...wall });
  return {
    parts,
    warnings: [
      `Vista previa aproximada (modo simulado): sin dientes ni cierre y con cortes rectos en las esquinas. El lateral mide ${per.toFixed(1)} mm, el perímetro de la línea media del grosor. El motor real dibuja todo.`,
    ],
  };
}
