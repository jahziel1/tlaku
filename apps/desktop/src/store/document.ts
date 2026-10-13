import type { Affine, DesignObject, Document, ImportResult, LayerSettings } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { multiplyAffine, translateAffine } from '@/lib/geometry';
import { makeLayer } from '@/lib/layers';
import { LAYER_PALETTE, paletteLabel } from '@/lib/palette';
import type { SliceCreator } from './types';

export interface HistoryEntry {
  /** Qué se hizo; se muestra en el historial. */
  label: I18nKey;
  /** Documento antes de la acción. */
  doc: Document;
  /** Acciones seguidas con la misma clave y poco tiempo entre sí se funden en un paso. */
  key?: string;
  at: number;
}

export interface DocumentSlice {
  doc: Document;
  filePath: string | null;
  dirty: boolean;
  /** Pasos para deshacer (el último es el más reciente). */
  past: HistoryEntry[];
  future: HistoryEntry[];

  newDocument: () => void;
  /** Sustituye el documento (abrir proyecto). Vacía el historial. */
  replaceDocument: (doc: Document, path: string | null) => void;
  markSaved: (path: string) => void;
  applyImport: (result: ImportResult) => void;

  updateLayer: (color: string, patch: Partial<LayerSettings>) => void;
  /** Crea la capa si no existe y devuelve sus ajustes. */
  ensureLayer: (color: string) => LayerSettings;
  setObjectsLayer: (ids: string[], color: string) => void;
  moveObjects: (ids: string[], dx: number, dy: number, key?: string) => void;
  deleteObjects: (ids: string[]) => void;
  /** Duplica y devuelve los ids nuevos. */
  duplicateObjects: (ids: string[], offset?: number) => string[];
  setObjectFlags: (ids: string[], flags: Partial<Pick<DesignObject, 'locked' | 'hidden'>>) => void;

  /** Aplica `matrix` (en coordenadas de documento) a los objetos no bloqueados. */
  transformObjects: (ids: string[], matrix: Affine, key?: string) => void;
  addObjects: (objects: DesignObject[], label?: I18nKey) => void;
  /** Nuevo orden de capas por color; los colores omitidos conservan su orden relativo al final. */
  reorderLayers: (colors: string[]) => void;
  /**
   * Cambio arbitrario con deshacer, para operaciones propias de cada área (agrupar, booleanas...).
   * `change` debe devolver el mismo objeto si no hay cambios.
   */
  applyDocChange: (label: I18nKey, change: (doc: Document) => Document, key?: string) => void;
  undo: () => void;
  redo: () => void;
  canUndo: () => boolean;
  canRedo: () => boolean;
}

export const HISTORY_LIMIT = 200;
const COALESCE_MS = 800;

export function emptyDocument(): Document {
  return { version: 1, objects: [], layers: [], device_id: null };
}

function paletteName(color: string): string {
  const entry = LAYER_PALETTE.find((p) => p.color.toLowerCase() === color.toLowerCase());
  return entry ? paletteLabel(entry) : color;
}

export function uniqueId(base: string, taken: Set<string>): string {
  let i = 1;
  let id = `${base}-${i}`;
  while (taken.has(id)) id = `${base}-${++i}`;
  return id;
}

