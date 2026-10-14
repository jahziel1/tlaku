import { getApi } from '@/api';
import type { DesignObject, Document } from '@/api/types';
import { replaceCleaned, type CleanupOp } from '@/features/arrange/cleanup';
import { textToVector } from '@/features/text/convert';
import type { I18nKey } from '@/i18n';
import { t, tn } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';
import { useReviewUi } from './store';
import type { Finding, FindingKind } from './types';

/**
 * Los arreglos de «Revisar diseño». No calculan geometría: llaman a las operaciones que ya existen
 * (`geometry_op` con unir, cerrar y quitar duplicados; `shape_op` con optimizar) sobre los objetos
 * del hallazgo y aplican el resultado como UN solo paso de deshacer, igual que `runCleanup`.
 */

/** Los únicos que entran en «Arreglar todo lo seguro»: no borran ni mueven nada. */
export const SAFE_KINDS: readonly FindingKind[] = ['overlap', 'joinable', 'near_closed'];

/** Qué hallazgos tienen un botón «Arreglar». Los demás solo se seleccionan o ignoran. */
const FIXABLE: readonly FindingKind[] = [...SAFE_KINDS, 'tiny', 'extra_nodes', 'text', 'silent_layer', 'hidden_layer'];

export const canFix = (f: Finding): boolean => FIXABLE.includes(f.kind);
export const isSafe = (f: Finding): boolean => SAFE_KINDS.includes(f.kind);

/** Un paso de limpieza sobre un conjunto de objetos. */
interface Step {
  op: CleanupOp | 'optimize';
  tolerance: number | null;
}

/** Ids con los que se llama a la operación: `fix_ids` si el motor los pide (quitar duplicados), si no los afectados. */
const targets = (f: Finding): string[] => (f.fix_ids && f.fix_ids.length > 0 ? f.fix_ids : f.object_ids);

const stepOf = (kind: FindingKind): Step | null => {
  switch (kind) {
    case 'overlap':
      return { op: 'dedupe', tolerance: null };
    case 'joinable':
      return { op: 'join', tolerance: null };
    case 'near_closed':
      return { op: 'close', tolerance: null };
    case 'extra_nodes':
      return { op: 'optimize', tolerance: null };
    default:
      return null;
  }
};

const ID_BASE: Record<Step['op'], string> = { join: 'join', close: 'close', dedupe: 'dedupe', optimize: 'opt' };

interface Computed {
  doc: Document;
  created: string[];
}

/**
 * Corre los pasos uno tras otro sobre una copia del documento. Tras cada uno, el conjunto de ids de
 * trabajo pierde los objetos consumidos y gana los nuevos, para que el paso siguiente siga lo que
 * quedó de ellos. No toca el store.
 */
async function compute(start: Document, scope: Set<string>, steps: readonly Step[]): Promise<Computed> {
  const api = getApi();
  let working = start;
  const created: string[] = [];
  for (const step of steps) {
    const operands = working.objects.filter((o) => scope.has(o.id) && !o.hidden && !o.locked && o.kind.type === 'vector');
    if (operands.length === 0) continue;
    let results: DesignObject[];
    if (step.op === 'optimize') {
      const r = await api.shapeOp({ op: 'optimize', objects: operands, params: { tolerance: step.tolerance ?? 0.02, smooth: false, corner_angle: 35 } });
      results = r.objects;
    } else {
      results = await api.geometryOp({ op: step.op, objects: operands, ...(step.tolerance !== null ? { tolerance: step.tolerance } : {}) });
    }
    const sent = new Set(operands.map((o) => o.id));
    const returned = new Set(results.map((r) => r.id));
    const consumed = operands.filter((o) => !returned.has(o.id)).map((o) => o.id);
    const fresh = results.filter((r) => !sent.has(r.id));
    if (consumed.length === 0 && fresh.length === 0) continue;
    const made: string[] = [];
    working = replaceCleaned(working, new Set(consumed), fresh, ID_BASE[step.op], made);
    for (const id of consumed) scope.delete(id);
    for (const id of made) scope.add(id);
    created.push(...made);
  }
  return { doc: working, created };
}

/**
 * Aplica el resultado de `compute` como un solo paso de deshacer. Si el diseño cambió mientras el
 * motor calculaba, no aplica nada (el resultado ya no corresponde) y avisa.
 */
