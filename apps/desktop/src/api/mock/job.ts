import { objectPolylines, pointsBounds } from '@/lib/geometry';
import { normalizeLayer, usesZ } from '@/lib/layers';
import type { DesignObject, DeviceProfile, Document, JobLayerSummary, JobOptions, JobPreview, LayerSettings, Point, TabSettings } from '../types';

type Move = [kind: 0 | 1, x: number, y: number, power: number];

/** Abscisas donde el segmento horizontal y = `y` cruza las polilíneas (par-impar). */
function scanCrossings(polys: { points: Point[] }[], y: number): number[] {
  const xs: number[] = [];
  for (const { points } of polys) {
    for (let i = 0; i < points.length; i++) {
      const a = points[i]!;
      const b = points[(i + 1) % points.length]!;
      if ((a[1] <= y && b[1] > y) || (b[1] <= y && a[1] > y)) {
        xs.push(a[0] + ((y - a[1]) / (b[1] - a[1])) * (b[0] - a[0]));
      }
    }
  }
  return xs.sort((p, q) => p - q);
}

function fillMoves(objs: DesignObject[], interval: number, power: number, overscan: number): Move[] {
  const polys = objs.flatMap((o) => objectPolylines(o)).filter((p) => p.closed);
  const bounds = pointsBounds(polys.flatMap((p) => p.points));
  if (!bounds) return [];
  const moves: Move[] = [];
  let dir = 1;
  for (let y = bounds[0][1] + interval / 2; y < bounds[1][1]; y += interval) {
    const xs = scanCrossings(polys, y);
    const spans: [number, number][] = [];
    for (let i = 0; i + 1 < xs.length; i += 2) spans.push([xs[i]!, xs[i + 1]!]);
    if (dir < 0) spans.reverse();
    for (const [x0, x1] of spans) {
      const [from, to] = dir > 0 ? [x0, x1] : [x1, x0];
      moves.push([0, from - dir * overscan, y, 0]);
      moves.push([1, from, y, 0]); // overscan: cut con potencia 0
      moves.push([1, to, y, power]);
      moves.push([1, to + dir * overscan, y, 0]);
    }
    if (spans.length) dir = -dir;
  }
  return moves;
}

/** Contornos concéntricos: copias de cada forma cerrada reducidas hacia el centro de su caja. */
function offsetMoves(objs: DesignObject[], interval: number, power: number): Move[] {
  const moves: Move[] = [];
  for (const { points } of objs.flatMap((o) => objectPolylines(o)).filter((p) => p.closed)) {
    const b = pointsBounds(points);
    if (!b) continue;
    const c: Point = [(b[0][0] + b[1][0]) / 2, (b[0][1] + b[1][1]) / 2];
    const half = Math.min(b[1][0] - b[0][0], b[1][1] - b[0][1]) / 2;
    for (let ring = 0; ring * interval < half; ring++) {
      const k = 1 - (ring * interval) / half;
      const pts = points.map((p): Point => [c[0] + (p[0] - c[0]) * k, c[1] + (p[1] - c[1]) * k]);
      moves.push([0, pts[0]![0], pts[0]![1], 0]);
      for (const p of pts.slice(1)) moves.push([1, p[0], p[1], power]);
      moves.push([1, pts[0]![0], pts[0]![1], power]);
    }
  }
  return moves;
}

/** Posiciones (mm desde el inicio del contorno) donde empieza cada puente. */
function tabStarts(length: number, tabs: TabSettings): number[] {
  const n = tabs.spacing > 0 ? Math.max(tabs.count, Math.floor(length / tabs.spacing)) : tabs.count;
  return Array.from({ length: n }, (_, i) => ((i + 0.5) * length) / n - tabs.width / 2);
}

/** Recorre una polilínea y la parte en tramos de corte y huecos (movimientos sin potencia). */
function polylineWithTabs(points: Point[], closed: boolean, power: number, tabs: TabSettings): Move[] {
  const path = closed ? [...points, points[0]!] : points;
  const total = path.slice(1).reduce((acc, p, i) => acc + Math.hypot(p[0] - path[i]![0], p[1] - path[i]![1]), 0);
  const gaps = tabStarts(total, tabs);
  const inGap = (d: number) => gaps.some((g) => d >= g && d < g + tabs.width);
  const moves: Move[] = [[0, path[0]![0], path[0]![1], 0]];
  let walked = 0;
  for (let i = 1; i < path.length; i++) {
    const a = path[i - 1]!;
    const b = path[i]!;
    const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
    // Se muestrea cada medio milímetro: basta para dibujar los huecos en la simulación.
    const steps = Math.max(1, Math.ceil(len / 0.5));
    for (let s = 1; s <= steps; s++) {
      const u = s / steps;
      const x = a[0] + (b[0] - a[0]) * u;
      const y = a[1] + (b[1] - a[1]) * u;
      const gap = inGap(walked + len * u);
      moves.push([gap ? 0 : 1, x, y, gap ? 0 : power]);
    }
    walked += len;
  }
  return moves;
}

