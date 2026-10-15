import type { RemoteLiveRoute, RemoteLiveRun } from '@/api/api';
import { MoveClass } from '@/features/preview/model';
import { isLaserMove, layerOfMove, type LiveRoute } from './route';

/**
 * Ruta simplificada de la vista en vivo para el celular. Se manda UNA vez por trabajo y después
 * solo viajan posiciones. Decisiones:
 *
 *  - Se mandan los TRAZOS CONTINUOS con láser (cortes y grabados), cada vértice con su tiempo de
 *    trabajo. Los desplazamientos no van: la página mueve la cabeza en línea recta de un trazo al
 *    siguiente usando los tiempos, así el cabezal se mueve fluido sin cargar la red.
 *  - Cada trazo se simplifica (Douglas-Peucker) con una tolerancia en mm: 0,1 mm o 1/4000 de la
 *    diagonal de la cama, lo que sea mayor; eso es invisible en una pantalla de celular. Los
 *    tiempos se conservan en los vértices que quedan.
 *  - Tope de tamaño (`MAX_BYTES`, ~300 KB de JSON). Si aun simplificado no cabe, primero se sube la
 *    tolerancia y luego, para rellenos e imágenes (decenas de miles de pasadas casi paralelas),
 *    se mandan «líneas agrupadas»: una de cada `s` pasadas, dibujada `s` veces más ancha con el paso
 *    entre líneas estimado de los datos. El área cubierta se ve igual y el tamaño cae a 1/s. Es
 *    más simple, exacto y barato de dibujar que un raster de baja resolución, que además habría
 *    que rehacer en cada trabajo con la paleta de las capas. Los cortes nunca se agrupan salvo
 *    como último recurso.
 *  - Rutas de más de `MAX_MOVES` movimientos no se resumen: el celular solo muestra el cabezal.
 */

export const MAX_BYTES = 300_000;
/** Más movimientos que esto: no vale la pena resumirlos. */
export const MAX_MOVES = 3_000_000;
/** Tolerancia mínima de la simplificación (mm). */
export const MIN_EPS_MM = 0.1;
/** Ancho de los cortes en el celular (mm). */
export const CUT_WIDTH_MM = 0.35;
const MIN_ENGRAVE_WIDTH_MM = 0.12;
const MAX_ENGRAVE_WIDTH_MM = 2;
/** Bytes que ocupa un vértice `[x,y,t]` en el JSON (≈ `123.45,123.45,1234.56,`). */
const BYTES_PER_VERTEX = 22;
const BYTES_PER_RUN = 28;

interface Run {
  k: 0 | 1;
  layer: number;
  /** Vértices `[x, y, t, ...]`. */
  p: number[];
}

/** Trazos continuos con láser: se parte donde hay un desplazamiento, un overscan o un cambio de capa o de clase. */
export function extractRuns(route: LiveRoute): Run[] {
  const { pos, times, cls } = route.model;
  const runs: Run[] = [];
  let cur: Run | null = null;
  let curLayer = -2;
  let curCls = -1;
  for (let k = 0; k < route.count; k++) {
    if (!isLaserMove(route, k)) {
      cur = null;
      continue;
    }
    const layer = layerOfMove(route, k);
    const c = cls[k]!;
    if (!cur || layer !== curLayer || c !== curCls) {
      cur = { k: c === MoveClass.Cut ? 0 : 1, layer, p: [pos[2 * k]!, pos[2 * k + 1]!, k > 0 ? times[k - 1]! : 0] };
      curLayer = layer;
      curCls = c;
      runs.push(cur);
    }
    cur.p.push(pos[2 * k + 2]!, pos[2 * k + 3]!, times[k]!);
  }
  return runs.filter((r) => r.p.length >= 6);
}

/** Douglas-Peucker iterativo sobre `[x, y, t, ...]`; conserva el primero y el último. */
export function simplifyRun(p: number[], eps: number): number[] {
  const n = p.length / 3;
  if (n <= 2) return p.slice();
  const keep = new Uint8Array(n);
  keep[0] = 1;
  keep[n - 1] = 1;
  const stack: [number, number][] = [[0, n - 1]];
  while (stack.length) {
    const [a, b] = stack.pop()!;
    const ax = p[3 * a]!;
    const ay = p[3 * a + 1]!;
    const dx = p[3 * b]! - ax;
    const dy = p[3 * b + 1]! - ay;
    const len = Math.hypot(dx, dy);
    let far = -1;
    let farD = eps;
    for (let i = a + 1; i < b; i++) {
      const px = p[3 * i]! - ax;
      const py = p[3 * i + 1]! - ay;
      const d = len > 1e-12 ? Math.abs(px * dy - py * dx) / len : Math.hypot(px, py);
      if (d > farD) {
        farD = d;
        far = i;
      }
    }
    if (far >= 0) {
      keep[far] = 1;
      stack.push([a, far], [far, b]);
    }
  }
  const out: number[] = [];
  for (let i = 0; i < n; i++) if (keep[i]) out.push(p[3 * i]!, p[3 * i + 1]!, p[3 * i + 2]!);
  return out;
}

