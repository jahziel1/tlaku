import type { JobPreview } from '@/api';
import type { RemoteLayer, RemoteRoute, RemoteSnapshot } from '@/api/api';
import { baseName } from '@/features/history/paths';
import { jobFraction } from '@/features/live/progress';
import { getLiveService } from '@/features/live/service';
import { hasOutput, selectActiveDevice, selectJobPreview } from '@/store';
import { jobElapsedSeconds, selectFramed, selectHeadDoc } from '@/store/machine';
import type { AppState } from '@/store/types';

export { jobFraction };

/**
 * Lo que la ventana le cuenta al servidor remoto (`src-tauri/src/remote`) para que el celular lo
 * muestre. Todo sale del estado que los eventos `machine://*` ya mantienen en el store: aquí solo se
 * resume. Funciones puras: se prueban sin red ni motor.
 */

/** Capa que se está ejecutando cuando el trabajo va en `fraction` del tiempo estimado. */
export function layerAt(preview: JobPreview | null, fraction: number): RemoteLayer | null {
  const layers = preview?.layers ?? [];
  if (layers.length === 0) return null;
  const total = layers.reduce((a, l) => a + l.seconds, 0);
  let i = layers.length - 1;
  if (total > 0) {
    const t = Math.min(1, Math.max(0, fraction)) * total;
    let acc = 0;
    for (let k = 0; k < layers.length; k++) {
      acc += layers[k]!.seconds;
      if (t < acc) {
        i = k;
        break;
      }
    }
  }
  const l = layers[i]!;
  return { index: i + 1, count: layers.length, color: l.color, mode: l.mode };
}

/** Resumen del trabajo y de la máquina para el celular. */
export function buildSnapshot(s: AppState, now: number): RemoteSnapshot {
  const device = selectActiveDevice(s);
  const active = s.jobState === 'running' || s.jobState === 'paused';
  const fraction = jobFraction(s, now);
  const elapsed = jobElapsedSeconds(s, now);
  const jobPreview = selectJobPreview(s);
  const total = jobPreview?.total_s ?? null;
  let remaining: number | null = null;
  if (active) {
    if (total !== null && total > 0) remaining = Math.max(0, total * (1 - fraction));
    else if (fraction > 0.02) remaining = (elapsed * (1 - fraction)) / fraction;
  }
  const head = selectHeadDoc(s);
  return {
    // Con un trabajo en curso, el documento es el de la pestaña desde la que se lanzó.
    doc_name: (active && s.jobOrigin ? s.jobOrigin.path : s.filePath) ? baseName((active && s.jobOrigin ? s.jobOrigin.path : s.filePath)!) : '',
    device_name: device.name,
    bed: [device.bed[0], device.bed[1]],
    job_state: s.jobState,
    job_message: s.jobMessage,
    progress: fraction,
    elapsed_s: active || s.jobState === 'finished' ? elapsed : 0,
    remaining_s: remaining,
    total_s: total,
    layer: active ? layerAt(jobPreview, fraction) : null,
    head: head ? [head[0], head[1]] : null,
    has_output: hasOutput(s),
    framed: selectFramed(s),
    // Solo lectura: dónde está la máquina sobre la ruta de la vista en vivo (null si la PC no la sigue).
    live: getLiveService()?.report() ?? null,
  };
}

// ---------------------------------------------------------------------------
// Recorrido
// ---------------------------------------------------------------------------

/** Puntos máximos del recorrido que se manda al celular. */
export const ROUTE_MAX_POINTS = 6000;
/** Más movimientos que esto: no vale la pena resumirlos (rellenos de imágenes enormes). */
const ROUTE_MAX_MOVES = 2_000_000;

type Run = number[];

/** Trazados continuos (`[x, y, x, y, ...]`) de los movimientos con láser; los saltos los separan. */
function cutRuns(moves: ArrayLike<number>): Run[] {
  const runs: Run[] = [];
  let run: Run | null = null;
  let lastX = NaN;
  let lastY = NaN;
  for (let i = 0; i + 3 < moves.length; i += 4) {
    const kind = moves[i]!;
    const x = moves[i + 1]!;
    const y = moves[i + 2]!;
    const power = moves[i + 3]!;
    if (kind === 1 && power > 0) {
      if (!run) {
        run = [lastX, lastY];
        runs.push(run);
      }
      run.push(x, y);
    } else {
      run = null;
    }
    lastX = x;
    lastY = y;
  }
  return runs.filter((r) => r.length >= 4 && Number.isFinite(r[0]!) && Number.isFinite(r[1]!));
}

/** Quita los puntos que quedan a menos de `eps` del último conservado (siempre deja el último). */
function simplify(run: Run, eps: number): Run {
  const out: Run = [run[0]!, run[1]!];
  let lx = run[0]!;
  let ly = run[1]!;
  const n = run.length / 2;
  for (let i = 1; i < n; i++) {
    const x = run[2 * i]!;
    const y = run[2 * i + 1]!;
    if (i === n - 1 || Math.hypot(x - lx, y - ly) >= eps) {
      out.push(x, y);
      lx = x;
      ly = y;
    }
  }
  return out;
}

const fmt = (v: number) => String(Math.round(v * 10) / 10);

/**
 * Trazado SVG del recorrido con láser, resumido para el celular: sin los saltos, con los puntos muy
 * juntos fundidos y, si aun así son demasiados, solo una de cada tantas pasadas (rellenos). `null`
 * si el trabajo no tiene nada que dibujar o es desmesurado.
 */
export function buildRoute(preview: JobPreview | null, bed: readonly [number, number]): RemoteRoute | null {
  if (!preview || preview.moves.length === 0 || preview.moves.length / 4 > ROUTE_MAX_MOVES) return null;
  let runs = cutRuns(preview.moves);
  if (runs.length === 0) return null;
  const eps = Math.hypot(bed[0], bed[1]) / 900;
  runs = runs.map((r) => simplify(r, eps));
  const points = runs.reduce((a, r) => a + r.length / 2, 0);
  if (points > ROUTE_MAX_POINTS) {
    const stride = Math.ceil(points / ROUTE_MAX_POINTS);
    runs = runs.filter((_, i) => i % stride === 0);
  }
  const d = runs.map((r) => 'M' + fmt(r[0]!) + ' ' + fmt(r[1]!) + r.slice(2).reduce((acc, v, k) => acc + (k % 2 === 0 ? 'L' : ' ') + fmt(v), '')).join('');
  return { bed: [bed[0], bed[1]], d };
}
