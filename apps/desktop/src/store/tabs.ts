import type { Document, JobOptions, JobPreview } from '@/api/types';
import { t } from '@/i18n';
import { normalizeLayers } from '@/lib/layers';
import { emptyDocument, trimHistory, type HistoryEntry } from './document';
import { DEFAULT_JOB_OPTIONS, type PreviewStatus } from './preview';
import { captureExtras, restoreExtras } from './tabExtras';
import type { AppState, SliceCreator } from './types';

/**
 * Pestañas de documentos dentro de una ventana.
 *
 * Diseño: los slices vivos (`doc`, `past`, `selectedIds`, `zoom`...) siempre guardan el estado de
 * la pestaña ACTIVA, así que todo el código que lee `s.doc` sigue funcionando sin saber de pestañas.
 * Las pestañas inactivas guardan su estado en `TabEntry.data` (una «foto» con referencias, sin copiar
 * documentos: son inmutables). Cambiar de pestaña intercambia la foto con los slices en un solo
 * `set`. Cada pestaña tiene un `id` estable; las operaciones asíncronas capturan el id al empezar y
 * aplican su resultado con `runInTab` (ver abajo), de modo que nunca escriben en otra pestaña.
 *
 * Qué es por pestaña: documento, historial, archivo, sucio/guardado, selección (y nodos), capa
 * activa, zoom/pan y la vista previa (visible, avance, opciones del trabajo, resultado). Qué es de la
 * ventana: portapapeles, herramienta, máquina, preferencias, dispositivo activo.
 */

/** Pasos de deshacer que conserva una pestaña INACTIVA (la activa llega a `HISTORY_LIMIT`). */
export const INACTIVE_HISTORY_STEPS = 30;
/**
 * Presupuesto de memoria del historial de una pestaña inactiva, en puntos de geometría retenidos
 * solo por los pasos (ver `HISTORY_POINT_BUDGET`): el 10 % del de la activa, unos 10 MB.
 */
export const INACTIVE_POINT_BUDGET = 200_000;

/** Estado de una pestaña que se intercambia con los slices vivos al activarla. */
export interface TabData {
  doc: Document;
  savedDoc: Document;
  filePath: string | null;
  dirty: boolean;
  past: HistoryEntry[];
  future: HistoryEntry[];
  selectedIds: string[];
  nodeSel: Record<string, number[]>;
  nodeCorners: Record<string, string[]>;
  activeLayer: string | null;
  zoom: number;
  panX: number;
  panY: number;
  autoFit: boolean;
  previewVisible: boolean;
  scrub: number;
  jobOptions: JobOptions;
  cutSelectionOnly: boolean;
  preview: JobPreview | null;
  previewStatus: PreviewStatus;
  previewStale: boolean;
  previewError: string | null;
  previewErrorCode: string | null;
  /** Estado por pestaña de fuera del store principal (ver `tabExtras.ts`: p. ej. los CSV leídos). */
  extras?: Record<string, unknown>;
}

export interface TabEntry {
  id: string;
  /** Número de «Sin título N» mientras no tenga archivo. */
  untitled: number;
  /** Foto del estado; `null` en la pestaña activa (su estado está en los slices vivos). */
  data: TabData | null;
}

/** Lo que muestra la barra de pestañas (la activa lee sus datos de los slices vivos). */
export interface TabView {
  id: string;
  untitled: number;
  filePath: string | null;
  dirty: boolean;
  active: boolean;
}

/** Desde qué pestaña se lanzó el trabajo en curso (el trabajo usa su copia, no el documento vivo). */
export interface JobOrigin {
  tabId: string;
  /** Nombre que mostraba la pestaña al lanzar. */
  name: string;
  /** Ruta del proyecto al lanzar (null = sin guardar). */
  path: string | null;
  /** Vista previa con la que se lanzó (para progreso y capa), si estaba al día. */
  preview: JobPreview | null;
}

export interface TabsSlice {
  tabs: TabEntry[];
  activeTabId: string;
  /** Sube solo cuando el USUARIO cambia de pestaña (no con `runInTab`): cancela gestos y avisa a la interfaz. */
  tabSwitches: number;
  jobOrigin: JobOrigin | null;

