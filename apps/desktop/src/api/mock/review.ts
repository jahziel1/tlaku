import { objectPolylines, pointsBounds } from '@/lib/geometry';
import { isGuideColor } from '@/lib/palette';
import { objectBoundsWithCopies } from '@/lib/virtualArray';
import type { Finding, FindingKind, ReviewArgs, Severity } from '@/features/review/types';
import type { LaserApi } from '../api';
import type { DesignObject } from '../types';
import { mockGeometryOp } from './geometry';
import { mockShapeOp } from './shapes';

type Lag = <T>(value: () => T) => Promise<T>;

const BED_SLACK = 0.01;
const MIN_NODES_SAVED = 2;
const SEVERITY_ORDER: Record<Severity, number> = { serious: 0, warning: 1, info: 2 };

/** Clave estable: tipo, capa y objetos (en el motor es un hash; aquí basta el texto). */
function make(kind: FindingKind, severity: Severity, layer: string | null, ids: string[], extra: Partial<Finding> = {}): Finding {
  const object_ids = [...new Set(ids)].sort();
  return { key: `${kind}:${layer ?? '-'}:${object_ids.join(',')}`, kind, severity, layer, object_ids, count: object_ids.length, value: 0, detail: 0, tolerance: null, ...extra };
}

/** Resolución real de una imagen: `[mínima, máxima]` ppp de sus dos ejes. */
function imageDpi(o: DesignObject): [number, number] | null {
  if (o.kind.type !== 'image') return null;
  const [a, b, c, d] = o.transform;
  const w = o.kind.size_mm[0] * Math.hypot(a, b);
  const h = o.kind.size_mm[1] * Math.hypot(c, d);
  if (!(w > 1e-9 && h > 1e-9) || o.kind.width_px === 0 || o.kind.height_px === 0) return null;
  const dx = o.kind.width_px / (w / 25.4);
  const dy = o.kind.height_px / (h / 25.4);
  return [Math.min(dx, dy), Math.max(dx, dy)];
}

/** Largo total de los trazos (mm). */
function strokeLength(objs: readonly DesignObject[]): number {
  let total = 0;
  for (const o of objs) {
    for (const { points, closed } of objectPolylines(o)) {
      const n = closed ? points.length : points.length - 1;
      for (let i = 0; i < n; i++) total += Math.hypot(points[(i + 1) % points.length]![0] - points[i]![0], points[(i + 1) % points.length]![1] - points[i]![1]);
    }
  }
  return total;
}

/** El hueco más grande entre los extremos de los trazos abiertos (mm), como lo informa el motor. */
function worstGap(objs: readonly DesignObject[]): number {
  let worst = 0;
  for (const o of objs) {
    for (const { points, closed } of objectPolylines(o)) {
      if (closed || points.length < 2) continue;
      const a = points[0]!;
      const b = points[points.length - 1]!;
      worst = Math.max(worst, Math.hypot(b[0] - a[0], b[1] - a[1]));
    }
  }
  return worst;
}

/**
 * «Revisar diseño» simulado. Detecta lo mismo que el motor, apoyándose en los otros métodos
 * simulados (qué objetos cambiaría `cerrar`, `unir`, `quitar duplicados` u `optimizar`). Es una
 * aproximación para desarrollar la interfaz: los números exactos son los del motor.
 */
