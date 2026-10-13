import type { Affine, DesignObject, Document, ImportResult, LayerSettings } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { bezPointCount } from '@/lib/bezpath';
import { multiplyAffine, transformKind, translateAffine } from '@/lib/geometry';
import { groupIdsOf, regroupCopies } from '@/lib/groups';
import { reconcileShapes } from '@/lib/paramShape';
import { makeLayer, normalizeLayers } from '@/lib/layers';
import { LAYER_PALETTE, paletteLabel } from '@/lib/palette';
import type { SliceCreator } from './types';

export interface HistoryEntry {
  /** Qué se hizo; se muestra en el historial. */
  label: I18nKey;
  /**
   * En `past`: documento antes de la acción (lo que restaura deshacer).
   * En `future`: documento después de la acción (lo que restaura rehacer).
   */
  doc: Document;
  /** Acciones seguidas con la misma clave y poco tiempo entre sí se funden en un paso. */
  key?: string;
  at: number;
  /**
   * Puntos de geometría que este paso mantiene vivos y que el documento actual ya no usa
   * (ver `entryCost`). Sirve para acotar la memoria del historial.
   */
  cost?: number;
}

/** Portapapeles interno: objetos copiados y los ajustes de las capas que usan. */
export interface ClipboardData {
  objects: DesignObject[];
  layers: LayerSettings[];
  /** Cuántas veces se ha pegado desplazado; cada pegado se corre un poco más. */
  pastes: number;
}

export interface DocumentSlice {
  doc: Document;
  filePath: string | null;
  /** Hay cambios respecto a lo guardado (se calcula por referencia con `savedDoc`). */
  dirty: boolean;
  /** Documento tal como se guardó o abrió por última vez. */
  savedDoc: Document;
  clipboard: ClipboardData | null;
  /** Pasos para deshacer (el último es el más reciente). */
  past: HistoryEntry[];
  future: HistoryEntry[];

  newDocument: () => void;
  /** Sustituye el documento (abrir proyecto). Vacía el historial. */
  replaceDocument: (doc: Document, path: string | null) => void;
  /** `saved` es el documento que se escribió (por defecto el actual): si cambió durante el guardado, sigue sucio. */
  markSaved: (path: string, saved?: Document) => void;
  /** Agrega lo importado al documento y devuelve los ids finales (pueden cambiar si chocaban). */
  applyImport: (result: ImportResult) => string[];

  updateLayer: (color: string, patch: Partial<LayerSettings>) => void;
  /** Crea la capa si no existe y devuelve sus ajustes. */
  ensureLayer: (color: string) => LayerSettings;
  setObjectsLayer: (ids: string[], color: string) => void;
  moveObjects: (ids: string[], dx: number, dy: number, key?: string) => void;
  deleteObjects: (ids: string[]) => void;
  /** Duplica y devuelve los ids nuevos. */
  duplicateObjects: (ids: string[], offset?: number) => string[];
  /** Copia al portapapeles interno. Devuelve cuántos objetos se copiaron. */
  copyObjects: (ids: string[]) => number;
  /** Copia y elimina (los bloqueados no se cortan). Devuelve cuántos objetos se cortaron. */
  cutObjects: (ids: string[]) => number;
  /** Pega el portapapeles (desplazado, o en el mismo lugar con `inPlace`) y devuelve los ids nuevos. */
  pasteObjects: (opts?: { inPlace?: boolean }) => string[];
  setObjectFlags: (ids: string[], flags: Partial<Pick<DesignObject, 'locked' | 'hidden'>>) => void;

  /**
   * Aplica `matrix` (en coordenadas de documento) a los objetos no bloqueados. Respeta los
   * interruptores de transformación (mover, escalar, girar). `label` nombra el paso en el historial.
   */
  transformObjects: (ids: string[], matrix: Affine, key?: string, label?: I18nKey) => void;
  addObjects: (objects: DesignObject[], label?: I18nKey) => void;
  /** Nuevo orden de capas por color; los colores omitidos conservan su orden relativo al final. */
  reorderLayers: (colors: string[]) => void;
  /**
   * Cambio arbitrario con deshacer, para operaciones propias de cada área (agrupar, booleanas...).
   * `change` debe devolver el mismo objeto si no hay cambios.
   */
  applyDocChange: (label: I18nKey, change: (doc: Document) => Document, key?: string) => void;
  /**
   * Cambio SIN paso de deshacer y sin marcar `dirty`, para datos derivados que se regeneran solos
   * (el contorno de un texto). Aplica el mismo `change` al documento actual, a `savedDoc` y a los
   * documentos de `past` y `future`, de modo que deshacer, rehacer y guardar siguen coherentes.
   * `change` se llama con cada documento distinto (una sola vez por documento) y debe devolver el
   * mismo objeto si no hay nada que cambiar; si quiere compartir los objetos nuevos entre pasos,
   * que los memorice él.
   */
  patchSilently: (change: (doc: Document) => Document) => void;
  undo: () => void;
  redo: () => void;
  /**
   * Salta a un punto del historial: 0 = estado inicial, `past.length` = estado actual y
   * `past.length + future.length` = el último paso rehacible.
   */
  jumpTo: (position: number) => void;
  /** Olvida todos los pasos de deshacer y rehacer (libera memoria); el documento no cambia. */
  clearHistory: () => void;
  canUndo: () => boolean;
  canRedo: () => boolean;
}