  /** Pestaña nueva vacía, activa. Devuelve su id. */
  newTab: () => string;
  /**
   * Abre un documento en una pestaña nueva y la activa. Si la pestaña activa está vacía y sin cambios,
   * la reemplaza. Con `unsaved`, el documento cuenta como con cambios sin guardar (recuperación).
   */
  openInTab: (doc: Document, path: string | null, opts?: { unsaved?: boolean }) => string;
  activateTab: (id: string) => void;
  /** Activa la pestaña `dir` posiciones a la derecha (negativo, a la izquierda), dando la vuelta. */
  cycleTab: (dir: number) => void;
  /** Quita la pestaña sin preguntar. La última se sustituye por una vacía. Devuelve la que queda activa. */
  removeTab: (id: string) => string;
  /** Mueve la pestaña a la posición `to` (0-based). */
  moveTab: (id: string, to: number) => void;
  /** Copia de la pestaña (sin archivo, con cambios sin guardar) justo a su derecha; queda activa. */
  duplicateTab: (id: string) => string | null;
  /**
   * Ejecuta `fn` (síncrona) con la pestaña `id` como activa y vuelve a la que estaba. Es lo que usan las
   * operaciones asíncronas para aplicar su resultado donde empezaron. Devuelve `undefined` si la pestaña
   * ya no existe (no se ejecuta nada).
   */
  runInTab: <T>(id: string, fn: () => T) => T | undefined;
  noteJobOrigin: (origin: JobOrigin | null) => void;
}

/** Datos de una pestaña, vistos por la barra y los diálogos. */
export function tabViews(s: Pick<AppState, 'tabs' | 'activeTabId' | 'filePath' | 'dirty'>): TabView[] {
  return s.tabs.map((e) =>
    e.id === s.activeTabId
      ? { id: e.id, untitled: e.untitled, filePath: s.filePath, dirty: s.dirty, active: true }
      : { id: e.id, untitled: e.untitled, filePath: e.data?.filePath ?? null, dirty: e.data?.dirty ?? false, active: false },
  );
}

/** Documento de cualquier pestaña (la activa, de los slices vivos). */
export function tabDoc(s: Pick<AppState, 'tabs' | 'activeTabId' | 'doc'>, id: string): Document | null {
  if (id === s.activeTabId) return s.doc;
  return s.tabs.find((e) => e.id === id)?.data?.doc ?? null;
}

/** ¿La pestaña activa está vacía y sin cambios? Abrir un archivo la reemplaza. */
export function isPristine(s: Pick<AppState, 'doc' | 'dirty' | 'filePath' | 'past' | 'future'>): boolean {
  return !s.dirty && s.filePath === null && s.past.length === 0 && s.future.length === 0 && s.doc.objects.length === 0;
}

/** Nombre de una pestaña: el del archivo, o «Sin título N» (sin número si es la única pestaña sin archivo... y no hay otras). */
export function tabLabel(v: Pick<TabView, 'filePath' | 'untitled'>, numbered: boolean): string {
  if (v.filePath) return v.filePath.split(/[\\/]/).pop() || v.filePath;
  return numbered ? t('tabs.untitledN', { n: v.untitled }) : t('doc.untitled');
}

/** Nombre de la pestaña activa tal como la ve el usuario (con número solo si hay varias pestañas). */
export function activeTabLabel(s: Pick<AppState, 'tabs' | 'activeTabId' | 'filePath' | 'dirty'>): string {
  const v = tabViews(s).find((x) => x.active);
  return v ? tabLabel(v, s.tabs.length > 1) : t('doc.untitled');
}

const sameFile = (a: string, b: string) => a.replace(/\\/g, '/') === b.replace(/\\/g, '/');

/** Id de la pestaña que tiene abierto `path`, si hay una. */
export function findTabByPath(s: Pick<AppState, 'tabs' | 'activeTabId' | 'filePath' | 'dirty'>, path: string): string | null {
  return tabViews(s).find((v) => v.filePath !== null && sameFile(v.filePath, path))?.id ?? null;
}

export function newTabId(): string {
  const bytes = new Uint8Array(6);
  if (typeof globalThis.crypto?.getRandomValues === 'function') globalThis.crypto.getRandomValues(bytes);
  else for (let i = 0; i < bytes.length; i++) bytes[i] = Math.floor(Math.random() * 256);
  return `tab-${Date.now().toString(36)}-${[...bytes].map((b) => b.toString(16).padStart(2, '0')).join('')}`;
}

function smallestFree(used: Iterable<number>): number {
  const taken = new Set(used);
  let n = 1;
  while (taken.has(n)) n++;
  return n;
}

/** Foto del estado vivo (la pestaña activa). */
export function captureTab(s: AppState): TabData {
  const computing = s.previewStatus === 'computing';
  return {
    doc: s.doc,
    savedDoc: s.savedDoc,
    filePath: s.filePath,
    dirty: s.dirty,
    past: s.past,
    future: s.future,
    selectedIds: s.selectedIds,
    nodeSel: s.nodeSel,
    nodeCorners: s.nodeCorners,
    activeLayer: s.activeLayer,
    zoom: s.zoom,
    panX: s.panX,
    panY: s.panY,
    autoFit: s.autoFit,
    previewVisible: s.previewVisible,
    scrub: s.scrub,
    jobOptions: s.jobOptions,
    cutSelectionOnly: s.cutSelectionOnly,
    preview: s.preview,
    // Un cálculo en vuelo se abandona al cambiar de pestaña: al volver se recalcula.
    previewStatus: computing ? 'idle' : s.previewStatus,
    previewStale: computing ? true : s.previewStale,
    previewError: s.previewError,
    previewErrorCode: s.previewErrorCode,
    extras: captureExtras(),
  };
}

