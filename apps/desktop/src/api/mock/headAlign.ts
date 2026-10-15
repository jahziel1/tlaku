import { applyH, invertH, type Mat3 } from '@/features/camera/geometry';
import type { LaserApi } from '../api';
import type { HeadAlignFit, HeadAlignIssue, HeadAlignPlan, HeadAlignReport, HeadFrameReport } from '../types';
import { pngSize } from './camera';

type Lag = <T>(value: () => T) => Promise<T>;
type Box = [number, number, number, number];

const MIN_GRID = 3;
const MAX_GRID = 6;
const MIN_SPAN = 40;
const round2 = (v: number) => Math.round(v * 100) / 100;

function gridOver(area: Box, n: number): HeadAlignPlan {
  const step = (lo: number, hi: number, i: number) => lo + ((hi - lo) * i) / (n - 1);
  const points: [number, number][] = [];
  for (let j = 0; j < n; j++) {
    const y = round2(step(area[1], area[3], j));
    for (let k = 0; k < n; k++) {
      const i = j % 2 === 0 ? k : n - 1 - k;
      points.push([round2(step(area[0], area[2], i)), y]);
    }
  }
  return { points, reference: [round2(area[0] + (area[2] - area[0]) / (2 * (n - 1))), round2(area[1])], area };
}

/** Mismo reparto que el motor (`plan_grid`): rejilla de lado a lado del área segura, en serpentina. */
export function mockHeadPlan(bed: [number, number], grid: number, margin: number, region?: Box | null): HeadAlignPlan {
  if (!Number.isInteger(grid) || grid < MIN_GRID || grid > MAX_GRID) throw new Error(`La rejilla debe tener entre ${MIN_GRID} y ${MAX_GRID} puntos por lado.`);
  let area: Box = [margin, margin, bed[0] - margin, bed[1] - margin];
  if (region) {
    area = [Math.max(area[0], Math.min(region[0], region[2])), Math.max(area[1], Math.min(region[1], region[3])), Math.min(area[2], Math.max(region[0], region[2])), Math.min(area[3], Math.max(region[1], region[3]))];
  }
  if (area[2] - area[0] < MIN_SPAN || area[3] - area[1] < MIN_SPAN) throw new Error(`El área segura mide menos de ${MIN_SPAN} mm de lado.`);
  return gridOver(area, grid);
}

/** Mismo criterio que `plan_refine`: la caja de lo visto crece 50 % y se recorta al área. */
export function mockHeadRefine(area: Box, seen: [number, number][], grid: number): HeadAlignPlan | null {
  if (seen.length === 0 || grid < MIN_GRID || grid > MAX_GRID) return null;
  const r: Box = [
    Math.min(...seen.map((p) => p[0])),
    Math.min(...seen.map((p) => p[1])),
    Math.max(...seen.map((p) => p[0])),
    Math.max(...seen.map((p) => p[1])),
  ];
  for (const k of [0, 1] as const) {
    const span = r[k + 2] - r[k];
    const pad = span < MIN_SPAN / 2 ? 60 : span * 0.5;
    r[k] -= pad;
    r[k + 2] += pad;
  }
  const c: Box = [Math.max(r[0], area[0]), Math.max(r[1], area[1]), Math.min(r[2], area[2]), Math.min(r[3], area[3])];
  if (c[2] - c[0] < MIN_SPAN || c[3] - c[1] < MIN_SPAN) return null;
  const full = (area[2] - area[0]) * (area[3] - area[1]);
  return (c[2] - c[0]) * (c[3] - c[1]) < 0.98 * full ? gridOver(c, grid) : null;
}

/** Ruido determinista y reproducible (entre -0.5 y 0.5). */
function wiggle(seed: number, k: number): number {
  const v = Math.sin(seed * 12.9898 + k * 78.233) * 43758.5453;
  return v - Math.floor(v) - 0.5;
}

/**
 * Métodos simulados de «alinear con el cabezal». No miran las fotos: el cabezal «se ve» siempre
 * en las posiciones de la zona central de la cama (como una cámara de tapa que no cubre las
 * esquinas) y el ajuste es una homografía cama -> imagen con ruido de unas décimas de mm.
 */
