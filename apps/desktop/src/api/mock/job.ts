import { t } from '@/i18n';
import { applyAffine, objectPolylines, pointsBounds } from '@/lib/geometry';
import { normalizeLayer, usesZ } from '@/lib/layers';
import { clipMoves } from '@/features/tiling/clip';
import type { DesignObject, DeviceProfile, Document, JobLayerSummary, JobOptions, JobPreview, LayerSettings, Point, TabSettings } from '../types';

type Move = [kind: 0 | 1, x: number, y: number, power: number];

/** Cada operación del trabajo simulado (la vista «por tonos» del modo simulado la usa para estimar la dosis). */
export type BlockCollector = (block: { op: LayerSettings; moves: [0 | 1, number, number, number][]; objects: DesignObject[] }) => void;

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

/** Largo de una polilínea. */
function pathLength(path: Point[]): number {
  return path.slice(1).reduce((acc, p, i) => acc + Math.hypot(p[0] - path[i]![0], p[1] - path[i]![1]), 0);
}

/** Sigue el contorno cerrado `extra` mm más allá del punto de partida (sobrecorte). */
function withOvercut(path: Point[], extra: number): Point[] {
  const out = [...path];
  let left = extra;
  for (let i = 1; i < path.length && left > 0; i++) {
    const a = path[i - 1]!;
    const b = path[i]!;
    const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (len <= left) {
      out.push(b);
      left -= len;
    } else {
      const u = left / len;
      out.push([a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u]);
      left = 0;
    }
  }
  return out;
}

type Gap = (d: number) => boolean;

interface StrokeStyle {
  /** Huecos del trazo según la distancia recorrida; recibe el largo del contorno. */
  gaps: (total: number) => Gap;
  /** Rampa de potencia: largo en mm y fracción inicial (0-1). */
  ramp: { length: number; start: number } | null;
  overcut: number;
  /** Cada cuántos mm se muestrea el trazo. */
  step: number;
}

/**
 * Recorre una polilínea y la parte en tramos de corte y huecos (movimientos sin potencia), con
 * rampa de potencia y sobrecorte. Sirve para puentes, perforación y modo punto.
 */
function strokeMoves(points: Point[], closed: boolean, power: number, style: StrokeStyle): Move[] {
  let path = closed ? [...points, points[0]!] : points;
  const inGap = style.gaps(pathLength(path));
  if (closed && style.overcut > 0) path = withOvercut(path, style.overcut);
  const total = pathLength(path);
  const powerAt = (d: number) => {
    if (!style.ramp) return power;
    const edge = Math.min(d, total - d);
    const k = edge >= style.ramp.length ? 1 : style.ramp.start + (1 - style.ramp.start) * (edge / style.ramp.length);
    return power * k;
  };
  const moves: Move[] = [[0, path[0]![0], path[0]![1], 0]];
  let walked = 0;
  for (let i = 1; i < path.length; i++) {
    const a = path[i - 1]!;
    const b = path[i]!;
    const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
    const steps = Math.max(1, Math.ceil(len / style.step));
    for (let s = 1; s <= steps; s++) {
      const u = s / steps;
      const x = a[0] + (b[0] - a[0]) * u;
      const y = a[1] + (b[1] - a[1]) * u;
      const gap = inGap(walked + len * u);
      moves.push([gap ? 0 : 1, x, y, gap ? 0 : powerAt(walked + len * u)]);
    }
    walked += len;
  }
  return moves;
}

