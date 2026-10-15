import { getApi } from '@/api';
import type { DesignObject, Document, GeometryOp, GeometryOpArgs } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { objectCurves } from '@/lib/geometry';
import { expandGroups } from '@/lib/groups';
import type { BezPath } from '@/lib/bezpath';
import { pinTab, type AppState } from '@/store';
import { uniqueId } from '@/store/document';
import { selectionUnits, type Unit } from './units';
import { useArrangeUi, type OffsetParams } from './uiStore';

/** Qué impide usar un conjunto de objetos en una operación geométrica. */
export type OperandProblem = 'locked' | 'image' | 'noContour' | 'open';

export interface SelectionAnalysis {
  /** Unidades (objeto suelto o grupo) de la selección. */
  units: number;
  /** Unidades que se pueden mover (sin miembros bloqueados ni ocultos). */
  movable: number;
  /** Primer problema que impide operar, si hay alguno. */
  problem: OperandProblem | null;
}

interface KindInfo {
  empty: boolean;
  open: boolean;
}

const kindCache = new WeakMap<object, KindInfo>();

/** ¿Algún subtrayecto dibujado queda sin cerrar? Un subtrayecto cierra con `close` o si vuelve a su inicio. */
function pathHasOpen(path: BezPath): boolean {
  let start: [number, number] | null = null;
  let last: [number, number] | null = null;
  let drawn = false;
  let closed = false;
  const openNow = () => drawn && !closed && !(start && last && Math.hypot(start[0] - last[0], start[1] - last[1]) < 1e-9);
  for (const seg of path) {
    if (seg.type === 'move_to') {
      if (openNow()) return true;
      start = seg.to;
      last = seg.to;
      drawn = false;
      closed = false;
    } else if (seg.type === 'close') {
      closed = true;
    } else {
      drawn = true;
      last = seg.to;
    }
  }
  return openNow();
}

/** Información de un `kind` calculada una sola vez (los `kind` son inmutables). */
export function kindInfo(kind: DesignObject['kind']): KindInfo {
  let info = kindCache.get(kind);
  if (!info) {
    const paths = kind.type === 'vector' ? kind.paths : kind.type === 'text' ? kind.outline : [];
    info = { empty: !paths.some((p) => p.some((s) => s.type !== 'move_to' && s.type !== 'close')), open: paths.some(pathHasOpen) };
    kindCache.set(kind, info);
  }
  return info;
}

let lastAnalysis: { doc: Document; ids: readonly string[]; result: SelectionAnalysis } | null = null;

/**
 * Resume si la selección sirve para una booleana, soldar o offset. Se guarda el último resultado
 * porque los comandos lo consultan en cada cambio del estado (el cursor se mueve 60 veces por segundo).
 */
export function analyzeSelection(s: Pick<AppState, 'doc' | 'selectedIds'>): SelectionAnalysis {
  if (lastAnalysis && lastAnalysis.doc === s.doc && lastAnalysis.ids === s.selectedIds) return lastAnalysis.result;
  const objects = s.doc.objects;
  const ids = expandGroups(objects, s.selectedIds);
  const wanted = new Set(ids);
  const picked = objects.filter((o) => wanted.has(o.id));
  const groupKeys = new Set(picked.map((o) => o.group ?? `#${o.id}`));
  let problem: OperandProblem | null = null;
  if (picked.some((o) => o.locked)) problem = 'locked';
  else if (picked.some((o) => o.kind.type === 'image')) problem = 'image';
  else if (picked.some((o) => kindInfo(o.kind).empty)) problem = 'noContour';
  else if (picked.some((o) => kindInfo(o.kind).open)) problem = 'open';
  const blocked = new Set(picked.filter((o) => o.locked || o.hidden).map((o) => o.group ?? `#${o.id}`));
  const result = { units: groupKeys.size, movable: groupKeys.size - blocked.size, problem };
  lastAnalysis = { doc: s.doc, ids: s.selectedIds, result };
  return result;
}

const PROBLEM_REASON: Record<OperandProblem, I18nKey> = {
  locked: 'arrange.reason.locked',
  image: 'arrange.reason.image',
  noContour: 'arrange.reason.noContour',
  open: 'arrange.reason.open',
};

const busyReason = (): I18nKey | null => (useArrangeUi.getState().busy ? 'arrange.reason.busy' : null);

/** Motivo por el que una booleana, soldar u offset no está disponible, o `true`. */
export function geometryReason(s: Pick<AppState, 'doc' | 'selectedIds'>, op: GeometryOp): true | I18nKey {
  if (s.selectedIds.length === 0) return 'arrange.reason.noSelection';
  const a = analyzeSelection(s);
  if (a.units === 0) return 'arrange.reason.noSelection';
  if (a.problem) return PROBLEM_REASON[a.problem];
  const isBoolean = op === 'union' || op === 'difference' || op === 'intersection' || op === 'xor';
  if (isBoolean && a.units < 2) return 'arrange.reason.needTwoClosed';
  return busyReason() ?? true;
}

