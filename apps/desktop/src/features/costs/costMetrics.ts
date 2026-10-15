import type { DesignObject, Document, JobPreview } from '@/api/types';
import { getModel } from '@/features/preview/model';
import { objectBoundsWithCopies, virtualCopies } from '@/lib/virtualArray';
import { objectPolylines, unionBounds } from '@/lib/geometry';
import type { JobFigures, TimeSource } from './costModel';
import type { JobLogEntry } from '@/features/help/jobLog';

/** Lo que Tlaku ya calcula del trabajo y que la calculadora aprovecha. */
export interface JobMetrics {
  /** Hay un diseño con salida (objetos visibles en capas activas). */
  hasDesign: boolean;
  /** Hay una vista previa al día con tiempos. */
  hasPreview: boolean;
  /** Estimado de la vista previa, en s. */
  totalS: number;
  /** Segundos con el láser encendido (corte y grabado) dentro de ese estimado. */
  laserOnS: number;
  /** Longitud de corte, en mm. */
  cutMm: number;
  /** Área de la caja envolvente del diseño, en mm². */
  bboxMm2: number;
  /** Área real de las formas cerradas, en mm². */
  realMm2: number;
  /** Objetos del diseño que salen (contando cada grupo como uno y las copias virtuales). */
  pieces: number;
  /** Los tiempos son una aproximación por distancia, no los del motor. */
  approximate: boolean;
}

export const EMPTY_METRICS: JobMetrics = {
  hasDesign: false,
  hasPreview: false,
  totalS: 0,
  laserOnS: 0,
  cutMm: 0,
  bboxMm2: 0,
  realMm2: 0,
  pieces: 0,
  approximate: false,
};

type Pt = [number, number];

/** Área con signo del polígono (fórmula de la lazada). */
function signedArea(pts: readonly Pt[]): number {
  let a = 0;
  for (let i = 0; i < pts.length; i++) {
    const [x1, y1] = pts[i]!;
    const [x2, y2] = pts[(i + 1) % pts.length]!;
    a += x1 * y2 - x2 * y1;
  }
  return a / 2;
}

function inside(p: Pt, poly: readonly Pt[]): boolean {
  let c = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [xi, yi] = poly[i]!;
    const [xj, yj] = poly[j]!;
    if (yi > p[1] !== yj > p[1] && p[0] < ((xj - xi) * (p[1] - yi)) / (yj - yi) + xi) c = !c;
  }
  return c;
}

/**
 * Área de un conjunto de contornos cerrados con la regla par-impar: un contorno dentro de otro es
 * un hueco (la «o» de un texto, un agujero de una pieza). Contornos de objetos distintos que se
 * traslapan se cuentan por separado.
 */
export function evenOddArea(rings: readonly (readonly Pt[])[]): number {
  let total = 0;
  rings.forEach((ring, i) => {
    if (ring.length < 3) return;
    let depth = 0;
    rings.forEach((other, j) => {
      if (i !== j && other.length >= 3 && inside(ring[0]!, other)) depth++;
    });
    total += (depth % 2 === 0 ? 1 : -1) * Math.abs(signedArea(ring));
  });
  return Math.max(0, total);
}

function objectArea(o: DesignObject): number {
  const rings = objectPolylines(o)
    .filter((p) => p.closed && p.points.length >= 3)
    .map((p) => p.points as Pt[]);
  return evenOddArea(rings) * (1 + virtualCopies(o).length);
}

/** Objetos que salen en el trabajo: visibles, en capas con salida y, si se pide, solo la selección. */
export function outputObjects(doc: Pick<Document, 'objects' | 'layers'>, selection: readonly string[] | null): DesignObject[] {
  const outputs = new Set(doc.layers.filter((l) => l.output).map((l) => l.color));
  const only = selection && selection.length > 0 ? new Set(selection) : null;
  return doc.objects.filter((o) => !o.hidden && outputs.has(o.layer) && (!only || only.has(o.id)));
}

/**
 * Tiempo, área y longitud de corte del trabajo. `preview` es el que ya calcula la app (se vuelve a
 * pedir al cambiar el diseño); `selection` son los ids del «solo la selección» o `null`.
 */
