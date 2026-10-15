import type { DesignObject, Document } from '@/api/types';

/**
 * Comparación simple entre una versión del historial y el documento actual: qué objetos y capas
 * cambiaron. Compara por contenido (JSON canónico): un `null` y un campo ausente cuentan igual, y
 * el orden de las claves no importa.
 */

export interface DocDiff {
  /** Objetos que están en el documento actual y no en la versión (restaurar los perdería). */
  added: DesignObject[];
  /** Objetos que estaban en la versión y ya no están (restaurar los devolvería). */
  removed: DesignObject[];
  /** Objetos en ambos, con contenido distinto (los del documento ACTUAL). */
  changed: DesignObject[];
  /** Colores de capa (`#rrggbb`) que solo están en el actual, solo en la versión, o cambiaron. */
  layersAdded: string[];
  layersRemoved: string[];
  layersChanged: string[];
  /** Nombre visible de una capa (`color` si no tiene). */
  layerNames: Record<string, string>;
  notesChanged: boolean;
  guidesChanged: boolean;
  deviceChanged: boolean;
  same: boolean;
}

function canonical(value: unknown): string {
  return JSON.stringify(value, (_key, v: unknown) => {
    if (v === null || v === undefined) return undefined;
    if (Array.isArray(v) || typeof v !== 'object') return v;
    return Object.fromEntries(Object.entries(v as Record<string, unknown>).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
  });
}

const sameValue = (a: unknown, b: unknown): boolean => a === b || canonical(a) === canonical(b);

/** Qué cambió entre la versión `version` y el documento `current`. Ambos deben estar normalizados igual (`normalizeLayers`). */
export function diffDocuments(version: Document, current: Document): DocDiff {
  const before = new Map(version.objects.map((o) => [o.id, o]));
  const now = new Map(current.objects.map((o) => [o.id, o]));
  const added = current.objects.filter((o) => !before.has(o.id));
  const removed = version.objects.filter((o) => !now.has(o.id));
  const changed = current.objects.filter((o) => {
    const old = before.get(o.id);
    return old !== undefined && !sameValue(old, o);
  });

  const layersBefore = new Map(version.layers.map((l) => [l.color, l]));
  const layersNow = new Map(current.layers.map((l) => [l.color, l]));
  const layersAdded = current.layers.filter((l) => !layersBefore.has(l.color)).map((l) => l.color);
  const layersRemoved = version.layers.filter((l) => !layersNow.has(l.color)).map((l) => l.color);
  const layersChanged = current.layers
    .filter((l) => {
      const old = layersBefore.get(l.color);
      return old !== undefined && !sameValue(old, l);
    })
    .map((l) => l.color);
  const layerNames: Record<string, string> = {};
  for (const l of [...version.layers, ...current.layers]) layerNames[l.color] = l.name?.trim() || l.color;

  const notesChanged = (version.notes ?? '') !== (current.notes ?? '');
  const guidesChanged = !sameValue(version.guides ?? [], current.guides ?? []);
  const deviceChanged = (version.device_id ?? null) !== (current.device_id ?? null);
  const same =
    added.length + removed.length + changed.length + layersAdded.length + layersRemoved.length + layersChanged.length === 0 &&
    !notesChanged &&
    !guidesChanged &&
    !deviceChanged;
  return { added, removed, changed, layersAdded, layersRemoved, layersChanged, layerNames, notesChanged, guidesChanged, deviceChanged, same };
}

/** Ids de los objetos del documento actual que difieren de la versión (para resaltarlos en el lienzo). */
export function highlightIds(diff: DocDiff): string[] {
  return [...diff.added, ...diff.changed].map((o) => o.id);
}