export function mockHeadAlign(lag: Lag): Pick<LaserApi, 'cameraHeadAlignPlan' | 'cameraHeadAlignRefine' | 'cameraHeadAlignSolve' | 'cameraHeadAlignShift'> {
  return {
    cameraHeadAlignPlan: ({ options }) => lag(() => mockHeadPlan(options.bed, options.grid, options.margin_mm, options.region)),
    cameraHeadAlignRefine: ({ area, seen, grid }) => lag(() => mockHeadRefine(area, seen, grid)),
    cameraHeadAlignSolve: ({ frames, config }) =>
      lag((): HeadAlignReport => {
        if (frames.length === 0) throw new Error('No hay fotos.');
        const [w, h] = pngSize(frames[0].png_base64) ?? [640, 480];
        const [bw, bh] = config.bed;
        // Zona que «ve» la cámara de la prueba: el 60 % central de la cama.
        const view: Box = [0.2 * bw, 0.2 * bh, 0.8 * bw, 0.8 * bh];
        const toImage: Mat3 = [
          [w / (view[2] - view[0]), 0, (-view[0] * w) / (view[2] - view[0])],
          [0, -h / (view[3] - view[1]), (view[3] * h) / (view[3] - view[1])],
          [0, 0, 1],
        ];
        const seed = frames.length * 31 + Math.round(frames[0].machine_mm[0]);
        const offset = config.offset_mm ?? [0, 0];
        const reports: HeadFrameReport[] = frames.map((f, i) => {
          const [x, y] = f.machine_mm;
          const inside = x >= view[0] && x <= view[2] && y >= view[1] && y <= view[3];
          if (!inside) return { machine_mm: f.machine_mm, px: null, failure: 'no_change', confidence: 0, error_mm: null, used: false };
          const [u, v] = applyH(toImage, [x, y]);
          return {
            machine_mm: f.machine_mm,
            px: [u + 0.5 + 0.4 * wiggle(seed, 2 * i), v + 0.5 + 0.4 * wiggle(seed, 2 * i + 1)],
            failure: null,
            confidence: 0.85 + 0.1 * Math.abs(wiggle(seed, 90 + i)),
            error_mm: 0.1 + 0.3 * Math.abs(wiggle(seed, 40 + i)),
            used: true,
          };
        });
        const seen = reports.filter((r) => r.px).map((r) => r.machine_mm);
        const issues: HeadAlignIssue[] = [];
        const failed = reports.length - seen.length;
        if (failed > 0) issues.push({ kind: 'not_detected', count: failed });
        if (seen.length < 5) {
          return { method: 'led', image_size: [w, h], frames: reports, seen_mm: seen, detected: seen.length, failed, fit: null, fit_error: `Quedan ${seen.length} puntos con el cabezal localizado y se necesitan al menos 5.`, alignment: null, lens: null, covered_mm: null, issues };
        }
        const covered: Box = [Math.min(...seen.map((p) => p[0])), Math.min(...seen.map((p) => p[1])), Math.max(...seen.map((p) => p[0])), Math.max(...seen.map((p) => p[1]))];
        const area = config.safe_area ?? [0, 0, bw, bh];
        if ((covered[2] - covered[0]) * (covered[3] - covered[1]) < 0.6 * (area[2] - area[0]) * (area[3] - area[1])) issues.push({ kind: 'partial_coverage', covered_mm: covered, area_mm: area });
        const toBed = invertH(toImage) ?? toImage;
        const used = reports.filter((r) => r.px);
        const bedPts = used.map((r): [number, number] => [r.machine_mm[0] + offset[0], r.machine_mm[1] + offset[1]]);
        const fit: HeadAlignFit = {
          to_bed: toBed,
          lens: null,
          points: used.map((r, i) => ({ machine_mm: r.machine_mm, image_px: r.px!, bed_mm: bedPts[i], error_mm: r.error_mm!, residual_mm: r.error_mm! * 0.6, used: true })),
          rms_mm: 0.15,
          max_mm: 0.3,
          loo_rms_mm: 0.25,
          loo_max_mm: 0.45,
          used: used.length,
          dropped: 0,
          covered_mm: covered,
          quality: 'good',
          offset_mm: [offset[0], offset[1]],
          parallax_scale: 1,
          nadir_mm: [0, 0],
          issues: [],
        };
        const corners = [[covered[0], covered[1]], [covered[2], covered[1]], [covered[2], covered[3]], [covered[0], covered[3]]] as [number, number][];
        const inv = invertH(toBed) ?? toImage;
        const alignment = {
          image_size: [w, h] as [number, number],
          image_px: corners.map((c) => applyH(inv, c)) as [[number, number], [number, number], [number, number], [number, number]],
          bed_mm: corners as [[number, number], [number, number], [number, number], [number, number]],
          to_bed: toBed,
          rms_mm: fit.rms_mm,
          method: 'head_sweep' as const,
          reference_height_mm: 0,
          samples: [{ height_mm: 0, image_px: used.map((r) => r.px!), bed_mm: bedPts, loo_rms_mm: fit.loo_rms_mm, loo_max_mm: fit.loo_max_mm }],
          height: null,
        };
        return { method: 'led', image_size: [w, h], frames: reports, seen_mm: seen, detected: seen.length, failed, fit, fit_error: null, alignment, lens: null, covered_mm: covered, issues: [...issues, ...fit.issues] };
      }),
    cameraHeadAlignShift: ({ alignment, ref_px, ref_bed }) =>
      lag(() => {
        const at = applyH(alignment.to_bed, ref_px);
        const d: [number, number] = [ref_bed[0] - at[0], ref_bed[1] - at[1]];
        if (!d.every(Number.isFinite)) throw new Error('La posición de referencia no es válida.');
        const t: Mat3 = [[1, 0, d[0]], [0, 1, d[1]], [0, 0, 1]];
        const m = alignment.to_bed;
        const to_bed = t.map((row) => [0, 1, 2].map((c) => row[0] * m[0][c] + row[1] * m[1][c] + row[2] * m[2][c])) as Mat3;
        return {
          alignment: {
            ...alignment,
            to_bed,
            samples: (alignment.samples ?? []).map((s) => ({ ...s, bed_mm: s.bed_mm.map((p) => [p[0] + d[0], p[1] + d[1]] as [number, number]) })),
          },
          shift_mm: d,
        };
      }),
  };
}
