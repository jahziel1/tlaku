/**
 * Generadores de piezas SIMULADOS para el modo mock (`crates/tlaku-parts` es el real). Siguen el mismo
 * diseño (rectángulo menos muescas, mismas reglas de aristas y mismo reparto de celdas), pero con
 * menos validaciones y sin la tapa deslizante (sale como caja abierta con una tapa lisa). Sirven
 * para probar el diálogo, la inserción y las capturas sin el motor; la verdad de la geometría y del
 * encaje es la del motor Rust.
 */
import type { GenerateResult } from '@/bindings/GenerateResult';
import type { PlacedPart } from '@/bindings/PlacedPart';
import type { PathDto } from '@/bindings/PathDto';
import schema from './partsSchema.json';
import { gearsParts } from './partsGears';

type P = [number, number];
type V3 = [number, number, number];
interface Rect { x0: number; y0: number; x1: number; y1: number }
interface Part { key: string; name: string; cuts: PathDto[]; engraves: PathDto[]; width: number; height: number }
type Params = Record<string, unknown>;

const num = (p: Params, k: string, d: number) => (typeof p[k] === 'number' ? (p[k] as number) : d);
const str = (p: Params, k: string, d: string) => (typeof p[k] === 'string' ? (p[k] as string) : d);
const bool = (p: Params, k: string, d: boolean) => (typeof p[k] === 'boolean' ? (p[k] as boolean) : d);

class Invalid extends Error {
  constructor(readonly field: string | null, message: string) {
    super(message);
  }
}

// ---- rectángulo menos rectángulos -------------------------------------------------------------

const SNAP = 1e-9;
const axis = (v: number[]) => {
  const s = [...v].sort((a, b) => a - b);
  return s.filter((x, i) => i === 0 || x - s[i - 1]! > SNAP);
};

export function rectMinusRects(w: number, h: number, holes: Rect[]): PathDto[] {
  const xs = axis([0, w, ...holes.flatMap((r) => [Math.min(Math.max(r.x0, 0), w), Math.min(Math.max(r.x1, 0), w)])]);
  const ys = axis([0, h, ...holes.flatMap((r) => [Math.min(Math.max(r.y0, 0), h), Math.min(Math.max(r.y1, 0), h)])]);
  const nx = xs.length - 1;
  const ny = ys.length - 1;
  const occ = (i: number, j: number) => {
    if (i < 0 || j < 0 || i >= nx || j >= ny) return false;
    const cx = (xs[i]! + xs[i + 1]!) / 2;
    const cy = (ys[j]! + ys[j + 1]!) / 2;
    return !holes.some((r) => cx > r.x0 && cx < r.x1 && cy > r.y0 && cy < r.y1);
  };
  const next = new Map<string, [number, number][]>();
  const add = (a: [number, number], b: [number, number]) => {
    const k = `${a[0]},${a[1]}`;
    next.set(k, [...(next.get(k) ?? []), b]);
  };
  for (let i = 0; i < nx; i++) {
    for (let j = 0; j < ny; j++) {
      if (!occ(i, j)) continue;
      if (!occ(i, j - 1)) add([i, j], [i + 1, j]);
      if (!occ(i + 1, j)) add([i + 1, j], [i + 1, j + 1]);
      if (!occ(i, j + 1)) add([i + 1, j + 1], [i, j + 1]);
      if (!occ(i - 1, j)) add([i, j + 1], [i, j]);
    }
  }
  const loops: PathDto[] = [];
  for (const startKey of [...next.keys()].sort()) {
    for (;;) {
      const first = next.get(startKey)?.pop();
      if (!first) break;
      const start = startKey.split(',').map(Number) as [number, number];
      const ring: [number, number][] = [start];
      let prev = start;
      let cur = first;
      while (cur[0] !== start[0] || cur[1] !== start[1]) {
        ring.push(cur);
        const outs = next.get(`${cur[0]},${cur[1]}`)!;
        let pick = 0;
        if (outs.length > 1) {
          const dx = cur[0] - prev[0];
          const dy = cur[1] - prev[1];
          const turn = (o: [number, number]) => dx * (o[1] - cur[1]) - dy * (o[0] - cur[0]);
          pick = outs.reduce((best, o, k) => (turn(o) > turn(outs[best]!) ? k : best), 0);
        }
        const nxt = outs.splice(pick, 1)[0]!;
        prev = cur;
        cur = nxt;
      }
      const n = ring.length;
      const dir = (a: [number, number], b: [number, number]) => Math.sign(b[0] - a[0]) * 2 + Math.sign(b[1] - a[1]);
      const keep = ring.filter((b, k) => dir(ring[(k + n - 1) % n]!, b) !== dir(b, ring[(k + 1) % n]!));
      if (keep.length >= 3) loops.push({ points: keep.map(([i, j]) => [xs[i]!, ys[j]!] as P), closed: true });
    }
  }
  return loops;
}

