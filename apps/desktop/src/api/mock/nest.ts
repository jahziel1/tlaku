import type { Affine, DesignObject, Point } from '../types';
import type { NestPlacement, NestRef, NestRequest, NestSheet, NestStatus } from '@/features/nest/types';
import { objectPolylines, pointsBounds, type Bounds } from '@/lib/geometry';
import type { LaserApi } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

/** Lo que tarda el acomodo simulado en "revelar" todas las piezas. */
export const MOCK_NEST_MS = 1500;

interface Instance {
  ref: NestRef;
  w: number;
  h: number;
  /** Mínimo de la caja de la pieza (para llevarla al origen antes de colocarla). */
  min: Point;
  max: Point;
  area: number;
  /** Puede girar 90° (ni «sin giro» ni veta, y las opciones lo permiten). */
  canRotate: boolean;
}

/** Una hoja usada: su caja, su área y lo que cayó en ella. */
interface SheetPlan {
  source: NestSheet['source'];
  remnantId: string | null;
  bounds: [number, number, number, number];
  area: number;
  /** Índices en `Job.placements`. */
  members: number[];
  /** Sobrante simulado: el rectángulo libre sobre la última fila (si es aprovechable). */
  leftovers: [number, number][][][];
}

interface Job {
  startedAt: number;
  /** Instante en que se pidió parar; congela el avance. */
  stoppedAt: number | null;
  placements: NestPlacement[];
  areas: number[];
  sheets: SheetPlan[];
  /** Cuántas no cupieron por tamaño (se calcularon de entrada). */
  unfit: NestRef[];
  total: number;
  timeLimitMs: number;
}

const polys = (objects: readonly DesignObject[]) => objects.flatMap((o) => objectPolylines(o));