/** Paso entre líneas (mm): mediana de la distancia perpendicular entre trazos de grabado consecutivos. */
export function estimatePitch(runs: Run[]): number {
  const engrave = runs.filter((r) => r.k === 1);
  const samples: number[] = [];
  const step = Math.max(1, Math.floor(engrave.length / 400));
  for (let i = 0; i + 1 < engrave.length && samples.length < 400; i += step) {
    const a = engrave[i]!.p;
    const b = engrave[i + 1]!.p;
    const dx = a[a.length - 3]! - a[0]!;
    const dy = a[a.length - 2]! - a[1]!;
    const len = Math.hypot(dx, dy);
    if (len < 1e-6) continue;
    const mx = (b[0]! + b[b.length - 3]!) / 2 - (a[0]! + a[a.length - 3]!) / 2;
    const my = (b[1]! + b[b.length - 2]!) / 2 - (a[1]! + a[a.length - 2]!) / 2;
    const perp = Math.abs(mx * dy - my * dx) / len;
    if (perp > 1e-4) samples.push(perp);
  }
  if (samples.length === 0) return MIN_ENGRAVE_WIDTH_MM;
  samples.sort((x, y) => x - y);
  return samples[samples.length >> 1]!;
}

const r2 = (v: number) => Math.round(v * 100) / 100;

function estimateBytes(runs: { p: number[] }[]): number {
  let b = 120;
  for (const r of runs) b += BYTES_PER_RUN + (r.p.length / 3) * BYTES_PER_VERTEX;
  return b;
}

/**
 * Arma la ruta del celular. `null` si no hay nada con láser o la ruta es desmesurada. El resultado
 * siempre cabe en `maxBytes` (se verifica serializándolo).
 */
export function buildLivePayload(route: LiveRoute, bed: readonly [number, number], maxBytes: number = MAX_BYTES): RemoteLiveRoute | null {
  if (route.count > MAX_MOVES) return null;
  const raw = extractRuns(route);
  if (raw.length === 0) return null;
  const diag = Math.hypot(bed[0], bed[1]);
  const pitch = estimatePitch(raw);
  let eps = Math.max(MIN_EPS_MM, diag / 4000);
  let simplified = raw.map((r) => ({ ...r, p: simplifyRun(r.p, eps) }));
  let decimated = false;
  let stride = 1;

  // 1. Más tolerancia (hasta ~1/500 de la diagonal, aún imperceptible en un celular).
  const maxEps = Math.max(0.5, diag / 500);
  while (estimateBytes(simplified) > maxBytes && eps < maxEps) {
    eps *= 2;
    simplified = raw.map((r) => ({ ...r, p: simplifyRun(r.p, eps) }));
    decimated = true;
  }
  // 2. Líneas agrupadas: una de cada `stride` pasadas de grabado.
  const engraveBytes = estimateBytes(simplified.filter((r) => r.k === 1));
  const rest = estimateBytes(simplified.filter((r) => r.k === 0));
  let kept = simplified;
  if (estimateBytes(simplified) > maxBytes && engraveBytes > 0) {
    stride = Math.max(2, Math.ceil(engraveBytes / Math.max(maxBytes - rest, maxBytes * 0.25)));
    let seen = 0;
    kept = simplified.filter((r) => r.k === 0 || seen++ % stride === 0);
    decimated = true;
  }
  // 3. Último recurso: también se agrupan los cortes.
  let cutStride = 1;
  if (estimateBytes(kept) > maxBytes) {
    cutStride = Math.ceil(estimateBytes(kept) / maxBytes);
    let seen = 0;
    kept = kept.filter((r) => r.k === 1 || seen++ % cutStride === 0);
    decimated = true;
  }
  const engraveWidth = Math.min(MAX_ENGRAVE_WIDTH_MM, Math.max(MIN_ENGRAVE_WIDTH_MM, pitch * stride));
  const runs: RemoteLiveRun[] = kept.map((r) => ({
    k: r.k,
    l: Math.max(0, r.layer),
    w: r.k === 0 ? CUT_WIDTH_MM * cutStride : engraveWidth,
    p: r.p.map(r2),
  }));
  const layers = route.layerColors.length > 0 ? route.layerColors.map((c) => (/^#[0-9a-f]{6}$/i.test(c) ? c : '#808080')) : ['#808080'];
  let payload: RemoteLiveRoute = { bed: [bed[0], bed[1]], total: r2(route.total), layers, runs, decimated };
  // Verificación exacta; si el cálculo estimado se quedó corto se vuelve a agrupar.
  let guard = 0;
  while (JSON.stringify(payload).length > maxBytes && payload.runs.length > 1 && guard++ < 12) {
    payload = { ...payload, runs: payload.runs.filter((_, i) => i % 2 === 0).map((r) => ({ ...r, w: Math.min(MAX_ENGRAVE_WIDTH_MM, r.w * 2) })), decimated: true };
  }
  return JSON.stringify(payload).length > maxBytes ? null : payload;
}
