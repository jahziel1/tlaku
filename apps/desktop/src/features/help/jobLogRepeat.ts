import { getApi } from '@/api';
import type { LayerSettings } from '@/api/types';
import { askChoice } from '@/features/history/prompt';
import { openProject } from '@/features/history/files';
import { baseName } from '@/features/history/paths';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { findTabByPath } from '@/store/tabs';
import { LAYER_LOG_FIELDS, type JobLayerLog, type JobLogEntry, type LayerLogField } from './jobLog';

/**
 * «Repetir con estos ajustes»: lleva a las capas del documento los ajustes con que salió un trabajo del historial.
 * Solo cambia el diseño (con un paso de deshacer y después de confirmar): enviar a la máquina sigue siendo cosa
 * del usuario, con su diálogo de inicio, su lista de comprobación y el preflight de siempre.
 */

export interface LayerChange {
  color: string;
  name: string;
  field: LayerLogField;
  from: number | boolean | undefined;
  to: number | boolean;
}

export interface RepeatPlan {
  changes: LayerChange[];
  /** Capas del trabajo que el documento ya no tiene (se identifican por color). */
  missing: JobLayerLog[];
}

const same = (a: unknown, b: unknown) => (typeof a === 'number' && typeof b === 'number' ? Math.abs(a - b) < 1e-9 : a === b);

/** Qué ajustes cambiarían en `layers` al repetir el trabajo `entry`. */
export function planRepeat(layers: readonly LayerSettings[], entry: Pick<JobLogEntry, 'layers'>): RepeatPlan {
  const byColor = new Map(layers.map((l) => [l.color.toLowerCase(), l]));
  const changes: LayerChange[] = [];
  const missing: JobLayerLog[] = [];
  for (const saved of entry.layers ?? []) {
    const layer = byColor.get(saved.color.toLowerCase());
    if (!layer) {
      missing.push(saved);
      continue;
    }
    const cur = layer as unknown as Record<string, unknown>;
    for (const field of LAYER_LOG_FIELDS) {
      const to = saved.settings[field];
      if (to === undefined || same(cur[field], to)) continue;
      changes.push({ color: layer.color, name: layer.name || layer.color, field, from: cur[field] as number | boolean | undefined, to });
    }
  }
  return { changes, missing };
}

/** Capas con los ajustes del trabajo aplicados (las mismas referencias si nada cambia). */
export function applyRepeat(layers: readonly LayerSettings[], entry: Pick<JobLogEntry, 'layers'>): LayerSettings[] {
  const saved = new Map((entry.layers ?? []).map((l) => [l.color.toLowerCase(), l]));
  return layers.map((layer) => {
    const s = saved.get(layer.color.toLowerCase());
    if (!s) return layer;
    const patch: Record<string, number | boolean> = {};
    const cur = layer as unknown as Record<string, unknown>;
    for (const field of LAYER_LOG_FIELDS) {
      const to = s.settings[field];
      if (to !== undefined && !same(cur[field], to)) patch[field] = to;
    }
    return Object.keys(patch).length > 0 ? { ...layer, ...patch } : layer;
  });
}

const FIELD_KEYS = {
  speed: 'help.jobLog.field.speed',
  power: 'help.jobLog.field.power',
  min_power: 'help.jobLog.field.minPower',
  passes: 'help.jobLog.field.passes',
  interval: 'help.jobLog.field.interval',
  angle: 'help.jobLog.field.angle',
  crosshatch: 'help.jobLog.field.crosshatch',
  bidirectional: 'help.jobLog.field.bidirectional',
  air: 'help.jobLog.field.air',
  kerf: 'help.jobLog.field.kerf',
} as const satisfies Record<LayerLogField, string>;

/** Nombre del ajuste en el idioma de la interfaz. */
export const fieldLabel = (f: LayerLogField): string => t(FIELD_KEYS[f]);

