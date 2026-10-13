import type { DesignObject, Document } from '@/api/types';
import type { I18nKey } from '@/i18n';
import type { AppState } from '@/store';

/**
 * Propiedades de corte por objeto: prioridad (menor = antes, solo dentro de su capa) y escala de
 * potencia (% de la potencia de la capa). `null` significa automática / 100 %.
 */
export type CutPatch = { priority?: number | null; power_scale?: number | null };

export const PRIORITY_LIMIT = 9999;

/** Prioridad válida: entero acotado, o `null` (automática) si no es un número. */
export function normalizePriority(v: number | null): number | null {
  if (v === null || !Number.isFinite(v)) return null;
  return Math.max(-PRIORITY_LIMIT, Math.min(PRIORITY_LIMIT, Math.round(v)));
}

/** Escala de potencia válida (0 a 100 %); 100 equivale a "sin escala" y se guarda como `null`. */
export function normalizePowerScale(v: number | null): number | null {
  if (v === null || !Number.isFinite(v)) return null;
  const pct = Math.max(0, Math.min(100, Math.round(v * 100) / 100));
  return pct >= 100 ? null : pct;
}

/** Valor común de `get` en `objects`, `'mixed'` si difieren. Sin objetos, `null`. */
export function commonValue(objects: readonly DesignObject[], get: (o: DesignObject) => number | null | undefined): number | null | 'mixed' {
  let first = true;
  let value: number | null = null;
  for (const o of objects) {
    const v = get(o) ?? null;
    if (first) {
      value = v;
      first = false;
    } else if (v !== value) {
      return 'mixed';
    }
  }
  return value;
}

/** Aplica `patch` a los objetos `ids`; devuelve el mismo documento si no cambia nada. */
export function patchCutProps(doc: Document, ids: readonly string[], patch: CutPatch): Document {
  const set = new Set(ids);
  const priority = 'priority' in patch ? normalizePriority(patch.priority ?? null) : undefined;
  const power = 'power_scale' in patch ? normalizePowerScale(patch.power_scale ?? null) : undefined;
  let changed = false;
  const objects = doc.objects.map((o) => {
    if (!set.has(o.id)) return o;
    const nextP = priority === undefined ? (o.priority ?? null) : priority;
    const nextS = power === undefined ? (o.power_scale ?? null) : power;
    if (nextP === (o.priority ?? null) && nextS === (o.power_scale ?? null)) return o;
    changed = true;
    // Se omiten los campos vacíos: el archivo solo guarda lo que el usuario fijó.
    const { priority: _p, power_scale: _s, ...rest } = o;
    return { ...rest, ...(nextP === null ? {} : { priority: nextP }), ...(nextS === null ? {} : { power_scale: nextS }) };
  });
  return changed ? { ...doc, objects } : doc;
}

/** Suma `delta` a la prioridad de cada objeto (automática cuenta como 0). */
export function stepPriorityDoc(doc: Document, ids: readonly string[], delta: number): Document {
  const set = new Set(ids);
  let changed = false;
  const objects = doc.objects.map((o) => {
    if (!set.has(o.id)) return o;
    const next = normalizePriority((o.priority ?? 0) + delta);
    if (next === (o.priority ?? null)) return o;
    changed = true;
    return { ...o, priority: next };
  });
  return changed ? { ...doc, objects } : doc;
}

const HISTORY: Record<keyof CutPatch, I18nKey> = {
  priority: 'canvas.history.priority',
  power_scale: 'canvas.history.powerScale',
};

/** Fija prioridad y/o escala de potencia de los objetos `ids` con un paso de deshacer. */
export function setCutProps(s: Pick<AppState, 'applyDocChange'>, ids: readonly string[], patch: CutPatch): void {
  if (ids.length === 0) return;
  const field = (Object.keys(patch) as (keyof CutPatch)[])[0];
  if (!field) return;
  s.applyDocChange(HISTORY[field], (doc) => patchCutProps(doc, ids, patch), `cut:${field}`);
}

/** Sube (`delta` negativo = corta antes) o baja la prioridad de los objetos `ids`. */
export function stepPriority(s: Pick<AppState, 'applyDocChange'>, ids: readonly string[], delta: number): void {
  if (ids.length === 0 || delta === 0) return;
  s.applyDocChange('canvas.history.priority', (doc) => stepPriorityDoc(doc, ids, delta), 'cut:priority');
}
