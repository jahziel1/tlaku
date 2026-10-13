import type { Document, LayerSettings } from '@/api/types';
import { normalizeLayer, type NestedKey } from '@/lib/layers';
import { useStore } from '@/store';

/**
 * El editor de capa trabaja sobre una "diana": la capa principal (`sub = null`) o una de sus
 * operaciones adicionales (`sub` = posición en `sub_layers`). Así la misma interfaz edita ambas.
 */
export interface LayerTarget {
  color: string;
  sub: number | null;
}

/**
 * Documento con la diana transformada por `fn`. Si la capa o la operación ya no existen devuelve el
 * mismo documento. Una capa vieja se completa con los valores por defecto antes de tocarla.
 */
export function mapTarget(doc: Document, { color, sub }: LayerTarget, fn: (l: LayerSettings) => LayerSettings): Document {
  let changed = false;
  const layers = doc.layers.map((l) => {
    if (l.color !== color) return l;
    const main = normalizeLayer(l);
    if (sub === null) {
      changed = true;
      return fn(main);
    }
    const target = main.sub_layers[sub];
    if (!target) return l;
    changed = true;
    return { ...main, sub_layers: main.sub_layers.map((s, i) => (i === sub ? fn(s) : s)) };
  });
  return changed ? { ...doc, layers } : doc;
}

export type SetPatch = (patch: Partial<LayerSettings>) => void;
export type SetIn = <G extends NestedKey, K extends keyof LayerSettings[G]>(group: G, key: K, value: LayerSettings[G][K]) => void;

/**
 * Funciones para cambiar la diana. Cada campo lleva su propia clave de deshacer, así que escribir
 * varias veces en el mismo campo es un solo paso y campos distintos son pasos distintos.
 */
export function useLayerSetters(target: LayerTarget): { set: SetPatch; setIn: SetIn } {
  const applyDocChange = useStore((s) => s.applyDocChange);
  const { color, sub } = target;
  const prefix = `layer:${color}${sub === null ? '' : `:sub${sub}`}`;
  const set: SetPatch = (patch) =>
    applyDocChange('history.editLayer', (doc) => mapTarget(doc, target, (l) => ({ ...l, ...patch })), `${prefix}:${Object.keys(patch).join(',')}`);
  const setIn: SetIn = (group, key, value) =>
    applyDocChange(
      'history.editLayer',
      (doc) => mapTarget(doc, target, (l) => ({ ...l, [group]: { ...l[group], [key]: value } })),
      `${prefix}:${group}.${String(key)}`,
    );
  return { set, setIn };
}
