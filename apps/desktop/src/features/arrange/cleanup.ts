import { getApi } from '@/api';
import type { DesignObject, Document } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { objectPolylines, polylineLength } from '@/lib/geometry';
import { expandGroups } from '@/lib/groups';
import { fmt } from '@/lib/platform';
import { pinTab, type AppState } from '@/store';
import { uniqueId } from '@/store/document';
import { kindInfo } from './geomOps';
import { useArrangeUi, type CleanupOp } from './uiStore';

export { CLEANUP_DEFAULTS, type CleanupOp } from './uiStore';

const HISTORY: Record<CleanupOp, I18nKey> = {
  join: 'arrange.history.join',
  close: 'arrange.history.close',
  dedupe: 'arrange.history.dedupe',
};

/** Objetos sobre los que operaría la limpieza: la selección con sus grupos completos, sin los ocultos. */
export function cleanupOperands(s: Pick<AppState, 'doc' | 'selectedIds'>): DesignObject[] {
  const wanted = new Set(expandGroups(s.doc.objects, s.selectedIds));
  return s.doc.objects.filter((o) => wanted.has(o.id) && !o.hidden);
}

/** Motivo por el que una limpieza no está disponible, o `true`. */
export function cleanupReason(s: Pick<AppState, 'doc' | 'selectedIds'>, op: CleanupOp): true | I18nKey {
  if (s.selectedIds.length === 0) return 'arrange.reason.noSelection';
  const objects = cleanupOperands(s);
  if (objects.length === 0) return 'arrange.reason.noSelection';
  if (objects.some((o) => o.locked)) return 'arrange.reason.locked';
  if (objects.some((o) => o.kind.type !== 'vector')) return 'arrange.reason.needVectors';
  // Unir y cerrar solo tienen sentido si hay algún trazo abierto; quitar duplicados sirve con cualquiera.
  if (op !== 'dedupe' && !objects.some((o) => kindInfo(o.kind).open)) return 'arrange.reason.noOpenPaths';
  return useArrangeUi.getState().busy ? 'arrange.reason.busy' : true;
}

/**
 * Cambia los objetos consumidos por los nuevos en un solo documento: los nuevos ocupan el lugar del
 * primer consumido en el orden de dibujo, con ids propios libres en el documento; conservan la capa,
 * el grupo y lo oculto que decidió el motor. Si ya no existe ninguno de los consumidos (se borraron
 * mientras el motor calculaba) devuelve el mismo documento.
 */
export function replaceCleaned(doc: Document, consumed: ReadonlySet<string>, fresh: readonly DesignObject[], idBase: string, created: string[]): Document {
  if (!doc.objects.some((o) => consumed.has(o.id))) return doc;
  const taken = new Set(doc.objects.map((o) => o.id));
  const renamed = fresh.map((r) => {
    const id = uniqueId(idBase, taken);
    taken.add(id);
    created.push(id);
    return { ...r, id, locked: false };
  });
  const objects: DesignObject[] = [];
  let placed = false;
  for (const o of doc.objects) {
    if (!consumed.has(o.id)) objects.push(o);
    else if (!placed) {
      objects.push(...renamed);
      placed = true;
    }
  }
  return { ...doc, objects };
}

interface PathStats {
  open: number;
  closed: number;
  length: number;
}

function pathStats(objects: readonly DesignObject[]): PathStats {
  const stats: PathStats = { open: 0, closed: 0, length: 0 };
  for (const o of objects) {
    for (const p of objectPolylines(o)) {
      if (p.closed) stats.closed++;
      else stats.open++;
      stats.length += polylineLength(p.points, p.closed);
    }
  }
  return stats;
}

/** Aviso con el resultado de una limpieza, a partir de los objetos consumidos y los nuevos. */
export function cleanupSummary(op: CleanupOp, consumed: readonly DesignObject[], fresh: readonly DesignObject[]): string {
  const before = pathStats(consumed);
  const after = pathStats(fresh);
  switch (op) {
    case 'join': {
      const closed = Math.max(0, after.closed - before.closed);
      // Los trazos abiertos que quedan, más los que se cerraron al unirse, son lo que salió de la unión.
      const to = after.open + closed;
      if (to >= before.open) return t('arrange.msg.joinOnlyClosed', { closed });
      return closed > 0 ? t('arrange.msg.joinClosed', { from: before.open, to, closed }) : t('arrange.msg.join', { from: before.open, to });
    }
    case 'close':
      return t('arrange.msg.close', { n: Math.max(0, before.open - after.open) });
    case 'dedupe':
      return t('arrange.msg.dedupe', { mm: fmt(Math.max(0, before.length - after.length), 2), n: consumed.length });
  }
}

/**
 * Ejecuta una limpieza sobre la selección: el motor devuelve todos los objetos (los sin cambio con su
 * mismo id, los modificados como objetos nuevos) y la interfaz quita los ids que ya no vuelven, deja
 * el resto y mete los nuevos donde estaba el primero quitado. Un solo paso de deshacer. Devuelve
 * true si cambió el documento; si no había nada que hacer, avisa y no toca nada.
 */
export async function runCleanup(s: AppState, op: CleanupOp, tolerance: number): Promise<boolean> {
  const reason = cleanupReason(s, op);
  if (reason !== true) {
    s.notify('warning', t(reason));
    return false;
  }
  const operands = cleanupOperands(s);
  const inTab = pinTab(s);
  const ui = useArrangeUi.getState();
  ui.setBusy(true);
  let results: DesignObject[];
  try {
    results = await getApi().geometryOp({ op, objects: operands, tolerance });
  } catch (e) {
    s.notify('error', errorMessage(e));
    return false;
  } finally {
    useArrangeUi.getState().setBusy(false);
  }

  const sent = new Set(operands.map((o) => o.id));
  const returned = new Set(results.map((r) => r.id));
  const consumed = operands.filter((o) => !returned.has(o.id));
  const fresh = results.filter((r) => !sent.has(r.id));
  if (consumed.length === 0 && fresh.length === 0) {
    s.notify('info', t(`arrange.msg.${op}None` as I18nKey));
    return false;
  }

  const created: string[] = [];
  const removeIds = new Set(consumed.map((o) => o.id));
  const applied = inTab(() => {
    s.applyDocChange(HISTORY[op], (doc) => {
      created.length = 0;
      return replaceCleaned(doc, removeIds, fresh, op, created);
    });
    const kept = results.filter((r) => sent.has(r.id)).map((r) => r.id);
    s.select([...kept, ...created]);
    return true;
  });
  if (!applied) return false; // la pestaña se cerró mientras tanto
  s.notify('success', cleanupSummary(op, consumed, fresh));
  return true;
}