/** Un grupo viaja como un solo objeto con todas sus curvas (así el motor ve un único operando). */
export function operandOf(unit: Unit): DesignObject {
  if (unit.objects.length === 1) return unit.objects[0]!;
  const first = unit.objects[0]!;
  return {
    id: unit.id,
    layer: first.layer,
    transform: [1, 0, 0, 1, 0, 0],
    kind: { type: 'vector', paths: unit.objects.flatMap(objectCurves) },
    locked: false,
    hidden: false,
    group: null,
  };
}

/** Las unidades con la principal (A) primero y el resto en el orden de selección. */
export function orderedUnits(units: readonly Unit[], primaryId?: string | null): Unit[] {
  const primary = units.find((u) => u.id === primaryId || u.ids.includes(primaryId ?? ''));
  return primary ? [primary, ...units.filter((u) => u !== primary)] : [...units];
}

const HISTORY: Record<GeometryOp, I18nKey> = {
  union: 'arrange.history.union',
  difference: 'arrange.history.difference',
  intersection: 'arrange.history.intersection',
  xor: 'arrange.history.xor',
  weld: 'arrange.history.weld',
  offset: 'arrange.history.offset',
  join: 'arrange.history.join',
  close: 'arrange.history.close',
  dedupe: 'arrange.history.dedupe',
};

/**
 * Cambia los operandos por el resultado en un solo documento nuevo: los resultados ocupan el lugar
 * del primer operando en el orden de dibujo y reciben ids propios. Si ya no existe ningún operando
 * (se borraron mientras el motor calculaba) devuelve el mismo documento.
 */
export function replaceWithResults(doc: Document, removeIds: ReadonlySet<string>, results: readonly DesignObject[], idBase: string, created: string[]): Document {
  const present = doc.objects.some((o) => removeIds.has(o.id));
  if (removeIds.size > 0 && !present) return doc;
  const taken = new Set(doc.objects.map((o) => o.id));
  const fresh = results.map((r) => {
    const id = uniqueId(idBase, taken);
    taken.add(id);
    created.push(id);
    return { ...r, id, transform: r.transform, locked: false, hidden: false, group: null };
  });
  if (removeIds.size === 0) return { ...doc, objects: [...doc.objects, ...fresh] };
  const objects: DesignObject[] = [];
  let placed = false;
  for (const o of doc.objects) {
    if (!removeIds.has(o.id)) {
      objects.push(o);
    } else if (!placed) {
      objects.push(...fresh);
      placed = true;
    }
  }
  return { ...doc, objects };
}

export interface GeometryRun {
  op: GeometryOp;
  /** Booleanas: unidad principal (A). */
  primaryId?: string | null;
  offset?: Pick<OffsetParams, 'distance' | 'join' | 'keepOriginal'>;
}

/**
 * Ejecuta una operación geométrica sobre la selección: pide el resultado al motor y lo aplica como
 * un solo paso de deshacer. Devuelve true si cambió el documento. Los errores del motor (por ejemplo,
 * formas abiertas) se avisan en español y no cambian nada.
 */
export async function runGeometry(s: AppState, run: GeometryRun): Promise<boolean> {
  const reason = geometryReason(s, run.op);
  if (reason !== true) {
    s.notify('warning', t(reason));
    return false;
  }
  const units = orderedUnits(selectionUnits(s.doc, s.selectedIds), run.primaryId);
  const args: GeometryOpArgs = { op: run.op, objects: units.map(operandOf) };
  if (run.op === 'offset') {
    args.distance = run.offset?.distance ?? 0;
    args.join = run.offset?.join ?? 'round';
  }
  const inTab = pinTab(s);
  const ui = useArrangeUi.getState();
  ui.setBusy(true);
  let results: DesignObject[];
  try {
    results = await getApi().geometryOp(args);
  } catch (e) {
    s.notify('error', errorMessage(e));
    return false;
  } finally {
    useArrangeUi.getState().setBusy(false);
  }
  if (results.length === 0) {
    s.notify('warning', t(run.op === 'offset' ? 'arrange.msg.offsetEmpty' : run.op === 'intersection' ? 'arrange.msg.noOverlap' : 'arrange.msg.empty'));
    return false;
  }

  // Qué operandos se consumen: todos, salvo en offset con "conservar el original" (y entonces solo los que dieron resultado).
  const remove = new Set<string>();
  if (run.op !== 'offset') for (const u of units) for (const id of u.ids) remove.add(id);
  else if (!run.offset?.keepOriginal) {
    for (const u of units) {
      const own = new RegExp(`^${u.id.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}-off-\\d+$`);
      if (results.some((r) => own.test(r.id))) for (const id of u.ids) remove.add(id);
    }
  }
  const created: string[] = [];
  // El resultado se escribe en la pestaña donde empezó la operación, aunque ahora haya otra activa.
  const applied = inTab(() => {
    s.applyDocChange(HISTORY[run.op], (doc) => {
      created.length = 0;
      return replaceWithResults(doc, remove, results, run.op, created);
    });
    if (created.length > 0) s.select(created);
    return true;
  });
  return applied === true && created.length > 0;
}