/** Valor de un ajuste para mostrarlo («1200», «60», «sí»). */
export function fieldValue(v: number | boolean | undefined): string {
  if (v === undefined) return '—';
  if (typeof v === 'boolean') return t(v ? 'help.jobLog.yes' : 'help.jobLog.no');
  return fmt(v, Math.abs(v) >= 100 ? 0 : 2);
}

/** Una línea por cambio, para el cuadro de confirmación. */
export function changeLines(changes: readonly LayerChange[]): string[] {
  return changes.map((c) => t('help.jobLog.repeat.line', { layer: c.name, field: fieldLabel(c.field), from: fieldValue(c.from), to: fieldValue(c.to) }));
}

/** Ajustes aplicados a la pestaña activa. Devuelve si cambió algo. */
function applyToActive(entry: JobLogEntry): boolean {
  const s = useStore.getState();
  const plan = planRepeat(s.doc.layers, entry);
  if (plan.changes.length === 0) return false;
  s.applyDocChange('help.jobLog.repeat.history', (doc) => ({ ...doc, layers: applyRepeat(doc.layers, entry) as LayerSettings[] }));
  return true;
}

/**
 * Abre el documento del trabajo (si existe; si ya está abierto, activa su pestaña) y, tras confirmar, le aplica los
 * ajustes guardados. Sin documento (no estaba guardado o ya no está) ofrece aplicarlos al de la pestaña activa.
 * Nunca envía nada a la máquina. Devuelve si se aplicó algún cambio.
 */
export async function repeatWithSettings(entry: JobLogEntry): Promise<boolean> {
  const s = () => useStore.getState();
  if (!entry.layers || entry.layers.length === 0) {
    s().notify('warning', t('help.jobLog.repeat.noLayers'));
    return false;
  }
  const path = entry.documentPath ?? null;
  let usingActive = false;
  if (path && (findTabByPath(s(), path) !== null || (await getApi().pathsExist({ paths: [path] }))[0])) {
    if (!(await openProject(path))) return false;
  } else {
    const choice = await askChoice({
      title: t('help.jobLog.repeat.noDoc.title'),
      message: path ? t('help.jobLog.repeat.noDoc.missing', { name: baseName(path) }) : t('help.jobLog.repeat.noDoc.unsaved'),
      choices: [
        { id: 'active', label: t('help.jobLog.repeat.noDoc.useActive'), tone: 'accent' },
        { id: 'cancel', label: t('help.jobLog.repeat.cancel') },
      ],
      cancelId: 'cancel',
    });
    if (choice !== 'active') return false;
    usingActive = true;
  }
  // Mientras se espera la respuesta el usuario puede cambiar de pestaña: el cambio va a la que se abrió.
  const tabId = s().activeTabId;
  const plan = planRepeat(s().doc.layers, entry);
  if (plan.changes.length === 0) {
    s().notify('info', t('help.jobLog.repeat.nothing'));
    return false;
  }
  const items = changeLines(plan.changes);
  if (plan.missing.length > 0) items.push(t('help.jobLog.repeat.missing', { layers: plan.missing.map((l) => l.name || l.color).join(', ') }));
  const choice = await askChoice({
    title: t('help.jobLog.repeat.title'),
    message: t(usingActive ? 'help.jobLog.repeat.messageActive' : 'help.jobLog.repeat.message', { doc: baseName(path ?? '') || t('help.jobLog.untitled'), n: plan.changes.length }),
    items,
    choices: [
      { id: 'apply', label: t('help.jobLog.repeat.apply'), tone: 'accent' },
      { id: 'cancel', label: t('help.jobLog.repeat.cancel') },
    ],
    cancelId: 'cancel',
  });
  if (choice !== 'apply') return false;
  const changed = s().runInTab(tabId, () => {
    const did = applyToActive(entry);
    if (did) void s().computePreview();
    return did;
  });
  if (changed) s().notify('success', t('help.jobLog.repeat.done'));
  return changed === true;
}