export const HISTORY_LIMIT = 200;
/**
 * Tope de memoria del historial, en puntos de geometría retenidos solo por los pasos.
 * Cada paso guarda el documento completo, pero con datos inmutables los objetos que no cambian se
 * comparten por referencia entre pasos: mover, cambiar de capa o ajustar una capa no copia
 * trayectorias. Lo que sí pesa es la geometría que una acción reemplaza o elimina (booleanas,
 * recortes, borrar un diseño denso). Un punto [x, y] ocupa unas decenas de bytes en el motor
 * de JS, así que 2 millones de puntos son del orden de 100 MB.
 */
export const HISTORY_POINT_BUDGET = 2_000_000;
const COALESCE_MS = 800;
/** Desplazamiento de fábrica en mm de cada pegado y de duplicar; el que rige es la preferencia `duplicateOffset`. */
export const PASTE_OFFSET = 5;

const pointsCache = new WeakMap<object, number>();

/** Puntos de geometría de un objeto (las imágenes solo guardan la ruta: no pesan). */
function kindPoints(kind: DesignObject['kind']): number {
  let n = pointsCache.get(kind);
  if (n === undefined) {
    const paths = kind.type === 'vector' ? kind.paths : kind.type === 'text' ? kind.outline : [];
    n = paths.reduce((sum, p) => sum + bezPointCount(p), 0);
    pointsCache.set(kind, n);
  }
  return n;
}

/**
 * Memoria que mantiene viva `before` y que `after` ya no usa: puntos de los objetos de `before`
 * cuya geometría no aparece en `after`, más un pequeño costo por la lista de referencias.
 * Es O(objetos) y no recorre trayectorias que se comparten.
 */
export function entryCost(before: Document, after: Document): number {
  const afterSet = new Set(after.objects);
  const beforeSet = new Set(before.objects);
  const keptKinds = new Set<object>();
  for (const o of after.objects) if (!beforeSet.has(o)) keptKinds.add(o.kind);
  let points = 0;
  for (const o of before.objects) {
    if (afterSet.has(o) || keptKinds.has(o.kind)) continue;
    points += kindPoints(o.kind);
  }
  return points + before.objects.length / 8;
}

/** Recorta por cantidad de pasos y por presupuesto de memoria; siempre conserva el paso más reciente. */
export function trimHistory(past: HistoryEntry[], limit = HISTORY_LIMIT, budget = HISTORY_POINT_BUDGET): HistoryEntry[] {
  let keepFrom = Math.max(0, past.length - limit);
  let acc = 0;
  for (let i = past.length - 1; i >= keepFrom; i--) {
    acc += past[i]!.cost ?? 0;
    if (acc > budget && i < past.length - 1) {
      keepFrom = i + 1;
      break;
    }
  }
  return keepFrom === 0 ? past : past.slice(keepFrom);
}

const round6 = (n: number) => Math.round(n * 1e6) / 1e6;
/** Evita que pasos de 0,1 mm acumulen ruido de coma flotante (10,299999...). */
const roundMove = (m: Affine): Affine => [m[0], m[1], m[2], m[3], round6(m[4]), round6(m[5])];