export const createDocumentSlice: SliceCreator<DocumentSlice> = (set, get) => {
  /** Aplica un cambio al documento y lo registra en el historial. */
  const commit = (label: I18nKey, change: (doc: Document) => Document, key?: string): void => {
    const { doc, past } = get();
    const next = change(doc);
    if (next === doc) return;
    const now = Date.now();
    const last = past[past.length - 1];
    const merge = key !== undefined && last?.key === key && now - last.at < COALESCE_MS;
    const entry: HistoryEntry = merge ? { ...last, at: now } : { label, doc, key, at: now };
    const trimmed = merge ? past.slice(0, -1) : past.slice(-(HISTORY_LIMIT - 1));
    set({ doc: next, dirty: true, past: [...trimmed, entry], future: [] });
  };

  return {
    doc: emptyDocument(),
    filePath: null,
    dirty: false,
    past: [],
    future: [],

    newDocument: () => {
      set({ doc: { ...emptyDocument(), device_id: get().doc.device_id }, filePath: null, dirty: false, past: [], future: [] });
      get().clearSelection();
    },

    replaceDocument: (doc, path) => {
      set({ doc, filePath: path, dirty: false, past: [], future: [] });
      get().clearSelection();
    },

    markSaved: (path) => set({ filePath: path, dirty: false }),

    applyImport: (result) =>
      commit('history.import', (doc) => {
        const known = new Set(doc.layers.map((l) => l.color.toLowerCase()));
        const taken = new Set(doc.objects.map((o) => o.id));
        const objects = result.objects.map((o) => {
          const id = taken.has(o.id) ? uniqueId(o.id, taken) : o.id;
          taken.add(id);
          return { ...o, id };
        });
        const newLayers = result.new_layers.filter((l) => !known.has(l.color.toLowerCase()));
        // Una capa usada por los objetos pero ausente de `new_layers` se crea con valores por defecto.
        const extra = [...new Set(objects.map((o) => o.layer))]
          .filter((c) => !known.has(c.toLowerCase()) && !newLayers.some((l) => l.color.toLowerCase() === c.toLowerCase()))
          .map((c) => makeLayer(c, paletteName(c)));
        return { ...doc, objects: [...doc.objects, ...objects], layers: [...doc.layers, ...newLayers, ...extra] };
      }),

    updateLayer: (color, patch) =>
      commit(
        'history.editLayer',
        (doc) => ({ ...doc, layers: doc.layers.map((l) => (l.color === color ? { ...l, ...patch } : l)) }),
        `layer:${color}:${Object.keys(patch).join(',')}`,
      ),

    ensureLayer: (color) => {
      const existing = get().doc.layers.find((l) => l.color.toLowerCase() === color.toLowerCase());
      if (existing) return existing;
      const layer = makeLayer(color, paletteName(color));
      commit('history.addLayer', (doc) => ({ ...doc, layers: [...doc.layers, layer] }));
      return layer;
    },

    setObjectsLayer: (ids, color) => {
      if (ids.length === 0) return;
      get().ensureLayer(color);
      const set_ = new Set(ids);
      commit('history.changeLayer', (doc) => ({
        ...doc,
        objects: doc.objects.map((o) => (set_.has(o.id) && !o.locked ? { ...o, layer: color } : o)),
      }));
    },

    moveObjects: (ids, dx, dy, key) => {
      if (ids.length === 0 || (dx === 0 && dy === 0)) return;
      const set_ = new Set(ids);
      commit(
        'history.move',
        (doc) => ({
          ...doc,
          objects: doc.objects.map((o) =>
            set_.has(o.id) && !o.locked ? { ...o, transform: translateAffine(o.transform, dx, dy) } : o,
          ),
        }),
        key,
      );
    },

    deleteObjects: (ids) => {
      if (ids.length === 0) return;
      const set_ = new Set(ids);
      commit('history.delete', (doc) => ({ ...doc, objects: doc.objects.filter((o) => !set_.has(o.id) || o.locked) }));
      get().pruneSelection();
    },

    duplicateObjects: (ids, offset = 5) => {
      const set_ = new Set(ids);
      const taken = new Set(get().doc.objects.map((o) => o.id));
      const created: string[] = [];
      commit('history.duplicate', (doc) => {
        const copies = doc.objects
          .filter((o) => set_.has(o.id))
          .map((o) => {
            const id = uniqueId(o.id.replace(/-\d+$/, ''), taken);
            taken.add(id);
            created.push(id);
            return { ...o, id, locked: false, transform: translateAffine(o.transform, offset, offset) };
          });
        return copies.length ? { ...doc, objects: [...doc.objects, ...copies] } : doc;
      });
      return created;
    },

    setObjectFlags: (ids, flags) => {
      const set_ = new Set(ids);
      commit('history.editObjects', (doc) => ({
        ...doc,
        objects: doc.objects.map((o) => (set_.has(o.id) ? { ...o, ...flags } : o)),
      }));
    },

    transformObjects: (ids, matrix, key) => {
      if (ids.length === 0) return;
      const set_ = new Set(ids);
      commit(
        'history.transform',
        (doc) => ({
          ...doc,
          objects: doc.objects.map((o) =>
            set_.has(o.id) && !o.locked ? { ...o, transform: multiplyAffine(matrix, o.transform) } : o,
          ),
        }),
        key,
      );
    },

    addObjects: (objects, label = 'history.add') => {
      if (objects.length === 0) return;
      for (const color of new Set(objects.map((o) => o.layer))) get().ensureLayer(color);
      commit(label, (doc) => ({ ...doc, objects: [...doc.objects, ...objects] }));
    },

    reorderLayers: (colors) => {
      const rank = new Map(colors.map((c, i) => [c.toLowerCase(), i]));
      commit('history.reorderLayers', (doc) => {
        const layers = [...doc.layers].sort(
          (a, b) => (rank.get(a.color.toLowerCase()) ?? Infinity) - (rank.get(b.color.toLowerCase()) ?? Infinity),
        );
        return layers.every((l, i) => l === doc.layers[i]) ? doc : { ...doc, layers };
      });
    },

    applyDocChange: (label, change, key) => commit(label, change, key),

    undo: () => {
      const { past, future, doc } = get();
      const last = past[past.length - 1];
      if (!last) return;
      set({ doc: last.doc, dirty: true, past: past.slice(0, -1), future: [{ ...last, doc }, ...future] });
      get().pruneSelection();
    },

    redo: () => {
      const { past, future, doc } = get();
      const next = future[0];
      if (!next) return;
      set({ doc: next.doc, dirty: true, past: [...past, { ...next, doc }], future: future.slice(1) });
      get().pruneSelection();
    },

    canUndo: () => get().past.length > 0,
    canRedo: () => get().future.length > 0,
  };
};
