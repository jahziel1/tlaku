import type { Affine, Point } from '@/api/types';
import type { FitChoice, FitMode, FitResult, FitWarning, MarkPair } from '@/features/printcut/types';
import { MIN_SEPARATION_MM, SCALE_WARN } from '@/features/printcut/types';

/**
 * Maqueta del ajuste de Print & Cut para el modo simulado (`npm run dev` sin Tauri y las pruebas).
 * La app real llama al comando `fit_registration`, cuyo cálculo es `tlaku_pipeline::fit_registration`
 * (Rust, la única fuente de verdad). Esta copia existe solo para que la maqueta responda igual;
 * `crates/tlaku-pipeline/tests/cross/registration.json` la comprueba con los mismos casos que al
 * motor (`registrationFit.test.ts` y `tests/cross.rs`). Si el motor cambia, hay que cambiarla.
 */
const dist = (a: Point, b: Point) => Math.hypot(a[0] - b[0], a[1] - b[1]);
const finite = (p: Point) => Number.isFinite(p[0]) && Number.isFinite(p[1]);
const apply = (t: Affine, p: Point): Point => [t[0] * p[0] + t[2] * p[1] + t[4], t[1] * p[0] + t[3] * p[1] + t[5]];

function centroid(points: Point[]): Point {
  const n = points.length;
  return [points.reduce((s, p) => s + p[0], 0) / n, points.reduce((s, p) => s + p[1], 0) / n];
}

/** El modo por omisión según cuántas marcas hay: 2 dan similitud, 3 o más afín. */
function autoMode(marks: number): FitMode {
  return marks >= 3 ? 'affine' : 'similarity';
}

/** ¿Los puntos están (casi) en línea recta? Compara el área que abarcan con su largo. */
function areCollinear(points: readonly Point[]): boolean {
  if (points.length < 3) return false;
  const c = centroid([...points]);
  let sxx = 0;
  let sxy = 0;
  let syy = 0;
  for (const [x, y] of points) {
    const dx = x - c[0];
    const dy = y - c[1];
    sxx += dx * dx;
    sxy += dx * dy;
    syy += dy * dy;
  }
  const det = sxx * syy - sxy * sxy;
  const energy = sxx + syy;
  return energy < 1e-12 || det <= 1e-9 * energy * energy;
}

/**
 * Transformación que lleva los puntos del diseño a los físicos. Rígida y similitud sirven con 2 o
 * más pares (Procrustes en 2D, mínimos cuadrados); la afín pide 3 o más no alineados.
 */