function lineMoves(objs: DesignObject[], layer: LayerSettings): Move[] {
  const power = layer.power;
  const tabs = layer.tabs.enabled && layer.tabs.width > 0 ? layer.tabs : null;
  const perf = layer.perforation.enabled && layer.perforation.cut_mm > 0 && layer.perforation.skip_mm > 0 ? layer.perforation : null;
  const dot = layer.dot_mode.enabled && layer.dot_mode.spacing_mm > 0 ? layer.dot_mode : null;
  // La rampa necesita un trazo continuo: con modo punto no se aplica.
  const ramp = layer.ramp.enabled && !dot && layer.ramp.length_mm > 0 ? layer.ramp : null;
  const periodGap =
    (period: number, on: number): Gap =>
    (d) =>
      d % period >= on;
  const moves: Move[] = [];
  for (const obj of objs) {
    const polys = objectPolylines(obj);
    for (const { points, closed } of polys) {
      if (points.length < 2) continue;
      // Con `skip_inner`, las formas dentro de otra (agujeros) no llevan puentes.
      const inner = polys.some((o) => o.closed && o.points !== points && pointInPoly(points[0]!, o.points));
      const withTabs = tabs !== null && !(tabs.skip_inner && inner);
      const withRamp = ramp !== null && !(ramp.outer_only && inner);
      const overcut = closed ? layer.overcut : 0;
      if (!withTabs && !perf && !dot && !withRamp && overcut <= 0) {
        moves.push([0, points[0]![0], points[0]![1], 0]);
        for (const p of points.slice(1)) moves.push([1, p[0], p[1], power]);
        if (closed) moves.push([1, points[0]![0], points[0]![1], power]);
        continue;
      }
      const style: StrokeStyle = {
        gaps: (total) => {
          const list: Gap[] = [];
          if (withTabs) {
            const starts = tabStarts(total, tabs);
            list.push((d) => starts.some((g) => d >= g && d < g + tabs.width));
          }
          if (perf) list.push(periodGap(perf.cut_mm + perf.skip_mm, perf.cut_mm));
          if (dot) list.push(periodGap(dot.spacing_mm, Math.min(0.15, dot.spacing_mm / 2)));
          return (d) => list.some((g) => g(d));
        },
        ramp: withRamp ? { length: ramp.length_mm, start: Math.min(1, Math.max(0, ramp.start_pct / 100)) } : null,
        overcut,
        // Cada medio milímetro basta para dibujar los huecos de los puentes; el patrón fino pide más resolución.
        step: Math.max(0.1, Math.min(0.5, perf ? Math.min(perf.cut_mm, perf.skip_mm) / 2 : 0.5, dot ? dot.spacing_mm / 4 : 0.5)),
      };
      moves.push(...strokeMoves(points, closed, power, style));
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
export function mockComputeJob(doc: Document, device: DeviceProfile, options: JobOptions, collect?: BlockCollector): JobPreview {
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

    // La capa y, después, cada operación adicional sobre los mismos objetos (cada una es una fila del resumen).
    const operations = [
      { op: layer, label: layer.name },
      ...layer.sub_layers.map((sub, i) => ({ op: { ...sub, color: layer.color, name: layer.name }, label: `${layer.name} (+${i + 1})` })),
    ];
    for (const { op, label } of operations) {
      const power = op.power;
      let moves: Move[];
      let overscanMm = 0;
      if (op.mode === 'fill') {
        overscanMm = op.overscan === 'auto' ? Math.min(6, (op.speed / 60) ** 2 / (2 * device.accel[0])) : op.overscan;
        const interval = Math.max(op.interval, 0.05);
        moves =
          op.fill_style === 'offset'
            ? offsetMoves(objs, interval, power)
            : op.fill_grouping === 'by_shape'
              ? objs.flatMap((o) => fillMoves([o], interval, power, overscanMm))
              : fillMoves(objs, interval, power, overscanMm);
      } else if (op.mode === 'image') {
        // Una imagen se barre completa, fila por fila, con potencia variable.
        moves = objs.flatMap((o) => {
          if (o.kind.type !== 'image') return [];
          const [w, h] = o.kind.size_mm;
          const x0 = o.transform[4];
          const y0 = o.transform[5];
          const out: Move[] = [];
          let dir = 1;
          for (let y = y0 + op.interval / 2; y < y0 + h; y += op.interval) {
            const [a, b] = dir > 0 ? [x0, x0 + w] : [x0 + w, x0];
            out.push([0, a, y, 0]);
            out.push([1, b, y, 20 + 35 * Math.abs(Math.sin(y / 5))]);
            dir = -dir;
          }
          return out;
        });
      } else {
        moves = lineMoves(objs, op);
      }
      // Tiling: solo lo que cae en el tramo, ya desplazado (el motor lo hace sobre la geometría).
      if (options.tile) moves = clipMoves(moves, options.tile);
      // Print & Cut: la transformación del registro se aplica al trabajo ya colocado, como en el motor.
      const registration = options.transform;
      if (registration) {
        moves = moves.map((m): Move => {
          const [x, y] = applyAffine(registration, [m[1], m[2]]);
          return [m[0], x, y, m[3]];
        });
      }
      if (moves.length === 0) continue;
      collect?.({ op, moves, objects: objs });

      // Pausas y arranque con potencia extra: se suman por cada trazo (un tramo apagado seguido de uno encendido).
      let strokes = 0;
      moves.forEach((m, i) => {
        if (m[0] === 1 && (i === 0 || moves[i - 1]![0] === 0)) strokes++;
      });
      const holdSeconds =
        op.mode === 'line'
          ? (strokes * (op.start_delay_ms + op.end_delay_ms + (op.cut_through.enabled ? op.cut_through.ms : 0))) / 1000
          : 0;

      // Tiempo acumulado al final de cada movimiento (sin aceleración: distancia / velocidad).
      const cutMm = moveLength(moves, 1, cursor);
      let seconds = 0;
      for (let p = 0; p < op.passes; p++) {
        for (const m of moves) {
          const dist = Math.hypot(m[1] - cursor[0], m[2] - cursor[1]);
          seconds += (dist / (m[0] === 1 ? op.speed : device.rapid_rate)) * 60;
          cursor = [m[1], m[2]];
          flat.push(m[0], m[1], m[2], m[3]);
          moveTimes.push(total + seconds);
        }
        seconds += holdSeconds;
      }
      total += seconds;

      const explanations: string[] = [];
      if (op.mode === 'line') {
        const holes = objs.flatMap((o) => objectPolylines(o)).filter((p) => p.closed).length;
        if (holes > 1) explanations.push(`Primero se cortan las ${holes - 1} figuras interiores y al final el contorno, para que la pieza no se mueva.`);
        if (op.tabs.enabled) explanations.push(`Se dejan puentes de ${op.tabs.width} mm sin cortar.`);
        if (op.perforation.enabled) explanations.push(`Perforación: ${op.perforation.cut_mm} mm cortados y ${op.perforation.skip_mm} mm saltados.`);
        if (op.dot_mode.enabled) explanations.push(`Modo punto: un disparo de ${op.dot_mode.dwell_ms} ms cada ${op.dot_mode.spacing_mm} mm.`);
        if (op.ramp.enabled && !op.dot_mode.enabled) explanations.push(`La potencia sube y baja en ${op.ramp.length_mm} mm al empezar y terminar cada trazo.`);
        if (op.overcut > 0) explanations.push(`Los contornos cerrados siguen ${op.overcut} mm más allá del cierre.`);
        if (op.lead_in.enabled || op.lead_out.enabled) explanations.push('Entradas y salidas de contorno: solo las calcula el motor, esta simulación las omite.');
        explanations.push('Dentro de cada grupo se elige el vecino más cercano para acortar los recorridos en vacío.');
      } else if (op.mode === 'fill' && op.fill_style === 'offset') {
        explanations.push(`Contornos concéntricos cada ${op.interval} mm, de afuera hacia adentro.`);
      } else if (op.mode === 'fill') {
        explanations.push(`Barrido horizontal cada ${op.interval} mm, en zigzag; margen de aceleración de ${overscanMm.toFixed(1)} mm a cada lado.`);
        if (op.fill_style === 'flood') explanations.push('Relleno por regiones: el motor recorta los viajes sobre espacios vacíos; esta simulación usa el barrido normal.');
        if (op.fill_grouping === 'by_shape') explanations.push('Cada forma se rellena completa antes de pasar a la siguiente.');
      } else {
        explanations.push(
          op.image.pass_through
            ? `Barrido de la imagen fila por fila, sin tramar (pass-through), a ${op.angle}°.`
            : `Barrido de la imagen fila por fila con tramado ${op.image.mode}, a ${op.angle}°.`,
        );
      }
      if (op.mode !== 'line' && op.angle_increment !== 0 && op.passes > 1) explanations.push(`Cada pasada gira el barrido ${op.angle_increment}°.`);
      if (op.mode === 'line' && (op.start_delay_ms > 0 || op.end_delay_ms > 0 || op.cut_through.enabled)) {
        explanations.push(`Las pausas y el corte pasante suman ${holdSeconds.toFixed(1)} s por pasada.`);
      }

      summaries.push({
        color: layer.color,
        mode: op.mode,
        seconds,
        summary: `${label}: ${(cutMm / 10).toFixed(0)} cm a ${op.speed} mm/min, ${op.power} % de potencia, ${op.passes} ${op.passes === 1 ? 'pasada' : 'pasadas'} (${fmtDuration(seconds)})`,
        travel_saved_pct: op.mode === 'line' ? 31 : null,
        explanations,
      });
    }
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
    order_note: summaries.length > 1 ? t('layers.order.note.mock') : null,
    warnings,
  };
}
