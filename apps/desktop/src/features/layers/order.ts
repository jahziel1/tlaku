import type { Document, LayerOrder, LayerSettings } from '@/api/types';
import { t } from '@/i18n';
import { countByLayer, type LayerCount } from '@/lib/layers';
import { colorName, isGuideColor } from '@/lib/palette';

/**
 * Cálculo local del orden de ejecución. Replica las reglas del motor (`laser-pipeline`,
 * `layers.rs`) para mostrar el orden al instante; cuando hay vista previa, el texto del
 * motor (`JobPreview.order_note`) manda.
 */

/** Tipo de operación de una capa, de menor a mayor riesgo: los grabados van antes que los cortes. */
export type LayerRole = 'engrave' | 'line' | 'cut';

const ROLE_RANK: Record<LayerRole, number> = { engrave: 0, line: 1, cut: 2 };

/** Potencia (%) desde la que una línea con varias pasadas se considera corte (igual que el motor). */
export const CUT_POWER_PCT = 80;

export type CutReason = 'kerf' | 'power';

export interface Classification {
  role: LayerRole;
  /** Por qué una capa de línea se trata como corte. */
  reason: CutReason | null;
}

export function classifyLayer(layer: LayerSettings): Classification {
  if (layer.mode !== 'line') return { role: 'engrave', reason: null };
  if (layer.kerf !== 0) return { role: 'cut', reason: 'kerf' };
  if (layer.power >= CUT_POWER_PCT && layer.passes > 1) return { role: 'cut', reason: 'power' };
  return { role: 'line', reason: null };
}

/** Por qué una capa no se ejecuta. */
export type SkipReason = 'guide' | 'off' | 'empty';

export interface ExecEntry {
  layer: LayerSettings;
  role: LayerRole;
  reason: CutReason | null;
  runs: boolean;
  skip: SkipReason | null;
  /** Posición (desde 1) en la ejecución; `null` si no se ejecuta. */
  position: number | null;
  count: LayerCount;
}

/**
 * Índices en el orden de ejecución. `forced` trae posiciones desde 1 (o null). Una posición mayor
 * que el número de capas equivale a "al final". El orden automático es estable.
 */
export function layerSequence(roles: readonly LayerRole[], forced: readonly (number | null)[], mode: LayerOrder): number[] {
  const seq = roles.map((_, i) => i).filter((i) => forced[i] === null);
  if (mode === 'auto') seq.sort((a, b) => ROLE_RANK[roles[a]!] - ROLE_RANK[roles[b]!] || a - b);
  const pinned = roles
    .map((_, i) => i)
    .filter((i) => forced[i] !== null)
    .sort((a, b) => forced[a]! - forced[b]! || a - b);
  for (const i of pinned) seq.splice(Math.min(Math.max(forced[i]! - 1, 0), seq.length), 0, i);
  return seq;
}

function skipReason(layer: LayerSettings, count: LayerCount): SkipReason | null {
  if (isGuideColor(layer.color)) return 'guide';
  if (!layer.output) return 'off';
  return count.live === 0 ? 'empty' : null;
}

/**
 * Todas las capas con su posición de ejecución: primero las que se ejecutan, en orden, y
 * después las que no (sin salida o sin objetos), en el orden del archivo.
 */
export function buildExecution(layers: readonly LayerSettings[], counts: ReadonlyMap<string, LayerCount>, mode: LayerOrder): ExecEntry[] {
  const info = layers.map((layer) => {
    const count = counts.get(layer.color.toLowerCase()) ?? { total: 0, live: 0 };
    const skip = skipReason(layer, count);
    return { layer, count, skip, ...classifyLayer(layer) };
  });
  const runIdx = info.map((_, i) => i).filter((i) => info[i]!.skip === null);
  const seq = layerSequence(
    runIdx.map((i) => info[i]!.role),
    runIdx.map((i) => info[i]!.layer.order),
    mode,
  ).map((k) => runIdx[k]!);
  const rest = info.map((_, i) => i).filter((i) => info[i]!.skip !== null);
  const entry = (i: number, position: number | null): ExecEntry => {
    const x = info[i]!;
    return { layer: x.layer, role: x.role, reason: x.reason, runs: position !== null, skip: x.skip, position, count: x.count };
  };
  return [...seq.map((i, n) => entry(i, n + 1)), ...rest.map((i) => entry(i, null))];
}