function lineMoves(objs: DesignObject[], power: number, tabs?: TabSettings): Move[] {
  const moves: Move[] = [];
  for (const obj of objs) {
    const polys = objectPolylines(obj);
    for (const { points, closed } of polys) {
      if (points.length < 2) continue;
      // Con `skip_inner`, las formas dentro de otra (agujeros) no llevan puentes.
      const inner = polys.some((o) => o.closed && o.points !== points && pointInPoly(points[0]!, o.points));
      if (tabs?.enabled && tabs.width > 0 && !(tabs.skip_inner && inner)) {
        moves.push(...polylineWithTabs(points, closed, power, tabs));
        continue;
      }
      moves.push([0, points[0]![0], points[0]![1], 0]);
      for (const p of points.slice(1)) moves.push([1, p[0], p[1], power]);
      if (closed) moves.push([1, points[0]![0], points[0]![1], power]);
    }
  }
  return moves;
}

function pointInPoly(p: Point, poly: Point[]): boolean {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const a = poly[i]!;
    const b = poly[j]!;
    if (a[1] > p[1] !== b[1] > p[1] && p[0] < ((b[0] - a[0]) * (p[1] - a[1])) / (b[1] - a[1]) + a[0]) inside = !inside;
  }
  return inside;
}

function moveLength(moves: Move[], kind: 0 | 1, from: Point): number {
  let total = 0;
  let prev = from;
  for (const m of moves) {
    if (m[0] === kind) total += Math.hypot(m[1] - prev[0], m[2] - prev[1]);
    prev = [m[1], m[2]];
  }
  return total;
}

export function fmtDuration(s: number): string {
  const m = Math.floor(s / 60);
  const sec = Math.round(s % 60);
  return `${m} min ${String(sec).padStart(2, '0')} s`;
}

/**
 * Simula `compute_job`: movimientos plausibles a partir del documento.
 * No es el planificador real: solo sirve para diseñar la interfaz.
 */