function pointInPolygon([x, y]: Point, poly: readonly Point[]): boolean {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [xi, yi] = poly[i]!;
    const [xj, yj] = poly[j]!;
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

const shoelace = (pts: readonly Point[]): number => {
  let s = 0;
  for (let i = 0; i < pts.length; i++) {
    const [x0, y0] = pts[i]!;
    const [x1, y1] = pts[(i + 1) % pts.length]!;
    s += x0 * y1 - x1 * y0;
  }
  return Math.abs(s) / 2;
};

/** Área neta (regla par-impar): los contornos anidados a profundidad impar restan. */
function netArea(objects: readonly DesignObject[]): number {
  const closed = polys(objects).filter((p) => p.closed && p.points.length >= 3);
  let total = 0;
  closed.forEach((p, i) => {
    const depth = closed.filter((q, j) => j !== i && pointInPolygon(p.points[0]!, q.points)).length;
    total += (depth % 2 === 0 ? 1 : -1) * shoelace(p.points);
  });
  return Math.max(0, total);
}

function invalid(msg: string): never {
  throw new Error(msg);
}

/** Valida la petición como lo haría el motor; los mensajes van en español. */
function validate(req: NestRequest): Bounds {
  if (!req.pieces || req.pieces.length === 0 || req.pieces.every((p) => !(p.copies >= 1))) {
    invalid('No hay piezas que acomodar');
  }
  if (!(req.spacing >= 0) || !(req.margin >= 0) || !Number.isFinite(req.spacing) || !Number.isFinite(req.margin)) {
    invalid('La separación y el margen deben ser números no negativos');
  }
  if (!(req.time_limit_s >= 1 && req.time_limit_s <= 600)) invalid('El tope de tiempo debe estar entre 1 y 600 segundos');
  if (!Number.isInteger(req.seed) || req.seed < 0) invalid('La semilla debe ser un entero mayor o igual que 0');
  if (!Number.isInteger(req.max_sheets) || req.max_sheets < 1) invalid('El máximo de hojas debe ser al menos 1');
  for (const r of req.remnants) {
    if (r.paths.length === 0 || r.paths.some((p) => p.length < 3)) invalid(`El retazo «${r.id}» no tiene un contorno cerrado válido.`);
  }
  const c = req.container;
  if (c.type === 'rect') {
    if (!(c.width > 0 && c.height > 0) || !Number.isFinite(c.width) || !Number.isFinite(c.height)) {
      invalid('El contenedor debe tener ancho y alto positivos');
    }
    return [[0, 0], [c.width, c.height]];
  }
  const closed = objectPolylines(c.object).filter((p) => p.closed && p.points.length >= 3);
  const b = pointsBounds(closed.flatMap((p) => p.points));
  if (!b || !(b[1][0] > b[0][0] && b[1][1] > b[0][1])) invalid('El material debe ser un contorno cerrado');
  return b;
}

function pieceInstances(req: NestRequest): Instance[] {
  const instances: Instance[] = [];
  for (const piece of req.pieces) {
    const pts = polys(piece.objects).flatMap((p) => p.points);
    const b = pointsBounds(pts);
    const netA = netArea(piece.objects);
    // El simulado solo gira 90°: «sin giro» y veta (0° y 180°) no lo permiten.
    const canRotate = req.rotation.mode !== 'none' && piece.rotation === 'inherit';
    for (let copy = 0; copy < piece.copies; copy++) {
      if (!b) continue;
      instances.push({ ref: { piece: piece.id, copy }, w: b[1][0] - b[0][0], h: b[1][1] - b[0][1], min: b[0], max: b[1], area: netA, canRotate });
    }
  }
  return instances;
}

/** Acomodo por filas con cajas envolventes en una hoja: 0°, o 90° si la pieza lo admite y no cabe de otra forma. */
function shelfPack(
  instances: readonly Instance[],
  req: Pick<NestRequest, 'spacing' | 'margin'>,
  area: Bounds,
): { placements: NestPlacement[]; placed: Instance[]; rest: Instance[]; usedTop: number } {
  const left = area[0][0] + req.margin;
  const right = area[1][0] - req.margin;
  const bottom = area[0][1] + req.margin;
  const top = area[1][1] - req.margin;
  const eps = 1e-9;

  const placements: NestPlacement[] = [];
  const placed: Instance[] = [];
  const rest: Instance[] = [];
  let x = left;
  let y = bottom;
  let rowH = 0;

  /** Intenta poner la caja `w`×`h`: en la fila actual y, si no, en una fila nueva. */
  const fit = (w: number, h: number): Point | null => {
    if (x + w <= right + eps && y + h <= top + eps) return [x, y];
    const ny = y + rowH + (rowH > 0 ? req.spacing : 0);
    if (left + w <= right + eps && ny + h <= top + eps) return [left, ny];
    return null;
  };

  for (const inst of instances) {
    let spot = fit(inst.w, inst.h);
    let rotated = false;
    if (!spot && inst.canRotate) {
      spot = fit(inst.h, inst.w);
      rotated = spot !== null;
    }
    if (!spot) {
      rest.push(inst);
      continue;
    }
    const [w, h] = rotated ? [inst.h, inst.w] : [inst.w, inst.h];
    if (spot[1] !== y) {
      y = spot[1];
      rowH = 0;
    }
    // 90° antihorario: (x, y) -> (-y, x); la caja gira y su mínimo pasa a (-maxY, minX).
    const transform: Affine = rotated
      ? [0, 1, -1, 0, spot[0] + inst.max[1], spot[1] - inst.min[0]]
      : [1, 0, 0, 1, spot[0] - inst.min[0], spot[1] - inst.min[1]];
    placements.push({ piece: inst.ref.piece, sheet: 0, copy: inst.ref.copy, rotation_deg: rotated ? 90 : 0, mirror: false, transform });
    placed.push(inst);
    x = spot[0] + w + req.spacing;
    rowH = Math.max(rowH, h);
  }
  return { placements, placed, rest, usedTop: y + rowH };
}

/** Caja y área de un retazo a partir de sus contornos (el primero es el exterior). */
function remnantArea(paths: readonly [number, number][][]): { bounds: Bounds; area: number } {
  const b = pointsBounds(paths.flat()) ?? [[0, 0], [0, 0]];
  const [outer, ...holes] = paths.map((p) => shoelace(p));
  return { bounds: b, area: Math.max(0, (outer ?? 0) - holes.reduce((a, h) => a + h, 0)) };
}

/**
 * Reparte las piezas: primero los retazos (los que no admiten nada se saltan) y luego hojas nuevas
 * iguales al contenedor, hasta `max_sheets` hojas usadas.
 */
function packSheets(req: NestRequest, area: Bounds): { placements: NestPlacement[]; areas: number[]; sheets: SheetPlan[]; unfit: NestRef[] } {
  let pending = pieceInstances(req);
  const placements: NestPlacement[] = [];
  const areas: number[] = [];
  const sheets: SheetPlan[] = [];
  const containerArea = req.container.type === 'rect' ? req.container.width * req.container.height : netArea([req.container.object]);
  const cb: [number, number, number, number] = [area[0][0], area[0][1], area[1][0], area[1][1]];
  const sources = [
    ...req.remnants.map((r) => ({ source: 'remnant' as const, id: r.id, ...remnantArea(r.paths), fresh: false })),
    ...Array.from({ length: req.max_sheets }, () => ({ source: 'container' as const, id: null, bounds: area, area: containerArea, fresh: true })),
  ];
  for (const src of sources) {
    if (pending.length === 0 || sheets.length >= req.max_sheets) break;
    const r = shelfPack(pending, req, src.bounds);
    if (r.placements.length === 0) {
      if (src.fresh) break; // ni una hoja nueva admite lo que queda
      continue;
    }
    const k = sheets.length;
    const members = r.placements.map((_, i) => placements.length + i);
    placements.push(...r.placements.map((p) => ({ ...p, sheet: k })));
    areas.push(...r.placed.map((i) => i.area));
    const bounds: [number, number, number, number] =
      src.source === 'container' ? cb : [src.bounds[0][0], src.bounds[0][1], src.bounds[1][0], src.bounds[1][1]];
    // Sobrante simulado: la franja libre sobre la última fila, si es aprovechable.
    const min = Math.max(5, req.leftover_min_mm);
    const y0 = r.usedTop + req.spacing;
    const free = bounds[3] - req.margin - y0;
    const wide = bounds[2] - bounds[0] - 2 * req.margin;
    const leftovers: [number, number][][][] =
      req.leftover_min_mm > 0 && free >= min && wide >= min
        ? [[[[bounds[0] + req.margin, y0], [bounds[2] - req.margin, y0], [bounds[2] - req.margin, bounds[3] - req.margin], [bounds[0] + req.margin, bounds[3] - req.margin]]]]
        : [];
    sheets.push({ source: src.source, remnantId: src.id, bounds, area: src.area, members, leftovers });
    pending = r.rest;
  }
  return { placements, areas, sheets, unfit: pending.map((i) => i.ref) };
}

/**
 * Métodos simulados del área `nest`: un acomodo por filas (con varias hojas y retazos) que se revela
 * en 1,5 s. `now` se puede inyectar en las pruebas para controlar el tiempo.
 */
export function mockNest(lag: Lag, now: () => number = Date.now): Pick<LaserApi, 'nestStart' | 'nestPoll' | 'nestCancel'> {
  const jobs = new Map<string, Job>();
  let counter = 0;
  let latest: string | null = null;

  /** Un trabajo nuevo cancela el anterior. */
  const stop = (id: string | null) => {
    const job = id ? jobs.get(id) : undefined;
    if (job && job.stoppedAt === null) job.stoppedAt = Math.min(now(), job.startedAt + endAfter(job));
  };
  const endAfter = (job: Job) => Math.min(MOCK_NEST_MS, job.timeLimitMs);

  return {
    nestStart: ({ request }) =>
      lag(() => {
        const area = validate(request);
        const { placements, areas, sheets, unfit } = packSheets(request, area);
        stop(latest);
        const id = `nest-${++counter}`;
        jobs.set(id, {
          startedAt: now(),
          stoppedAt: null,
          placements,
          areas,
          sheets,
          unfit,
          total: placements.length + unfit.length,
          timeLimitMs: request.time_limit_s * 1000,
        });
        latest = id;
        return id;
      }),

    nestPoll: ({ job_id }) =>
      lag((): NestStatus => {
        const job = jobs.get(job_id);
        if (!job) invalid('El trabajo de acomodo no existe o ya terminó');
        const limit = endAfter(job);
        const end = job.startedAt + limit;
        const at = Math.min(job.stoppedAt ?? now(), end);
        const elapsed = Math.max(0, at - job.startedAt);
        const done = job.stoppedAt !== null || elapsed >= limit;
        const shown = Math.floor((job.placements.length * elapsed) / MOCK_NEST_MS);
        const placements = job.placements.slice(0, shown);
        const unplaced: NestRef[] = done
          ? [...job.placements.slice(shown).map((p) => ({ piece: p.piece, copy: p.copy })), ...job.unfit]
          : [];
        // Solo se ven las hojas que ya recibieron alguna pieza; el sobrante, al terminar y completo.
        const sheets: NestSheet[] = [];
        let placedArea = 0;
        let totalArea = 0;
        job.sheets.forEach((sp, k) => {
          const seen = sp.members.filter((m) => m < shown);
          if (seen.length === 0) return;
          const a = seen.reduce((sum, m) => sum + (job.areas[m] ?? 0), 0);
          placedArea += a;
          totalArea += sp.area;
          sheets.push({
            index: k,
            source: sp.source,
            remnant_id: sp.remnantId,
            bounds: sp.bounds,
            utilization: sp.area > 0 ? Math.min(100, (a / sp.area) * 100) : 0,
            placed: seen.length,
            area_mm2: sp.area,
            placed_area_mm2: a,
            leftovers: done && shown === job.placements.length ? sp.leftovers : [],
          });
        });
        return {
          running: !done,
          elapsed_ms: elapsed,
          evaluations: Math.round(elapsed * 4),
          utilization: totalArea > 0 ? Math.min(100, (placedArea / totalArea) * 100) : 0,
          placed: placements.length,
          total: job.total,
          placements,
          sheets,
          unplaced,
          timed_out: done && job.timeLimitMs < MOCK_NEST_MS && elapsed >= job.timeLimitMs,
          error: null,
        };
      }),

    nestCancel: ({ job_id }) =>
      lag(() => {
        stop(jobs.has(job_id) ? job_id : null);
      }),
  };
}