export function emptyDocument(): Document {
  return { version: 2, objects: [], layers: [], device_id: null };
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
  const initialDoc = emptyDocument();

  /** Aplica un cambio al documento y lo registra en el historial. */
  const commit = (label: I18nKey, change: (doc: Document) => Document, key?: string): void => {
    const { doc, past, savedDoc } = get();
    // Las formas paramétricas conservan sus parámetros coherentes con el contorno (ver `lib/paramShape`).
    const next = reconcileShapes(doc, change(doc));
    if (next === doc) return;
    const now = Date.now();
    const last = past[past.length - 1];
    const merge = key !== undefined && last?.key === key && now - last.at < COALESCE_MS;
    // Al fundir pasos se conserva el documento de antes del primero; solo se recalcula el costo.
    const base = merge ? last! : { label, doc, key, at: now };
    const entry: HistoryEntry = { ...base, at: now, cost: entryCost(base.doc, next) };
    const kept = trimHistory([...(merge ? past.slice(0, -1) : past), entry]);
    set({ doc: next, dirty: next !== savedDoc, past: kept, future: [] });
  };

  /** Nuevo documento base: lo guardado, el historial y la selección empiezan de cero. */
  const reset = (doc: Document, filePath: string | null): void => {
    set({ doc, savedDoc: doc, filePath, dirty: false, past: [], future: [] });
    get().clearSelection();
  };

  /** Intercambia el documento actual por el de un paso (deshacer/rehacer) con su costo al día. */
  const swap = (entry: HistoryEntry, current: Document): HistoryEntry => ({
    ...entry,
    doc: current,
    cost: entryCost(current, entry.doc),
  });

  return {
    doc: initialDoc,
    filePath: null,
    dirty: false,
    savedDoc: initialDoc,
    clipboard: null,
    past: [],
    future: [],

    newDocument: () => reset({ ...emptyDocument(), device_id: get().doc.device_id }, null),

    // Documentos de antes de la fase 4 llegan sin puentes, Z ni estilo de relleno.
    replaceDocument: (doc, path) => reset(normalizeLayers(doc), path),

    markSaved: (path, saved) => {
      const savedDoc = saved ?? get().doc;
      set({ filePath: path, savedDoc, dirty: get().doc !== savedDoc });
    },

    applyImport: (result) => {
      const ids: string[] = [];
      commit('history.import', (doc) => {
        ids.length = 0;
        const known = new Set(doc.layers.map((l) => l.color.toLowerCase()));
        const taken = new Set(doc.objects.map((o) => o.id));
        const objects = result.objects.map((o) => {
          const id = taken.has(o.id) ? uniqueId(o.id, taken) : o.id;
          taken.add(id);
          ids.push(id);
          return { ...o, id };
        });
        const newLayers = result.new_layers.filter((l) => !known.has(l.color.toLowerCase()));
        // Una capa usada por los objetos pero ausente de `new_layers` se crea con valores por defecto.
        const extra = [...new Set(objects.map((o) => o.layer))]
          .filter((c) => !known.has(c.toLowerCase()) && !newLayers.some((l) => l.color.toLowerCase() === c.toLowerCase()))
          .map((c) => makeLayer(c, paletteName(c)));
        return { ...doc, objects: [...doc.objects, ...objects], layers: [...doc.layers, ...newLayers, ...extra] };
      });
      return ids;
    },

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
      if (!get().transformAllow.move) return void get().notifyKey('info', 'canvas.xform.off.move');
      const set_ = new Set(ids);
      commit(
        'history.move',
        (doc) => ({
          ...doc,
          objects: doc.objects.map((o) =>
            set_.has(o.id) && !o.locked ? { ...o, transform: roundMove(translateAffine(o.transform, dx, dy)) } : o,
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

    duplicateObjects: (ids, offset = get().prefs.duplicateOffset) => {
      const set_ = new Set(ids);
      const taken = new Set(get().doc.objects.map((o) => o.id));
      const created: string[] = [];
      commit('history.duplicate', (doc) => {
        const copies = regroupCopies(
          doc.objects
            .filter((o) => set_.has(o.id))
            .map((o) => {
              const id = uniqueId(o.id.replace(/-\d+$/, ''), taken);
              taken.add(id);
              created.push(id);
              return { ...o, id, locked: false, transform: translateAffine(o.transform, offset, offset) };
            }),
          groupIdsOf(doc.objects),
        );
        return copies.length ? { ...doc, objects: [...doc.objects, ...copies] } : doc;
      });
      return created;
    },

    copyObjects: (ids) => {
      const set_ = new Set(ids);
      const { doc } = get();
      const objects = doc.objects.filter((o) => set_.has(o.id));
      if (objects.length === 0) return 0;
      const used = new Set(objects.map((o) => o.layer.toLowerCase()));
      const layers = doc.layers.filter((l) => used.has(l.color.toLowerCase()));
      set({ clipboard: { objects, layers, pastes: 0 } });
      return objects.length;
    },

    cutObjects: (ids) => {
      const movable = new Set(get().doc.objects.filter((o) => !o.locked).map((o) => o.id));
      const cuttable = ids.filter((id) => movable.has(id));
      const n = get().copyObjects(cuttable);
      if (n === 0) return 0;
      // Al cortar, el primer pegado cae donde estaba el original.
      set((s) => ({ clipboard: s.clipboard && { ...s.clipboard, pastes: -1 } }));
      commit('history.cut', (doc) => {
        const gone = new Set(cuttable);
        return { ...doc, objects: doc.objects.filter((o) => !gone.has(o.id)) };
      });
      get().pruneSelection();
      return n;
    },

    pasteObjects: ({ inPlace = false } = {}) => {
      const clip = get().clipboard;
      if (!clip || clip.objects.length === 0) return [];
      const offset = inPlace ? 0 : get().prefs.duplicateOffset * (clip.pastes + 1);
      const created: string[] = [];
      commit('history.paste', (doc) => {
        created.length = 0;
        const taken = new Set(doc.objects.map((o) => o.id));
        const copies = regroupCopies(
          clip.objects.map((o) => {
            const id = uniqueId(o.id.replace(/-\d+$/, ''), taken);
            taken.add(id);
            created.push(id);
            const transform = offset === 0 ? o.transform : roundMove(translateAffine(o.transform, offset, offset));
            return { ...o, id, locked: false, transform };
          }),
          groupIdsOf(doc.objects),
        );
        // Las capas que faltan (otro proyecto, o borradas) se recrean con los ajustes que tenían.
        const have = new Set(doc.layers.map((l) => l.color.toLowerCase()));
        const missing = clip.layers.filter((l) => !have.has(l.color.toLowerCase()));
        const stray = [...new Set(copies.map((o) => o.layer))]
          .filter((c) => !have.has(c.toLowerCase()) && !missing.some((l) => l.color.toLowerCase() === c.toLowerCase()))
          .map((c) => makeLayer(c, paletteName(c)));
        return { ...doc, objects: [...doc.objects, ...copies], layers: [...doc.layers, ...missing, ...stray] };
      });
      if (!inPlace) set((s) => ({ clipboard: s.clipboard && { ...s.clipboard, pastes: s.clipboard.pastes + 1 } }));
      return created;
    },

    setObjectFlags: (ids, flags) => {
      const set_ = new Set(ids);
      commit('history.editObjects', (doc) => ({
        ...doc,
        objects: doc.objects.map((o) => (set_.has(o.id) ? { ...o, ...flags } : o)),
      }));
    },

    transformObjects: (ids, matrix, key, label = 'history.transform') => {
      if (ids.length === 0) return;
      const kind = transformKind(matrix);
      if (!get().transformAllow[kind]) return void get().notifyKey('info', `canvas.xform.off.${kind}`);
      const set_ = new Set(ids);
      commit(
        label,
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

    patchSilently: (change) =>
      set((s) => {
        // Un mismo documento (por ejemplo `savedDoc` que también está en `past`) da el mismo resultado:
        // así la comparación por referencia de `dirty` no se rompe.
        const seen = new Map<Document, Document>();
        const patch = (d: Document): Document => {
          let out = seen.get(d);
          if (!out) {
            out = change(d);
            seen.set(d, out);
          }
          return out;
        };
        const doc = patch(s.doc);
        const savedDoc = patch(s.savedDoc);
        let touched = doc !== s.doc || savedDoc !== s.savedDoc;
        const patchEntries = (list: HistoryEntry[]): HistoryEntry[] => {
          let any = false;
          const out = list.map((e) => {
            const d = patch(e.doc);
            if (d === e.doc) return e;
            any = true;
            return { ...e, doc: d };
          });
          touched ||= any;
          return any ? out : list;
        };
        const past = patchEntries(s.past);
        const future = patchEntries(s.future);
        if (!touched) return s;
        return { doc, savedDoc, past, future, dirty: doc !== savedDoc };
      }),

    undo: () => {
      const { past, future, doc, savedDoc } = get();
      const last = past[past.length - 1];
      if (!last) return;
      set({ doc: last.doc, dirty: last.doc !== savedDoc, past: past.slice(0, -1), future: [swap(last, doc), ...future] });
      get().pruneSelection();
    },

    redo: () => {
      const { past, future, doc, savedDoc } = get();
      const next = future[0];
      if (!next) return;
      set({ doc: next.doc, dirty: next.doc !== savedDoc, past: trimHistory([...past, swap(next, doc)]), future: future.slice(1) });
      get().pruneSelection();
    },

    jumpTo: (position) => {
      const { past, future, doc, savedDoc } = get();
      const target = Math.max(0, Math.min(past.length + future.length, Math.round(position)));
      const now = past.length;
      if (target === now) return;
      if (target < now) {
        // Cada paso deshecho pasa a `future` guardando el documento posterior a él.
        const undone = past.slice(target).map((e, i, arr) => swap(e, i + 1 < arr.length ? arr[i + 1]!.doc : doc));
        const nextDoc = past[target]!.doc;
        set({ doc: nextDoc, dirty: nextDoc !== savedDoc, past: past.slice(0, target), future: [...undone, ...future] });
      } else {
        const k = target - now;
        const redone = future.slice(0, k).map((e, i) => swap(e, i === 0 ? doc : future[i - 1]!.doc));
        const nextDoc = future[k - 1]!.doc;
        set({ doc: nextDoc, dirty: nextDoc !== savedDoc, past: trimHistory([...past, ...redone]), future: future.slice(k) });
      }
      get().pruneSelection();
    },

    clearHistory: () => set({ past: [], future: [] }),

    canUndo: () => get().past.length > 0,
    canRedo: () => get().future.length > 0,
  };
};