// ---- dientes ----------------------------------------------------------------------------------

interface Cells { start: number; len: number; n: number }
const planCells = (start: number, len: number, target: number): Cells => {
  const ideal = Math.max(len / target, 1);
  const f = Math.floor(ideal);
  const below = Math.max(f % 2 === 1 ? f : Math.max(f - 1, 0), 3);
  const above = below + 2;
  const err = (n: number) => Math.abs(len / n - target);
  return { start, len, n: err(above) < err(below) && ideal > below ? above : below };
};
const cellAt = (c: Cells, k: number) => c.start + (c.len * k) / c.n;

// ---- caja -------------------------------------------------------------------------------------

type Id = 'front' | 'back' | 'left' | 'right' | 'bottom' | 'top' | 'lid';
interface Panel { id: Id; w: number; h: number; o: V3; u: V3; v: V3; rects: Rect[]; voids: Rect[] }

const to3 = (p: Panel, u: number, v: number): V3 => [0, 1, 2].map((k) => p.o[k]! + u * p.u[k]! + v * p.v[k]!) as V3;
const local = (p: Panel, q: V3): P => {
  const d = [0, 1, 2].map((k) => q[k]! - p.o[k]!);
  return [d.reduce((a, x, k) => a + x * p.u[k]!, 0), d.reduce((a, x, k) => a + x * p.v[k]!, 0)];
};
const sideEnds = (p: Panel, s: number): [V3, V3] => {
  const { w, h } = p;
  const e: [P, P] = [[[0, 0], [w, 0]], [[w, 0], [w, h]], [[w, h], [0, h]], [[0, h], [0, 0]]][s] as [P, P];
  return [to3(p, ...e[0]), to3(p, ...e[1])];
};
const edgeOf = (a: V3, b: V3) => {
  const ax = [0, 1, 2].find((k) => Math.abs(a[k]! - b[k]!) > 1e-9);
  if (ax === undefined) return null;
  const o = [0, 1, 2].filter((k) => k !== ax).map((k) => Math.round(a[k]! * 1e6));
  return { key: `${ax}:${o[0]}:${o[1]}`, axis: ax, lo: Math.min(a[ax]!, b[ax]!), hi: Math.max(a[ax]!, b[ax]!) };
};
const isFB = (id: Id) => id === 'front' || id === 'back';
const ownsEven = (me: Id, other: Id) => (isFB(me) ? true : isFB(other) ? false : me === 'left' || me === 'right');

