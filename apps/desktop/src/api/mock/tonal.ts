import type { LaserApi } from '../api';
import type { DesignObject, LayerSettings } from '../types';
import { toneMix, toneDarkness } from '@/features/tonal/curve';
import type { Box, TonalPreview, TonalRequest } from '@/features/tonal/types';
import { mockComputeJob } from './job';
import { simulatedDot } from './images';

type Lag = <T>(value: () => T) => Promise<T>;

/** Margen de la hoja alrededor de lo grabado (mm), como `SHEET_MARGIN_MM` del motor. */
const SHEET_MARGIN_MM = 3;
const MIN_PX_MM = 0.005;
const MAX_SIDE_PX = 4000;
const MAX_CELLS = 16_000_000;

/** PNG de 1 x 1 píxel transparente: sin canvas (pruebas) no hay dónde pintar. */
const BLANK_PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';

interface Op {
  op: LayerSettings;
  moves: [0 | 1, number, number, number][];
  objects: DesignObject[];
}

/** Rejilla de dosis del modo simulado: es una versión reducida de `DoseMap` del motor (sin ancho de punto ni M3). */
class Grid {
  readonly w: number;
  readonly h: number;
  readonly dose: Float32Array;
  constructor(
    readonly region: Box,
    readonly px: number,
  ) {
    this.w = Math.max(1, Math.ceil((region[2] - region[0]) / px));
    this.h = Math.max(1, Math.ceil((region[3] - region[1]) / px));
    if (this.w * this.h > MAX_CELLS) throw new Error('too big');
    this.dose = new Float32Array(this.w * this.h);
  }

  /** Suma `energy` (fracción de potencia x s) a lo largo de un segmento, a pasos de medio píxel. */
  segment(ax: number, ay: number, bx: number, by: number, energyPerMm: number): void {
    const len = Math.hypot(bx - ax, by - ay);
    if (!(len > 0)) return;
    const n = Math.max(1, Math.ceil(len / (this.px / 2)));
    const each = (energyPerMm * len) / n / (this.px * this.px);
    for (let i = 0; i < n; i++) {
      const t = (i + 0.5) / n;
      const ix = Math.floor((ax + (bx - ax) * t - this.region[0]) / this.px);
      const iy = Math.floor((ay + (by - ay) * t - this.region[1]) / this.px);
      if (ix >= 0 && iy >= 0 && ix < this.w && iy < this.h) this.dose[iy * this.w + ix]! += each;
    }
  }

  /** Reparte la dosis en el ancho del punto del láser con un promedio de caja separable (si el punto cabe en un píxel, no hace nada). */
  spread(widthMm: number): void {
    const k = Math.round(widthMm / this.px);
    if (!(k >= 2)) return;
    const r = Math.floor(k / 2);
    const inv = 1 / (2 * r + 1);
    const pass = (n: number, stride: number, lines: number, lineStride: number) => {
      const out = new Float64Array(n);
      for (let l = 0; l < lines; l++) {
        const base = l * lineStride;
        let sum = 0;
        for (let i = 0; i < Math.min(r, n); i++) sum += this.dose[base + i * stride]!;
        for (let i = 0; i < n; i++) {
          if (i + r < n) sum += this.dose[base + (i + r) * stride]!;
          out[i] = sum * inv;
          if (i - r >= 0) sum -= this.dose[base + (i - r) * stride]!;
        }
        for (let i = 0; i < n; i++) this.dose[base + i * stride] = out[i]!;
      }
    };
    pass(this.w, 1, this.h, this.w);
    pass(this.h, this.w, this.w, 1);
  }

  /** Un cuadrado `size` x `size` mm con esa dosis; si es menor que un píxel, se promedia en él. */
  square(x: number, y: number, size: number, dose: number): void {
    const x0 = Math.max(0, Math.floor((x - this.region[0]) / this.px));
    const y0 = Math.max(0, Math.floor((y - this.region[1]) / this.px));
    const x1 = Math.min(this.w - 1, Math.floor((x + size - this.region[0]) / this.px));
    const y1 = Math.min(this.h - 1, Math.floor((y + size - this.region[1]) / this.px));
    const weight = size >= this.px ? 1 : (size * size) / (this.px * this.px);
    for (let iy = y0; iy <= y1; iy++) for (let ix = x0; ix <= x1; ix++) this.dose[iy * this.w + ix]! += dose * weight;
  }
}

/** Caja de lo que realmente graba (con potencia), más el margen de la hoja. */
function sheetOf(ops: Op[]): Box | null {
  let b: Box | null = null;
  const grow = (x: number, y: number) => {
    b = b ? [Math.min(b[0], x), Math.min(b[1], y), Math.max(b[2], x), Math.max(b[3], y)] : [x, y, x, y];
  };
  for (const { op, moves, objects } of ops) {
    if (op.mode === 'image') {
      for (const o of objects) {
        if (o.kind.type !== 'image') continue;
        const [w, h] = o.kind.size_mm;
        grow(o.transform[4], o.transform[5]);
        grow(o.transform[4] + w, o.transform[5] + h);
      }
      continue;
    }
    let prev: [number, number] | null = null;
    for (const m of moves) {
      if (m[0] === 1 && m[3] > 0) {
        if (prev) grow(prev[0], prev[1]);
        grow(m[1], m[2]);
      }
      prev = [m[1], m[2]];
    }
  }
  const r = b as Box | null;
  return r ? [r[0] - SHEET_MARGIN_MM, r[1] - SHEET_MARGIN_MM, r[2] + SHEET_MARGIN_MM, r[3] + SHEET_MARGIN_MM] : null;
}