export function jobMetrics(doc: Pick<Document, 'objects' | 'layers'>, preview: JobPreview | null, selection: readonly string[] | null): JobMetrics {
  const objs = outputObjects(doc, selection);
  const box = unionBounds(objs.map(objectBoundsWithCopies));
  const bboxMm2 = box ? Math.max(0, (box[1][0] - box[0][0]) * (box[1][1] - box[0][1])) : 0;
  const realMm2 = objs.reduce((s, o) => s + objectArea(o), 0);
  const units = new Set<string>();
  let copies = 0;
  for (const o of objs) {
    const key = o.group ?? o.id;
    if (!units.has(key)) {
      units.add(key);
      copies += virtualCopies(o).length;
    }
  }
  const base = { hasDesign: objs.length > 0, bboxMm2, realMm2, pieces: units.size + copies };
  if (!preview || preview.moves.length === 0) return { ...EMPTY_METRICS, ...base };
  const model = getModel(preview);
  const d = model.distances;
  return {
    ...base,
    hasPreview: true,
    totalS: preview.total_s,
    laserOnS: d.cut.seconds + d.engrave.seconds,
    cutMm: d.cut.mm,
    approximate: model.approximate,
  };
}

/** Trabajos terminados de una máquina, del más nuevo al más viejo. */
export function finishedJobs(log: readonly JobLogEntry[], machine: string): JobLogEntry[] {
  return log.filter((e) => e.result === 'finished' && e.machine === machine && e.actualS > 0);
}

/** Mínimo de trabajos con estimado para fiarse del factor de calibración. */
export const CALIBRATION_MIN_JOBS = 3;

/**
 * Factor real ÷ estimado de la máquina: la mediana de `actualS / estimatedS` de sus últimos trabajos
 * terminados (la mediana resiste a un trabajo raro). `null` si hay menos de `CALIBRATION_MIN_JOBS`.
 */
export function calibrationFactor(log: readonly JobLogEntry[], machine: string): { factor: number; jobs: number } | null {
  const ratios = finishedJobs(log, machine)
    .filter((e) => e.estimatedS !== null && e.estimatedS > 0)
    .slice(0, 20)
    .map((e) => e.actualS / e.estimatedS!)
    .sort((a, b) => a - b);
  if (ratios.length < CALIBRATION_MIN_JOBS) return null;
  const mid = Math.floor(ratios.length / 2);
  const factor = ratios.length % 2 ? ratios[mid]! : (ratios[mid - 1]! + ratios[mid]!) / 2;
  return { factor, jobs: ratios.length };
}

/** Último trabajo terminado de la máquina (segundos reales), o `null`. */
export function lastRealSeconds(log: readonly JobLogEntry[], machine: string): number | null {
  return finishedJobs(log, machine)[0]?.actualS ?? null;
}

/**
 * Tiempo de una pasada según la fuente. Si la fuente pedida no se puede (sin historial), cae en el
 * estimado y devuelve cuál usó en realidad.
 */
export function resolveTime(
  source: TimeSource,
  m: Pick<JobMetrics, 'totalS' | 'laserOnS'>,
  log: readonly JobLogEntry[],
  machine: string,
): { runS: number; laserOnS: number; used: TimeSource; factor: number | null } {
  if (source === 'calibrated') {
    const cal = calibrationFactor(log, machine);
    if (cal) return { runS: m.totalS * cal.factor, laserOnS: m.laserOnS * cal.factor, used: 'calibrated', factor: cal.factor };
  }
  if (source === 'lastReal') {
    const real = lastRealSeconds(log, machine);
    if (real !== null) {
      // El tiempo con láser se escala igual que el total: la proporción encendido/total es la del estimado.
      const ratio = m.totalS > 0 ? Math.min(1, m.laserOnS / m.totalS) : 0;
      return { runS: real, laserOnS: real * ratio, used: 'lastReal', factor: null };
    }
  }
  return { runS: m.totalS, laserOnS: m.laserOnS, used: 'estimate', factor: null };
}

/** Datos del trabajo en la forma que pide `computeCost`. */
export function toFigures(m: JobMetrics, time: { runS: number; laserOnS: number }): JobFigures {
  return { runS: time.runS, laserOnS: time.laserOnS, bboxMm2: m.bboxMm2, realMm2: m.realMm2 };
}