function boxParts(p: Params): Generated {
  const kind = str(p, 'kind', 'closed');
  const dims = str(p, 'dims', 'outer');
  const t = num(p, 'thickness_mm', 3);
  const finger = num(p, 'finger_mm', 10);
  const fit = num(p, 'fit_mm', 0);
  let w = num(p, 'width_mm', 100);
  let d = num(p, 'depth_mm', 80);
  let h = num(p, 'height_mm', 60);
  if (dims === 'inner') {
    w += 2 * t;
    d += 2 * t;
    h += kind === 'closed' ? 2 * t : t;
  }
  const min = 2 * t + 4.5;
  for (const [f, v] of [['width_mm', w], ['depth_mm', d], ['height_mm', h]] as const) {
    if (v < min) throw new Invalid(f, `La caja es demasiado pequeña para dientes de este grosor: cada medida debe ser de al menos ${min.toFixed(1)} mm.`);
  }
  if (Math.max(w, d, h) / finger > 150) throw new Invalid('finger_mm', 'Habría más de 150 dientes por arista: sube el ancho de diente.');
  const lidKind = kind === 'closed' ? 'top' : kind === 'sliding_lid' ? 'lid' : null;
  const ids: Id[] = ['front', 'back', 'left', 'right', 'bottom', ...(lidKind ? [lidKind as Id] : [])];
  const frame = (id: Id): [number, number, V3, V3, V3] => {
    switch (id) {
      case 'front': return [w, h, [0, 0, 0], [1, 0, 0], [0, 0, 1]];
      case 'back': return [w, h, [w, d, 0], [-1, 0, 0], [0, 0, 1]];
      case 'left': return [d, h, [0, d, 0], [0, -1, 0], [0, 0, 1]];
      case 'right': return [d, h, [w, 0, 0], [0, 1, 0], [0, 0, 1]];
      case 'bottom': return [w, d, [0, d, 0], [1, 0, 0], [0, -1, 0]];
      case 'top': return [w, d, [0, 0, h], [1, 0, 0], [0, 1, 0]];
      default: return [w - 0.2, d - 2 * t, [0.1, 2 * t, h], [1, 0, 0], [0, 1, 0]];
    }
  };
  const panels: Panel[] = ids.map((id) => {
    const [pw, ph, o, u, v] = frame(id);
    return { id, w: pw, h: ph, o, u, v, rects: [], voids: [] };
  });
  const joined = panels.filter((x) => x.id !== 'lid');
  for (const panel of joined) {
    for (let s = 0; s < 4; s++) {
      const [a3, b3] = sideEnds(panel, s);
      const mine = edgeOf(a3, b3);
      if (!mine) continue;
      for (const other of joined) {
        if (other === panel) continue;
        let theirs: ReturnType<typeof edgeOf> = null;
        for (let s2 = 0; s2 < 4 && !theirs; s2++) {
          const [c, e] = sideEnds(other, s2);
          const found = edgeOf(c, e);
          if (found && found.key === mine.key) theirs = found;
        }
        if (!theirs) continue;
        let from = Math.max(mine.lo, theirs.lo);
        let to = Math.min(mine.hi, theirs.hi);
        if (mine.axis === 1) {
          from += t;
          to -= t;
        }
        if (to - from <= 0) continue;
        const cells = planCells(from, to - from, finger);
        const meEven = ownsEven(panel.id, other.id);
        for (let k = 0; k < cells.n; k++) {
          if ((k % 2 === 0) === meEven) continue;
          const along = (c: number) => {
            const pt = [...a3] as V3;
            pt[mine.axis] = c;
            const [lu, lv] = local(panel, pt);
            return s % 2 === 0 ? lu : lv;
          };
          const sideLen = s % 2 === 0 ? panel.w : panel.h;
          const x = Math.max(Math.min(along(cellAt(cells, k)), along(cellAt(cells, k + 1))) - fit / 4, 0);
          const y = Math.min(Math.max(along(cellAt(cells, k)), along(cellAt(cells, k + 1))) + fit / 4, sideLen);
          const { w: pw, h: ph } = panel;
          panel.rects.push(s === 0 ? { x0: x, y0: 0, x1: y, y1: t } : s === 1 ? { x0: pw - t, y0: x, x1: pw, y1: y } : s === 2 ? { x0: x, y0: ph - t, x1: y, y1: ph } : { x0: 0, y0: x, x1: t, y1: y });
        }
        break;
      }
    }
  }
  const labels = bool(p, 'labels', false);
  const names: Record<Id, string> = { front: 'Frente', back: 'Atrás', left: 'Lateral izquierdo', right: 'Lateral derecho', bottom: 'Base', top: 'Tapa', lid: 'Tapa deslizante' };
  const marks: Record<Id, string> = { front: 'FRENTE', back: 'ATRAS', left: 'IZQ', right: 'DER', bottom: 'BASE', top: 'TAPA', lid: 'TAPA' };
  const parts: Part[] = panels.map((panel) => {
    const cuts = rectMinusRects(panel.w, panel.h, [...panel.rects, ...panel.voids]);
    const engraves = labels ? textCentered(marks[panel.id], panel.w / 2, panel.h / 2, Math.min(6, Math.max(2.5, Math.min(panel.w, panel.h) * 0.1))) : [];
    return { key: panel.id, name: names[panel.id], cuts, engraves, width: panel.w, height: panel.h };
  });
  const warnings = [];
  if (kind === 'sliding_lid') warnings.push('Modo simulado: la tapa deslizante se dibuja como tapa lisa sobre una caja abierta.');
  if (fit !== 0) warnings.push('Ya hay holgura de encaje: no la sumes a un kerf de capa que compense lo mismo.');
  return { parts, warnings };
}

// ---- bisagra ----------------------------------------------------------------------------------

const tri = (phase: number) => {
  const x = (((phase / (2 * Math.PI)) % 1) + 1) % 1;
  return x < 0.25 ? 4 * x : x < 0.75 ? 2 - 4 * x : 4 * x - 4;
};