export function mockComputeJob(doc: Document, device: DeviceProfile, options: JobOptions): JobPreview {
  const selection = options.selection ? new Set(options.selection) : null;
  const layers = doc.layers.filter((l) => l.output);
  // Automático: lo que se forzó con `order`, y luego grabar antes de cortar (la pieza se suelta al final).
  const rank = (l: Document['layers'][number]) => l.order ?? (l.mode === 'line' ? 200 : 100);
  const ordered = options.layer_order === 'file' ? layers : [...layers].sort((a, b) => rank(a) - rank(b));

  const flat: number[] = [];
  const moveTimes: number[] = [];
  const summaries: JobLayerSummary[] = [];
  const warnings: string[] = [];
  let total = 0;
  let cursor: Point = [0, 0];

  for (const raw of ordered) {
    // Un documento guardado antes de la Fase 4 llega sin los campos nuevos.
    const layer: LayerSettings = normalizeLayer(raw);
    if (usesZ(layer) && !device.z_axis) {
      warnings.push(`La capa '${layer.name}' tiene ajustes de Z, pero la máquina no tiene eje Z: se ignoran.`);
    }
    const candidates = doc.objects.filter(
      (o) => o.layer === layer.color && !o.hidden && (!selection || selection.has(o.id)),
    );
    // Igual que el motor: un texto sin contorno generado no se corta.
    const objs = candidates.filter((o) => o.kind.type !== 'text' || o.kind.outline.length > 0);
    for (const o of candidates) if (!objs.includes(o)) warnings.push(`Objeto '${o.id}' omitido: el texto no tiene contorno generado.`);
    if (objs.length === 0) continue;

    const power = layer.power;
    let moves: Move[];
    let overscanMm = 0;
    if (layer.mode === 'fill') {
      overscanMm = layer.overscan === 'auto' ? Math.min(6, (layer.speed / 60) ** 2 / (2 * device.accel[0])) : layer.overscan;
      moves = layer.fill_style === 'offset' ? offsetMoves(objs, Math.max(layer.interval, 0.05), power) : fillMoves(objs, Math.max(layer.interval, 0.05), power, overscanMm);
    } else if (layer.mode === 'image') {
      // Una imagen se barre completa, fila por fila, con potencia variable.
      moves = objs.flatMap((o) => {
        if (o.kind.type !== 'image') return [];
        const [w, h] = o.kind.size_mm;
        const x0 = o.transform[4];
        const y0 = o.transform[5];
        const out: Move[] = [];
        let dir = 1;
        for (let y = y0 + layer.interval / 2; y < y0 + h; y += layer.interval) {
          const [a, b] = dir > 0 ? [x0, x0 + w] : [x0 + w, x0];
          out.push([0, a, y, 0]);
          out.push([1, b, y, 20 + 35 * Math.abs(Math.sin(y / 5))]);
          dir = -dir;
        }
        return out;
      });
    } else {
      moves = lineMoves(objs, power, layer.tabs);
    }
    if (moves.length === 0) continue;

    // Tiempo acumulado al final de cada movimiento (sin aceleración: distancia / velocidad).
    const cutMm = moveLength(moves, 1, cursor);
    let seconds = 0;
    for (let p = 0; p < layer.passes; p++) {
      for (const m of moves) {
        const dist = Math.hypot(m[1] - cursor[0], m[2] - cursor[1]);
        seconds += (dist / (m[0] === 1 ? layer.speed : device.rapid_rate)) * 60;
        cursor = [m[1], m[2]];
        flat.push(m[0], m[1], m[2], m[3]);
        moveTimes.push(total + seconds);
      }
    }
    total += seconds;

    const explanations: string[] = [];
    if (layer.mode === 'line') {
      const holes = objs.flatMap((o) => objectPolylines(o)).filter((p) => p.closed).length;
      if (holes > 1) explanations.push(`Primero se cortan las ${holes - 1} figuras interiores y al final el contorno, para que la pieza no se mueva.`);
      if (layer.tabs.enabled) explanations.push(`Se dejan puentes de ${layer.tabs.width} mm sin cortar.`);
      explanations.push('Dentro de cada grupo se elige el vecino más cercano para acortar los recorridos en vacío.');
    } else if (layer.mode === 'fill' && layer.fill_style === 'offset') {
      explanations.push(`Contornos concéntricos cada ${layer.interval} mm, de afuera hacia adentro.`);
    } else if (layer.mode === 'fill') {
      explanations.push(`Barrido horizontal cada ${layer.interval} mm, en zigzag; margen de aceleración de ${overscanMm.toFixed(1)} mm a cada lado.`);
    } else {
      explanations.push(`Barrido de la imagen fila por fila con tramado ${layer.image.mode}, a ${layer.angle}°.`);
    }

    summaries.push({
      color: layer.color,
      mode: layer.mode,
      seconds,
      summary: `${layer.name}: ${(cutMm / 10).toFixed(0)} cm a ${layer.speed} mm/min, ${layer.power} % de potencia, ${layer.passes} ${layer.passes === 1 ? 'pasada' : 'pasadas'} (${fmtDuration(seconds)})`,
      travel_saved_pct: layer.mode === 'line' ? 31 : null,
      explanations,
    });
  }

  const pts: Point[] = [];
  for (let i = 0; i < flat.length; i += 4) pts.push([flat[i + 1]!, flat[i + 2]!]);
  const b = pointsBounds(pts) ?? ([[0, 0], [0, 0]] as [Point, Point]);
  const bounds: [Point, Point] = [b[0], b[1]];
  if (bounds[0][0] < 0 || bounds[0][1] < 0 || bounds[1][0] > device.bed[0] || bounds[1][1] > device.bed[1]) {
    warnings.push('El trabajo, con el margen de overscan, se sale de la cama de la máquina.');
  }
  if (flat.length === 0) warnings.push('No hay nada que enviar: activa la salida de alguna capa con objetos.');

  return {
    layers: summaries,
    moves: flat,
    move_times: moveTimes,
    total_s: total,
    bounds,
    order_note: summaries.length > 1 ? 'Orden de capas automático: primero las que graban, al final la que corta.' : null,
    warnings,
  };
}