export function mockReviewDesign({ doc, bed, options = {} }: ReviewArgs): Finding[] {
  const closeMm = options.close_mm ?? 0.5;
  const joinMm = options.join_mm ?? 0.1;
  const dedupeMm = options.dedupe_mm ?? 0.01;
  const tinyMm = options.tiny_mm ?? 0.2;
  const nodeMm = options.node_mm ?? 0.02;
  const dpiHigh = options.dpi_high ?? 2;
  const dpiLow = options.dpi_low ?? 0.5;
  const out: Finding[] = [];
  const outside: [string[], string[]] = [[], []];
  const texts: string[] = [];
  let textsEmpty = 0;

  const colors = [...new Set(doc.objects.filter((o) => !o.hidden).map((o) => o.layer))];
  for (const color of colors) {
    if (isGuideColor(color)) continue;
    const members = doc.objects.filter((o) => !o.hidden && o.layer === color);
    const layer = doc.layers.find((l) => l.color.toLowerCase() === color.toLowerCase());
    if (layer && !layer.output) {
      out.push(make('silent_layer', 'serious', color, members.map((o) => o.id)));
      continue;
    }
    if (layer && !layer.visible) out.push(make('hidden_layer', 'warning', color, members.map((o) => o.id)));

    const wanted = layer && layer.mode === 'image' && layer.interval > 1e-9 ? 25.4 / layer.interval : null;
    const high: string[] = [];
    const low: string[] = [];
    let worstHigh = Infinity;
    let worstLow = 0;
    for (const o of members) {
      const b = bed[0] > 0 && bed[1] > 0 ? objectBoundsWithCopies(o) : null;
      if (b) {
        const [[x0, y0], [x1, y1]] = b;
        // Tocar el borde de la cama (una recta sobre y = 0) es estar dentro.
        if (x1 < -BED_SLACK || y1 < -BED_SLACK || x0 > bed[0] + BED_SLACK || y0 > bed[1] + BED_SLACK) outside[0].push(o.id);
        else if (x0 < -BED_SLACK || y0 < -BED_SLACK || x1 > bed[0] + BED_SLACK || y1 > bed[1] + BED_SLACK) outside[1].push(o.id);
      }
      if (o.kind.type === 'text') {
        texts.push(o.id);
        if (o.kind.outline.length === 0) textsEmpty++;
      }
      const dpi = wanted ? imageDpi(o) : null;
      if (wanted && dpi) {
        if (dpi[0] > wanted * dpiHigh) {
          high.push(o.id);
          worstHigh = Math.min(worstHigh, dpi[0]);
        } else if (dpi[1] < wanted * dpiLow) {
          low.push(o.id);
          worstLow = Math.max(worstLow, dpi[1]);
        }
      }
    }
    if (high.length) out.push(make('image_dpi_high', 'info', color, high, { value: worstHigh, detail: wanted ?? 0 }));
    if (low.length) out.push(make('image_dpi_low', 'info', color, low, { value: worstLow, detail: wanted ?? 0 }));
    if (layer?.mode === 'image') continue;

    const eligible = members.filter((o) => !o.locked && o.kind.type === 'vector');
    const size = (o: DesignObject): [number, number] | null => {
      const b = pointsBounds(objectPolylines(o).flatMap((p) => p.points));
      return b ? [b[1][0] - b[0][0], b[1][1] - b[0][1]] : null;
    };
    const tiny = eligible.filter((o) => {
      const s = size(o);
      return s !== null && s[0] < tinyMm && s[1] < tinyMm;
    });
    if (tiny.length) out.push(make('tiny', 'warning', color, tiny.map((o) => o.id), { value: tinyMm, tolerance: tinyMm }));
    const live = eligible.filter((o) => size(o) !== null && !tiny.includes(o));
    if (live.length === 0) continue;
    const liveIds = live.map((o) => o.id);
    const consumed = (op: 'close' | 'join' | 'dedupe', tolerance: number): string[] => {
      const result = new Set(mockGeometryOp({ op, objects: live, tolerance }).map((r) => r.id));
      return liveIds.filter((id) => !result.has(id));
    };

    const near = consumed('close', closeMm);
    if (near.length) out.push(make('near_closed', 'warning', color, near, { value: worstGap(live.filter((o) => near.includes(o.id))), tolerance: closeMm }));
    const dup = consumed('dedupe', dedupeMm);
    if (dup.length) {
      const removed = Math.max(0, strokeLength(live) - strokeLength(mockGeometryOp({ op: 'dedupe', objects: live, tolerance: dedupeMm })));
      out.push(make('overlap', 'warning', color, dup, { value: removed, fix_ids: liveIds, tolerance: dedupeMm }));
    }
    const joined = consumed('join', joinMm).filter((id) => !near.includes(id));
    if (joined.length >= 2) out.push(make('joinable', 'warning', color, joined, { tolerance: joinMm }));
    let saved = 0;
    const nodes = live.filter((o) => {
      const stats = mockShapeOp({ op: 'optimize', objects: [o], params: { tolerance: nodeMm } }).stats;
      const gain = stats.nodes_before - stats.nodes_after;
      if (gain >= MIN_NODES_SAVED) saved += gain;
      return gain >= MIN_NODES_SAVED;
    });
    if (nodes.length) out.push(make('extra_nodes', 'info', color, nodes.map((o) => o.id), { value: saved, tolerance: nodeMm }));
  }

  if (outside[0].length) out.push(make('outside_all', 'serious', null, outside[0]));
  if (outside[1].length) out.push(make('outside_part', 'serious', null, outside[1]));
  if (texts.length) out.push(make('text', textsEmpty > 0 ? 'warning' : 'info', null, texts, { detail: textsEmpty }));
  return out.sort((a, b) => SEVERITY_ORDER[a.severity] - SEVERITY_ORDER[b.severity]);
}

export function mockReview(lag: Lag): Pick<LaserApi, 'reviewDesign'> {
  return { reviewDesign: (args) => lag(() => mockReviewDesign(args)) };
}