function hingeParts(p: Params): Generated {
  const pattern = str(p, 'pattern', 'straight');
  const vertical = str(p, 'orientation', 'horizontal') === 'vertical';
  const W = num(p, 'width_mm', 100);
  const H = num(p, 'height_mm', 50);
  const L = num(p, 'cut_length_mm', 20);
  const bridge = num(p, 'bridge_mm', 3);
  const gap = num(p, 'row_gap_mm', 2);
  const m = num(p, 'margin_mm', 3);
  const amp = pattern === 'straight' ? 0 : num(p, 'amplitude_mm', 1.5);
  const lambda = num(p, 'wavelength_mm', 10);
  const radius = num(p, 'corner_radius_mm', 0);
  const [w, h] = vertical ? [H, W] : [W, H];
  const lo = m;
  const hi = w - m;
  const yLo = m + amp;
  const yHi = h - m - amp;
  if (hi - lo < 0.8 || yHi < yLo) throw new Invalid(null, 'La zona es demasiado pequeña para su margen: no cabe ningún corte.');
  const rows = Math.floor((yHi - yLo) / gap) + 1;
  if (rows * (Math.ceil((hi - lo) / (L + bridge)) + 1) > 20000) throw new Invalid(null, 'La bisagra tendría más de 20000 cortes: baja el tamaño de la zona o sube el largo de los cortes y la separación entre filas.');
  const yc = (yLo + yHi) / 2;
  const pitch = L + bridge;
  const kMax = Math.ceil((hi - lo) / pitch) + 1;
  const omega = (2 * Math.PI) / lambda;
  const cuts: PathDto[] = [];
  if (radius > 0) {
    if (radius > Math.min(W, H) / 2) throw new Invalid('corner_radius_mm', 'El radio de las esquinas no puede pasar de la mitad del lado menor.');
  }
  if (bool(p, 'outline', true)) cuts.push(roundedOutline(W, H, radius));
  const slits: PathDto[] = [];
  for (let r = 0; r < rows; r++) {
    const y0 = yc + (r - (rows - 1) / 2) * gap;
    const shift = r % 2 === 0 ? 0 : 0.5;
    for (let k = -kMax; k <= kMax; k++) {
      const c = w / 2 + (k + shift) * pitch;
      const a = Math.max(c - L / 2, lo);
      const b = Math.min(c + L / 2, hi);
      if (b - a < 0.8) continue;
      const xs = [a];
      if (pattern !== 'straight') {
        const step = pattern === 'wave' ? lambda / 24 : lambda / 4;
        for (let i = Math.floor((a - w / 2) / step) + 1; w / 2 + i * step < b - 1e-9; i++) {
          const x = w / 2 + i * step;
          if (x > a + 1e-9) xs.push(x);
        }
      }
      xs.push(b);
      const yAt = (x: number) => y0 + amp * (pattern === 'wave' ? Math.sin(omega * (x - w / 2)) : pattern === 'zigzag' ? tri(omega * (x - w / 2)) : 0);
      const pts = xs.map((x): P => (vertical ? [yAt(x), x] : [x, yAt(x)]));
      slits.push({ points: pts, closed: false });
    }
  }
  if (slits.length === 0) throw new Invalid(null, 'La zona es demasiado pequeña para su margen: no cabe ningún corte.');
  const warnings = bridge < 1 ? [`Los puentes de ${bridge} mm son muy finos: la bisagra puede rasgarse al flexionar.`] : [];
  return { parts: [{ key: 'hinge', name: 'Bisagra flexible', cuts: [...cuts, ...slits], engraves: [], width: W, height: H }], warnings };
}

function roundedOutline(w: number, h: number, r: number): PathDto {
  if (r <= 1e-9) return { points: [[0, 0], [w, 0], [w, h], [0, h]], closed: true };
  const n = 8;
  const pts: P[] = [];
  const corners: [number, number, number][] = [[w - r, r, -Math.PI / 2], [w - r, h - r, 0], [r, h - r, Math.PI / 2], [r, r, Math.PI]];
  for (const [cx, cy, a0] of corners) {
    for (let k = 0; k <= n; k++) {
      const a = a0 + (Math.PI / 2) * (k / n);
      pts.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
    }
  }
  return { points: pts, closed: true };
}

// ---- prueba de encaje -------------------------------------------------------------------------