function commit(snapshot: Document, computed: Computed, label: I18nKey, done: string): boolean {
  const s = useStore.getState();
  if (s.doc !== snapshot) {
    s.notify('warning', t('review.msg.changed'));
    return false;
  }
  if (computed.doc === snapshot) {
    s.notify('info', t('review.msg.nothing'));
    return false;
  }
  s.applyDocChange(label, (doc) => (doc === snapshot ? computed.doc : doc));
  if (computed.created.length > 0) s.select(computed.created);
  s.notify('success', done);
  return true;
}

const sameColor = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** Corre `task` con los botones del panel en espera y avisa los errores del motor. */
async function guarded(task: () => Promise<boolean> | boolean): Promise<boolean> {
  const ui = useReviewUi.getState();
  if (ui.busy) return false;
  ui.set({ busy: true });
  try {
    return await task();
  } catch (e) {
    useStore.getState().notify('error', errorMessage(e));
    return false;
  } finally {
    useReviewUi.getState().set({ busy: false });
  }
}

/** Arregla un hallazgo con un solo paso de deshacer. Devuelve si cambió el documento. */
export function fixFinding(f: Finding): Promise<boolean> {
  return guarded(async () => {
    const s = useStore.getState();
    const label = `review.history.${f.kind}` as I18nKey;
    const done = t('review.msg.fixed', { what: t(`review.fix.${f.kind}` as I18nKey).toLowerCase() });
    const snapshot = s.doc;
    switch (f.kind) {
      case 'silent_layer':
      case 'hidden_layer': {
        const patch = f.kind === 'silent_layer' ? { output: true } : { visible: true };
        const layer = f.layer;
        if (!layer) return false;
        return commit(
          snapshot,
          { doc: { ...snapshot, layers: snapshot.layers.map((l) => (sameColor(l.color, layer) ? { ...l, ...patch } : l)) }, created: [] },
          label,
          done,
        );
      }
      case 'text': {
        let next = snapshot;
        for (const id of f.object_ids) next = textToVector(next, id);
        return commit(snapshot, { doc: next, created: [] }, label, done);
      }
      case 'tiny': {
        const ids = new Set(f.object_ids);
        const objects = snapshot.objects.filter((o) => !ids.has(o.id) || o.locked);
        if (objects.length === snapshot.objects.length) return false;
        const committed = commit(snapshot, { doc: { ...snapshot, objects }, created: [] }, label, done);
        if (committed) useStore.getState().select(useStore.getState().selectedIds.filter((id) => !ids.has(id)));
        return committed;
      }
      default: {
        const step = stepOf(f.kind);
        if (!step) return false;
        const computed = await compute(snapshot, new Set(targets(f)), [{ ...step, tolerance: f.tolerance }]);
        return commit(snapshot, computed, label, done);
      }
    }
  });
}

/**
 * «Arreglar todo lo seguro»: quitar duplicados, unir y cerrar, en ese orden y capa por capa, todo en
 * un solo paso de deshacer. Solo mira los hallazgos seguros de la lista que se le da (sin los ignorados).
 */
export function fixSafe(findings: readonly Finding[]): Promise<boolean> {
  return guarded(async () => {
    const safe = findings.filter(isSafe);
    if (safe.length === 0) {
      useStore.getState().notify('info', t('review.fixSafe.none'));
      return false;
    }
    const snapshot = useStore.getState().doc;
    let working = snapshot;
    const created: string[] = [];
    // Por capa, para que cada capa siga a sus propios objetos.
    for (const layer of [...new Set(safe.map((f) => f.layer ?? ''))]) {
      const mine = safe.filter((f) => (f.layer ?? '') === layer);
      const scope = new Set(mine.flatMap(targets));
      const steps = SAFE_KINDS.flatMap((kind) => {
        const found = mine.find((f) => f.kind === kind);
        const base = stepOf(kind);
        return found && base ? [{ ...base, tolerance: found.tolerance }] : [];
      });
      const r = await compute(working, scope, steps);
      working = r.doc;
      created.push(...r.created);
    }
    return commit(snapshot, { doc: working, created }, 'review.history.safe', t('review.msg.safe'));
  });
}

/** Selecciona los objetos del hallazgo (sin moverlos ni cambiarlos). */
export function selectFinding(f: Finding): void {
  const s = useStore.getState();
  s.select(f.object_ids);
  s.notify('info', tn('review.msg.selected', f.object_ids.length));
}