/** Recorta el historial de una pestaña que pasa a segundo plano (ver `INACTIVE_*`). */
export function trimInactive(d: TabData): TabData {
  const past = trimHistory(d.past, INACTIVE_HISTORY_STEPS, INACTIVE_POINT_BUDGET);
  let future = d.future.length > INACTIVE_HISTORY_STEPS ? d.future.slice(0, INACTIVE_HISTORY_STEPS) : d.future;
  let acc = 0;
  for (let i = 0; i < future.length; i++) {
    acc += future[i]!.cost ?? 0;
    if (acc > INACTIVE_POINT_BUDGET && i > 0) {
      future = future.slice(0, i);
      break;
    }
  }
  return past === d.past && future === d.future ? d : { ...d, past, future };
}

/**
 * Lo que se escribe en los slices vivos al activar una foto. Además restaura (como efecto) el estado por
 * pestaña de fuera del store (`extras`), ANTES del `set` del llamador: así los suscriptores del store
 * ya ven los CSV de la pestaña que se activa junto con su documento.
 */
function restoreOf(d: TabData): Partial<AppState> {
  restoreExtras(d.extras);
  return {
    doc: d.doc,
    savedDoc: d.savedDoc,
    filePath: d.filePath,
    dirty: d.dirty,
    past: d.past,
    future: d.future,
    selectedIds: d.selectedIds,
    nodeSel: d.nodeSel,
    nodeCorners: d.nodeCorners,
    activeLayer: d.activeLayer,
    zoom: d.zoom,
    panX: d.panX,
    panY: d.panY,
    autoFit: d.autoFit,
    previewVisible: d.previewVisible,
    scrub: d.scrub,
    jobOptions: d.jobOptions,
    cutSelectionOnly: d.cutSelectionOnly,
    preview: d.preview,
    previewStatus: d.previewStatus,
    previewStale: d.previewStale,
    previewError: d.previewError,
    previewErrorCode: d.previewErrorCode,
    playing: false,
    cursor: null,
  };
}

/** Opciones de trabajo de una pestaña nueva: las de ahora, sin lo que es de un trabajo concreto. */
function inheritedJobOptions(current: JobOptions): JobOptions {
  return { ...current, selection: null, use_selection_origin: false, transform: null, tile: null };
}

/** Foto de una pestaña recién abierta con `doc`. */
function blankData(s: AppState, doc: Document, filePath: string | null, unsaved: boolean): TabData {
  return {
    doc,
    savedDoc: unsaved ? emptyDocument() : doc,
    filePath,
    dirty: unsaved,
    past: [],
    future: [],
    selectedIds: [],
    nodeSel: {},
    nodeCorners: {},
    activeLayer: null,
    zoom: s.zoom,
    panX: s.panX,
    panY: s.panY,
    autoFit: true,
    previewVisible: false,
    scrub: 1,
    jobOptions: inheritedJobOptions(s.jobOptions ?? DEFAULT_JOB_OPTIONS),
    cutSelectionOnly: false,
    preview: null,
    previewStatus: 'idle',
    previewStale: true,
    previewError: null,
    previewErrorCode: null,
  };
}