function fitParts(p: Params): Generated {
  const mode = str(p, 'mode', 'thickness');
  const T = num(p, 'thickness_mm', 3);
  const F = num(p, 'finger_mm', 10);
  const step = num(p, 'step_mm', 0.05);
  const count = Math.round(num(p, 'count', 4));
  const len = num(p, 'slot_length_mm', 12);
  const labels = bool(p, 'labels', true);
  const nominal = mode === 'finger' ? F : T;
  if (nominal - count * step < 0.3) throw new Invalid('step_mm', 'Con ese paso y ese número de ranuras la más angosta queda por debajo de 0,3 mm.');
  const offsets = Array.from({ length: 2 * count + 1 }, (_, i) => Math.round((i - count) * step * 1000) / 1000);
  const text = (o: number) => (Math.abs(o) < 1e-9 ? '0.00' : `${o > 0 ? '+' : '-'}${Math.abs(o).toFixed(2)}`);
  const labelW = Math.max(...offsets.map((o) => textWidth(text(o), 3)));
  const pitch = Math.max(nominal + offsets[offsets.length - 1]! + 5, labelW + 3, 9);
  const margin = 5;
  const plateW = 2 * margin + pitch * offsets.length;
  const y0 = 11;
  const y1 = y0 + len;
  const plateH = y1 + 11;
  const holes: Rect[] = [];
  const engraves: PathDto[] = [];
  offsets.forEach((o, i) => {
    const cx = margin + pitch * (i + 0.5);
    const wSlot = nominal + o;
    holes.push({ x0: cx - wSlot / 2, y0, x1: cx + wSlot / 2, y1 });
    if (labels) engraves.push(...textCentered(text(o), cx, 5.5, 3));
  });
  const head = `${mode === 'finger' ? 'F' : 'T'}=${nominal.toFixed(2)}`;
  if (labels) engraves.push(...textStrokes(head, margin, y1 + 4, 3));
  const plate: Part = { key: 'plate', name: 'Placa de ranuras', cuts: rectMinusRects(plateW, plateH, holes).sort((a, b) => b.points.length - a.points.length), engraves, width: plateW, height: plateH };
  const tabW = mode === 'finger' ? F : len - 1.5;
  const tabLen = mode === 'finger' ? Math.min(len, 10) : 8;
  const gripW = Math.max(tabW, len) + 8;
  const x0 = (gripW - tabW) / 2;
  const x1 = (gripW + tabW) / 2;
  const tab: Part = {
    key: 'tab',
    name: 'Macho',
    cuts: [{ points: [[0, 0], [gripW, 0], [gripW, 8], [x1, 8], [x1, 8 + tabLen], [x0, 8 + tabLen], [x0, 8], [0, 8]], closed: true }],
    engraves: labels ? textCentered(head, gripW / 2, 4, 3) : [],
    width: gripW,
    height: 8 + tabLen,
  };
  return { parts: [plate, tab], warnings: [] };
}

// ---- rótulos (misma tabla de trazos que el motor) ---------------------------------------------

const GLYPHS: Record<string, [number, string]> = {
  ' ': [2, ''], A: [2, '00 03 14 23 20|02 22'], B: [2, '00 04 14 23 12 02|12 21 10 00'], C: [2, '24 04 00 20'],
  D: [2, '00 04 14 23 21 10 00'], E: [2, '24 04 00 20|02 12'], F: [2, '00 04 24|02 12'], G: [2, '24 04 00 20 22 12'],
  H: [2, '04 00|02 22|24 20'], I: [2, '04 24|14 10|00 20'], J: [2, '24 21 10 01'], K: [2, '04 00|24 02 20'],
  L: [2, '04 00 20'], M: [2, '00 04 12 24 20'], N: [2, '00 04 20 24'], O: [2, '00 04 24 20 00'], P: [2, '00 04 24 22 02'],
  Q: [2, '00 04 24 20 00|11 20'], R: [2, '00 04 24 22 02|12 20'], S: [2, '24 04 02 22 20 00'], T: [2, '04 24|14 10'],
  U: [2, '04 00 20 24'], V: [2, '04 10 24'], W: [2, '04 00 12 20 24'], X: [2, '04 20|00 24'], Y: [2, '04 12 24|12 10'],
  Z: [2, '04 24 00 20'], '0': [2, '00 04 24 20 00|00 24'], '1': [2, '03 14 10|00 20'], '2': [2, '04 24 22 02 00 20'],
  '3': [2, '04 24 20 00|02 22'], '4': [2, '04 02 22|24 20'], '5': [2, '24 04 02 22 20 00'], '6': [2, '24 04 00 20 22 02'],
  '7': [2, '04 24 20'], '8': [2, '00 04 24 20 00|02 22'], '9': [2, '20 24 04 02 22'], '.': [1, '00 01'], '-': [2, '02 22'],
  '+': [2, '02 22|13 11'], '=': [2, '01 21|03 23'], '/': [2, '00 24'], x: [2, '01 23|03 21'],
};
const glyphOf = (ch: string) => GLYPHS[ch === 'x' ? 'x' : ch.normalize('NFD').replace(/[̀-ͯ]/g, '').toUpperCase()];
const textWidth = (text: string, height: number) => {
  const units = [...text].map(glyphOf).filter(Boolean).reduce((a, g) => a + g![0] + 1, 0);
  return Math.max(units - 1, 0) * (height / 4);
};
function textStrokes(text: string, x: number, y: number, height: number): PathDto[] {
  const s = height / 4;
  let cursor = 0;
  const out: PathDto[] = [];
  for (const g of [...text].map(glyphOf)) {
    if (!g) continue;
    for (const stroke of g[1].split('|').filter(Boolean)) {
      out.push({ points: stroke.split(' ').map((q): P => [x + (cursor + Number(q[0])) * s, y + Number(q[1]) * s]), closed: false });
    }
    cursor += g[0] + 1;
  }
  return out;
}
const textCentered = (text: string, cx: number, cy: number, height: number) => textStrokes(text, cx - textWidth(text, height) / 2, cy - height / 2, height);