/** Orden de ejecución de un documento. */
export function executionOf(doc: Document, mode: LayerOrder): ExecEntry[] {
  return buildExecution(doc.layers, countByLayer(doc), mode);
}

const ROLE_ORDER: LayerRole[] = ['engrave', 'line', 'cut'];

/**
 * Explicación del orden, equivalente a `order_note` del motor pero con nombres de color
 * legibles. `null` si hay menos de dos capas que ejecutar.
 */
export function localOrderNote(entries: readonly ExecEntry[], mode: LayerOrder): string | null {
  const running = entries.filter((e) => e.runs);
  if (running.length < 2) return null;
  const free = running.filter((e) => e.layer.order === null);
  const pinned = running.filter((e) => e.layer.order !== null).sort((a, b) => a.layer.order! - b.layer.order!);

  let note: string;
  if (mode === 'file') {
    note = t('layers.order.note.file');
  } else if (free.length === 0) {
    note = t('layers.order.note.pinnedOnly');
  } else {
    const present = ROLE_ORDER.filter((r) => free.some((e) => e.role === r));
    if (present.length < 2) {
      note = t('layers.order.note.sameKind');
    } else {
      const parts = present.map((r, i) => {
        const n = t(`layers.order.group.${r}`);
        return i === 0 ? t('layers.order.first', { n }) : i === present.length - 1 ? t('layers.order.last', { n }) : t('layers.order.then', { n });
      });
      note = t('layers.order.note.auto', { parts: parts.join(', ') });
    }
    const cuts = free.filter((e) => e.reason !== null);
    if (cuts.length > 0) {
      const list = cuts.map((e) => t('layers.order.cutBecause', { name: e.layer.name || colorName(e.layer.color), why: t(`layers.cutReason.${e.reason!}`) }));
      note += ` (${list.join('; ')})`;
    }
  }
  if (pinned.length > 0) {
    const list = pinned.map((e) => t('layers.order.pinnedAt', { name: e.layer.name || colorName(e.layer.color), n: e.layer.order! }));
    note += `${free.length === 0 ? ': ' : '; '}${t('layers.order.pinnedLabel')} ${list.join(', ')}`;
  }
  return `${note}.`;
}

/** Capas (de las que se ejecutan) que piden la misma posición forzada que `color`, o `null`. */
export function orderConflict(layers: readonly LayerSettings[], color: string, position: number): LayerSettings | null {
  return layers.find((l) => l.order === position && l.color.toLowerCase() !== color.toLowerCase()) ?? null;
}

/**
 * Mueve una capa a otra posición de la ejecución. Cambia el orden del archivo y, si el orden
 * automático no alcanzaría para dejarla ahí (por ejemplo un corte antes de un grabado), fija
 * su posición con `order`. Las demás posiciones fijadas se actualizan para no moverse de sitio.
 * Devuelve el mismo documento si no hay cambio.
 */
export function moveLayerInDoc(doc: Document, mode: LayerOrder, color: string, toIndex: number): Document {
  const counts = countByLayer(doc);
  const entries = buildExecution(doc.layers, counts, mode);
  const running = entries.filter((e) => e.runs);
  const from = running.findIndex((e) => e.layer.color.toLowerCase() === color.toLowerCase());
  if (from < 0 || running.length < 2) return doc;
  const to = Math.min(Math.max(toIndex, 0), running.length - 1);
  if (from === to) return doc;

  const seq = running.map((e) => e.layer);
  const [moved] = seq.splice(from, 1);
  seq.splice(to, 0, moved!);
  const seqColors = seq.map((l) => l.color.toLowerCase());
  const position = new Map(seqColors.map((c, i) => [c, i + 1]));
  const stopped = entries.filter((e) => !e.runs).map((e) => e.layer);

  const build = (pinMoved: boolean): LayerSettings[] =>
    [...seq, ...stopped].map((l) => {
      const c = l.color.toLowerCase();
      if (c === color.toLowerCase()) return pinMoved || l.order !== null ? { ...l, order: to + 1 } : l;
      return l.order !== null && position.has(c) ? { ...l, order: position.get(c)! } : l;
    });

  const matches = (layers: LayerSettings[]) => {
    const got = buildExecution(layers, counts, mode)
      .filter((e) => e.runs)
      .map((e) => e.layer.color.toLowerCase());
    return got.length === seqColors.length && got.every((c, i) => c === seqColors[i]);
  };

  let layers = build(false);
  if (!matches(layers)) layers = build(true);
  return { ...doc, layers };
}