/** El ajuste con un modo ya resuelto (`fit_registration` del motor). */
export function fitWithMode(pairs: readonly MarkPair[], mode: FitMode): FitResult {
  const need = mode === 'affine' ? 3 : 2;
  if (pairs.length < need) return { ok: false, error: { kind: 'few', need, have: pairs.length } };
  if (pairs.some((p) => !finite(p.design) || !finite(p.physical))) return { ok: false, error: { kind: 'nan' } };
  for (let i = 0; i < pairs.length; i++) {
    for (let j = i + 1; j < pairs.length; j++) {
      const a = pairs[i]!;
      const b = pairs[j]!;
      if (dist(a.design, b.design) < MIN_SEPARATION_MM || dist(a.physical, b.physical) < MIN_SEPARATION_MM) {
        return { ok: false, error: { kind: 'close', a: i + 1, b: j + 1 } };
      }
    }
  }

  const design = pairs.map((p) => p.design);
  const physical = pairs.map((p) => p.physical);
  const cd = centroid(design);
  const cp = centroid(physical);
  const a = design.map((p): Point => [p[0] - cd[0], p[1] - cd[1]]);
  const b = physical.map((p): Point => [p[0] - cp[0], p[1] - cp[1]]);

  let dot = 0;
  let cross = 0;
  let energy = 0;
  a.forEach((p, i) => {
    const q = b[i]!;
    dot += p[0] * q[0] + p[1] * q[1];
    cross += p[0] * q[1] - p[1] * q[0];
    energy += p[0] * p[0] + p[1] * p[1];
  });
  const theta = Math.atan2(cross, dot);
  const simScale = Math.hypot(dot, cross) / energy;

  let m: [[number, number], [number, number]];
  if (mode === 'affine') {
    const saa = [[0, 0], [0, 0]];
    const sba = [[0, 0], [0, 0]];
    a.forEach((p, k) => {
      const q = b[k]!;
      for (let i = 0; i < 2; i++) {
        for (let j = 0; j < 2; j++) {
          saa[i]![j]! += p[i]! * p[j]!;
          sba[i]![j]! += q[i]! * p[j]!;
        }
      }
    });
    const det = saa[0]![0]! * saa[1]![1]! - saa[0]![1]! * saa[1]![0]!;
    if (Math.abs(det) <= 1e-9 * energy * energy) return { ok: false, error: { kind: 'collinear' } };
    const inv = [
      [saa[1]![1]! / det, -saa[0]![1]! / det],
      [-saa[1]![0]! / det, saa[0]![0]! / det],
    ];
    m = [[0, 0], [0, 0]];
    for (let i = 0; i < 2; i++) {
      for (let j = 0; j < 2; j++) m[i]![j] = sba[i]![0]! * inv[0]![j]! + sba[i]![1]! * inv[1]![j]!;
    }
  } else {
    const s = mode === 'rigid' ? 1 : simScale;
    const sin = Math.sin(theta);
    const cos = Math.cos(theta);
    m = [[s * cos, -s * sin], [s * sin, s * cos]];
  }
  const t: Affine = [
    m[0][0],
    m[1][0],
    m[0][1],
    m[1][1],
    cp[0] - (m[0][0] * cd[0] + m[0][1] * cd[1]),
    cp[1] - (m[1][0] * cd[0] + m[1][1] * cd[1]),
  ];

  const residuals = pairs.map((p) => dist(apply(t, p.design), p.physical));
  const residualRms = Math.sqrt(residuals.reduce((s, r) => s + r * r, 0) / residuals.length);
  const residualMax = residuals.reduce((mx, r) => Math.max(mx, r), 0);

  const det = t[0] * t[3] - t[1] * t[2];
  const scaleXY: [number, number] = [Math.hypot(t[0], t[1]), Math.hypot(t[2], t[3])];
  const [scale, rotation] = mode === 'affine' ? [Math.sqrt(Math.abs(det)), Math.atan2(t[1] - t[2], t[0] + t[3])] : [simScale, theta];

  const warnings: FitWarning[] = [];
  if (det < 0) warnings.push({ kind: 'mirrored' });
  if (Math.abs(scale - 1) > SCALE_WARN) warnings.push({ kind: 'scale', pct: scale * 100 });
  if (mode === 'affine' && det > 0 && Math.abs(scaleXY[0] - scaleXY[1]) / Math.max(scaleXY[0], scaleXY[1]) > SCALE_WARN) {
    warnings.push({ kind: 'anisotropy', xPct: scaleXY[0] * 100, yPct: scaleXY[1] * 100 });
  }

  return {
    ok: true,
    fit: { transform: t, mode, rotationDeg: (rotation * 180) / Math.PI, scale, scaleXY, residuals, residualRms, residualMax, warnings },
  };
}

/** El modo que se usa de verdad (igual que `FitChoice::resolve` del motor). */
export function resolveFitMode(choice: FitChoice, designPoints: readonly Point[]): FitMode {
  const wanted = choice === 'auto' ? autoMode(designPoints.length) : choice;
  if (wanted === 'affine' && (designPoints.length < 3 || areCollinear(designPoints))) return 'similarity';
  return wanted;
}

/** Lo que responde la maqueta a `fit_registration`. */
export function mockFitRegistration(pairs: readonly MarkPair[], choice: FitChoice): FitResult {
  return fitWithMode(pairs, resolveFitMode(choice, pairs.map((p) => p.design)));
}