// ---- acomodo y fachada ------------------------------------------------------------------------

interface Generated { parts: Part[]; warnings: string[] }

function validateRanges(id: string, params: Params): void {
  const info = (schema as { id: string; params: { key: string; kind: { type: string; min?: number; max?: number } }[] }[]).find((g) => g.id === id);
  for (const spec of info?.params ?? []) {
    const v = params[spec.key];
    if (spec.kind.min === undefined || spec.kind.max === undefined || v === undefined) continue;
    if (typeof v !== 'number' || Number.isNaN(v) || v < spec.kind.min || v > spec.kind.max) {
      throw new Invalid(spec.key, `Debe estar entre ${spec.kind.min} y ${spec.kind.max}.`);
    }
  }
}

const shift = (paths: PathDto[], dx: number, dy: number): PathDto[] => paths.map((p) => ({ ...p, points: p.points.map(([x, y]): P => [x + dx, y + dy]) }));

/** Genera y acomoda en filas, como `tlaku_parts::generate`. */
export function mockGenerate(id: string, params: Params, maxWidth: number | null | undefined, gapMm: number | null | undefined): GenerateResult {
  const gap = typeof gapMm === 'number' && Number.isFinite(gapMm) ? Math.min(Math.max(gapMm, 0), 100) : 5;
  if (!['box', 'hinge', 'fit_test', 'gears'].includes(id)) throw new Error(`No existe el generador «${id}».`);
  try {
    // Como el motor: lo que falte toma el valor de fábrica (el mismo JSON que da el formulario).
    const defaults = (schema as unknown as { id: string; defaults: Params }[]).find((g) => g.id === id)?.defaults ?? {};
    const full: Params = { ...defaults, ...params };
    validateRanges(id, full);
    const g = id === 'box' ? boxParts(full) : id === 'hinge' ? hingeParts(full) : id === 'gears' ? gearsParts(full, (f, m) => new Invalid(f, m)) : fitParts(full);
    const max = maxWidth && maxWidth > 0 ? maxWidth : Infinity;
    let x = 0, y = 0, rowH = 0, width = 0;
    const placed: PlacedPart[] = g.parts.map((p) => {
      if (x > 0 && x + p.width > max + 1e-9) {
        y += rowH + gap;
        x = 0;
        rowH = 0;
      }
      const at = { x, y };
      x += p.width + gap;
      rowH = Math.max(rowH, p.height);
      width = Math.max(width, x - gap);
      return { key: p.key, name: p.name, x: at.x, y: at.y, width: p.width, height: p.height, cuts: shift(p.cuts, at.x, at.y), engraves: shift(p.engraves, at.x, at.y) };
    });
    return { parts: placed, warnings: g.warnings, width, height: y + rowH, error: null };
  } catch (e) {
    if (e instanceof Invalid) return { parts: [], warnings: [], width: 0, height: 0, error: { field: e.field, message: e.message } };
    throw e;
  }
}