function deposit(grid: Grid, ops: Op[]): void {
  for (const { op, moves, objects } of ops) {
    const v = op.speed / 60;
    if (!(v > 0)) continue;
    if (op.mode === 'image') {
      // La imagen del modo simulado es la misma que la de su vista previa de capa: puntos tramados.
      const step = Math.max(op.interval, 0.02);
      for (const o of objects) {
        if (o.kind.type !== 'image') continue;
        const [w, h] = o.kind.size_mm;
        const ox = o.transform[4];
        const oy = o.transform[5];
        const cols = Math.floor(w / step);
        const rows = Math.floor(h / step);
        const dotDose = (op.power / 100 / v / step) * op.passes;
        for (let r = 0; r < rows; r++) {
          for (let c = 0; c < cols; c++) {
            const x = ox + c * step;
            const y = oy + h - (r + 1) * step;
            if (x > grid.region[2] || x + step < grid.region[0] || y > grid.region[3] || y + step < grid.region[1]) continue;
            if (simulatedDot(op, (c + 0.5) / cols, (r + 0.5) / rows, c, r)) grid.square(x, y, step, dotDose);
          }
        }
      }
      continue;
    }
    let prev: [number, number] | null = null;
    for (const m of moves) {
      if (m[0] === 1 && m[3] > 0 && prev) grid.segment(prev[0], prev[1], m[1], m[2], ((m[3] / 100) / v) * op.passes);
      prev = [m[1], m[2]];
    }
  }
}

function toPng(rgba: Uint8ClampedArray, w: number, h: number): string {
  if (typeof document === 'undefined') return BLANK_PNG;
  const canvas = document.createElement('canvas');
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext('2d');
  if (!ctx) return BLANK_PNG;
  ctx.putImageData(new ImageData(rgba as Uint8ClampedArray<ArrayBuffer>, w, h), 0, 0);
  const url = canvas.toDataURL('image/png');
  const comma = url.indexOf(',');
  return comma > 0 ? url.slice(comma + 1) : BLANK_PNG;
}

/** Igual que `render_tonal` del motor, con el trabajo simulado del modo mock. */
export function mockTonalRender(ops: Op[], request: TonalRequest): TonalPreview {
  const sheet = sheetOf(ops);
  if (!sheet) throw new Error('No hay nada que grabar: el trabajo no tiene movimientos con láser encendido.');
  const r = request.region_mm;
  const region: Box = r ? [Math.max(r[0], sheet[0]), Math.max(r[1], sheet[1]), Math.min(r[2], sheet[2]), Math.min(r[3], sheet[3])] : sheet;
  if (!(region[2] > region[0] && region[3] > region[1])) throw new Error('La zona pedida para la vista por tonos queda fuera del material.');
  const side = Math.min(MAX_SIDE_PX, Math.max(64, request.max_px));
  const px = Math.max(MIN_PX_MM, Math.max(region[2] - region[0], region[3] - region[1]) / side);
  const grid = new Grid(region, px);
  deposit(grid, ops);
  grid.spread(request.spot_mm);
  const { w, h } = grid;
  const covered: Box = [region[0], region[1], region[0] + w * px, region[1] + h * px];
  const rgba = new Uint8ClampedArray(w * h * 4);
  let maxDose = 0;
  for (let row = 0; row < h; row++) {
    const iy = h - 1 - row;
    const y = covered[1] + (iy + 0.5) * px;
    for (let col = 0; col < w; col++) {
      const x = covered[0] + (col + 0.5) * px;
      const d = grid.dose[iy * w + col]!;
      if (d > maxDose) maxDose = d;
      const o = (row * w + col) * 4;
      const c = toneMix(request.curve, toneDarkness(request.curve, d));
      rgba[o] = c[0];
      rgba[o + 1] = c[1];
      rgba[o + 2] = c[2];
      rgba[o + 3] = x < sheet[0] || x > sheet[2] || y < sheet[1] || y > sheet[3] ? 0 : 255;
    }
  }
  return { png_base64: toPng(rgba, w, h), width_px: w, height_px: h, covered_mm: covered, sheet_mm: sheet, px_mm: px, max_dose: maxDose };
}

/** Vista «por tonos» del modo simulado: una versión reducida de la del motor sobre el trabajo simulado. */
export function mockTonal(lag: Lag): Pick<LaserApi, 'tonalPreview'> {
  return {
    tonalPreview: ({ doc, device, options, request }) =>
      lag(() => {
        const ops: Op[] = [];
        mockComputeJob(doc, device, options, (b) => ops.push(b));
        return mockTonalRender(ops, request);
      }),
  };
}