export const createTabsSlice: SliceCreator<TabsSlice> = (set, get) => {
  const firstId = newTabId();

  /** Números de «Sin título» en uso (solo cuentan las pestañas sin archivo). */
  const usedUntitled = (): number[] => tabViews(get()).filter((v) => v.filePath === null).map((v) => v.untitled);

  /** Activa la pestaña `id` intercambiando fotos. `user`: cambio del usuario (recorta historial y avisa). */
  const swapTo = (id: string, user: boolean): void => {
    const s = get();
    const target = s.tabs.find((e) => e.id === id);
    if (!target?.data || id === s.activeTabId) return;
    const leaving = captureTab(s);
    const stored = user ? trimInactive(leaving) : leaving;
    set({
      ...restoreOf(target.data),
      tabs: s.tabs.map((e) => (e.id === s.activeTabId ? { ...e, data: stored } : e.id === id ? { ...e, data: null } : e)),
      activeTabId: id,
      ...(user ? { tabSwitches: s.tabSwitches + 1 } : {}),
    });
    get().abandonPreview();
  };

  /** Pone `data` como pestaña nueva en `at` y la activa. */
  const insertActive = (data: TabData, at: number): string => {
    const s = get();
    const id = newTabId();
    const entry: TabEntry = { id, untitled: smallestFree(usedUntitled()), data: null };
    const leaving = trimInactive(captureTab(s));
    const tabs = s.tabs.map((e) => (e.id === s.activeTabId ? { ...e, data: leaving } : e));
    tabs.splice(Math.max(0, Math.min(tabs.length, at)), 0, entry);
    set({ ...restoreOf(data), tabs, activeTabId: id, tabSwitches: s.tabSwitches + 1 });
    get().abandonPreview();
    return id;
  };

  return {
    tabs: [{ id: firstId, untitled: 1, data: null }],
    activeTabId: firstId,
    tabSwitches: 0,
    jobOrigin: null,

    newTab: () => {
      const s = get();
      const id = insertActive(blankData(s, { ...emptyDocument(), device_id: s.doc.device_id }, null, false), s.tabs.length);
      get().fitBed();
      return id;
    },

    openInTab: (doc, path, opts) => {
      const s = get();
      const unsaved = opts?.unsaved === true;
      if (isPristine(s)) {
        s.replaceDocument(doc, path);
        if (unsaved) set({ savedDoc: emptyDocument(), dirty: true });
        return s.activeTabId;
      }
      return insertActive(blankData(s, normalizeLayers(doc), path, unsaved), s.tabs.length);
    },

    activateTab: (id) => swapTo(id, true),

    cycleTab: (dir) => {
      const { tabs, activeTabId } = get();
      if (tabs.length < 2) return;
      const at = tabs.findIndex((e) => e.id === activeTabId);
      const next = tabs[(((at + dir) % tabs.length) + tabs.length) % tabs.length]!;
      swapTo(next.id, true);
    },

    removeTab: (id) => {
      const s = get();
      const at = s.tabs.findIndex((e) => e.id === id);
      if (at < 0) return s.activeTabId;
      if (s.tabs.length === 1) {
        // La última pestaña se sustituye por una vacía (como cerrar y abrir otra): id nuevo, copia de recuperación aparte.
        const fresh = newTabId();
        const data = blankData(s, { ...emptyDocument(), device_id: s.doc.device_id }, null, false);
        set({ ...restoreOf(data), tabs: [{ id: fresh, untitled: 1, data: null }], activeTabId: fresh, tabSwitches: s.tabSwitches + 1 });
        get().abandonPreview();
        get().fitBed();
        return fresh;
      }
      if (id !== s.activeTabId) {
        set({ tabs: s.tabs.filter((e) => e.id !== id) });
        return s.activeTabId;
      }
      const neighbour = s.tabs[at + 1] ?? s.tabs[at - 1]!;
      swapTo(neighbour.id, true);
      set((cur) => ({ tabs: cur.tabs.filter((e) => e.id !== id) }));
      return neighbour.id;
    },

    moveTab: (id, to) => {
      const { tabs } = get();
      const from = tabs.findIndex((e) => e.id === id);
      if (from < 0) return;
      const target = Math.max(0, Math.min(tabs.length - 1, Math.round(to)));
      if (target === from) return;
      const next = tabs.slice();
      const [moved] = next.splice(from, 1);
      next.splice(target, 0, moved!);
      set({ tabs: next });
    },

    duplicateTab: (id) => {
      const s = get();
      const at = s.tabs.findIndex((e) => e.id === id);
      if (at < 0) return null;
      const src = id === s.activeTabId ? captureTab(s) : s.tabs[at]!.data;
      if (!src) return null;
      const data: TabData = {
        ...src,
        savedDoc: emptyDocument(),
        filePath: null,
        dirty: true,
        past: [],
        future: [],
        selectedIds: [],
        nodeSel: {},
        nodeCorners: {},
        previewStatus: 'idle',
        previewStale: true,
        preview: null,
      };
      return insertActive(data, at + 1);
    },

    runInTab: <T,>(id: string, fn: () => T): T | undefined => {
      const s = get();
      if (s.activeTabId === id) return fn();
      if (!s.tabs.some((e) => e.id === id && e.data)) return undefined;
      const back = s.activeTabId;
      swapTo(id, false);
      try {
        return fn();
      } finally {
        swapTo(back, false);
      }
    },

    noteJobOrigin: (jobOrigin) => set({ jobOrigin }),
  };
};

/**
 * Fija la pestaña activa para una operación asíncrona: `const inTab = pinTab(s)` ANTES del primer `await`, y el
 * resultado se aplica con `inTab(() => { ...escribe en el store... })` DESPUÉS. Si el usuario cambió de pestaña
 * mientras tanto, se escribe en la que empezó la operación (la activa se restaura después). Devuelve
 * `undefined` si esa pestaña ya se cerró: no se escribe nada.
 */
export function pinTab(s: Pick<AppState, 'activeTabId' | 'runInTab'>): <T>(fn: () => T) => T | undefined {
  const id = s.activeTabId;
  return (fn) => s.runInTab(id, fn);
}
