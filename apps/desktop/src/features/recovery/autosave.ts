import { getApi, type LaserApi } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { tabDoc, tabLabel, tabViews, useStore, type AppState } from '@/store';

/** Lo mínimo que se necesita del store (sirve el real y el de las pruebas). */
export interface AutosaveStore {
  getState: () => AppState;
  subscribe: (listener: (state: AppState, prev: AppState) => void) => () => void;
}

export type AutosaveApi = Pick<LaserApi, 'autosaveWrite' | 'autosaveDiscard'>;

export interface AutosaverOptions {
  newId?: () => string;
  /** Temporizador inyectable para las pruebas. */
  setInterval?: (fn: () => void, ms: number) => unknown;
  clearInterval?: (handle: unknown) => void;
}

export type TickResult = 'saved' | 'skipped' | 'failed';

function randomId(): string {
  const bytes = new Uint8Array(8);
  globalThis.crypto.getRandomValues(bytes);
  return `doc-${Date.now().toString(36)}-${[...bytes].map((b) => b.toString(16).padStart(2, '0')).join('')}`;
}

/** Una «sesión de copia» de una pestaña: la copia en disco y el último documento que se escribió en ella. */
interface Session {
  id: string;
  lastWritten: AppState['doc'] | null;
}

/**
 * Autoguardado: cada N segundos (preferencia `autosaveSecs`), y solo si hay cambios sin guardar
 * desde la última copia, manda el documento de CADA pestaña con cambios a `autosaveWrite`. Una copia
 * pertenece a una «sesión» de una pestaña: cuando la pestaña deja de tener cambios (se guardó, se
 * deshizo todo) o se cierra, su copia se borra y la siguiente sesión usa otro id. Así, tras un cierre
 * inesperado queda una copia por pestaña con cambios, y recuperar una no toca a las demás.
 */
export class Autosaver {
  private readonly sessions = new Map<string, Session>();
  /** Si cada pestaña tenía cambios en la última revisión (para notar cuándo dejan de tenerlos). */
  private wasDirty = new Map<string, boolean>();
  private writing = false;
  private failing = false;
  private handle: unknown = null;
  private armedFor = '';
  private off: (() => void) | null = null;
  private readonly newId: () => string;
  private readonly setTimer: (fn: () => void, ms: number) => unknown;
  private readonly clearTimer: (handle: unknown) => void;

  constructor(
    private readonly api: AutosaveApi,
    private readonly store: AutosaveStore,
    opts: AutosaverOptions = {},
  ) {
    this.newId = opts.newId ?? randomId;
    this.setTimer = opts.setInterval ?? ((fn, ms) => setInterval(fn, ms));
    this.clearTimer = opts.clearInterval ?? ((h) => clearInterval(h as ReturnType<typeof setInterval>));
    this.session(this.store.getState().activeTabId);
  }

  private session(tabId: string): Session {
    let s = this.sessions.get(tabId);
    if (!s) {
      s = { id: this.newId(), lastWritten: null };
      this.sessions.set(tabId, s);
    }
    return s;
  }

  /** Id de la copia de la sesión actual de una pestaña (por omisión, la activa). */
  currentId(tabId: string = this.store.getState().activeTabId): string {
    return this.session(tabId).id;
  }

  start(): () => void {
    this.rearm();
    this.wasDirty = this.dirtyMap(this.store.getState());
    this.off = this.store.subscribe((s, prev) => {
      if (s.prefs.autosave !== prev.prefs.autosave || s.prefs.autosaveSecs !== prev.prefs.autosaveSecs) this.rearm();
      if (s.tabs === prev.tabs && s.dirty === prev.dirty && s.activeTabId === prev.activeTabId) return;
      const now = this.dirtyMap(s);
      for (const id of [...this.sessions.keys()]) {
        const dirty = now.get(id);
        if (dirty === undefined) void this.dropTab(id); // la pestaña se cerró: su copia ya no hace falta
        else if (!dirty && this.wasDirty.get(id)) void this.endSession(id); // sin cambios: tampoco
      }
      this.wasDirty = now;
    });
    return () => this.stop();
  }

  stop(): void {
    this.off?.();
    this.off = null;
    if (this.handle !== null) this.clearTimer(this.handle);
    this.handle = null;
    this.armedFor = '';
  }

  private dirtyMap(s: AppState): Map<string, boolean> {
    return new Map(tabViews(s).map((v) => [v.id, v.dirty]));
  }

  private rearm(): void {
    const { autosave, autosaveSecs } = this.store.getState().prefs;
    const key = autosave ? String(autosaveSecs) : 'off';
    if (key === this.armedFor) return;
    if (this.handle !== null) this.clearTimer(this.handle);
    this.handle = autosave ? this.setTimer(() => void this.tick(), autosaveSecs * 1000) : null;
    this.armedFor = key;
  }

  /**
   * Una vuelta del temporizador sobre todas las pestañas con cambios (o solo sobre `only`). Con `force` guarda
   * aunque el autoguardado esté apagado o no haya cambios nuevos.
   */
  async tick(force = false, only?: string): Promise<TickResult> {
    if (this.writing) return 'skipped';
    this.writing = true;
    let result: TickResult = 'skipped';
    try {
      for (const view of tabViews(this.store.getState())) {
        if (only !== undefined && view.id !== only) continue;
        const s = this.store.getState();
        if (!force && (!s.prefs.autosave || !view.dirty)) continue;
        const doc = tabDoc(s, view.id);
        if (!doc) continue;
        const sess = this.session(view.id);
        if (!force && doc === sess.lastWritten) continue;
        try {
          await this.api.autosaveWrite({ id: sess.id, name: tabLabel(view, s.tabs.length > 1), path: view.filePath, doc });
          sess.lastWritten = doc;
          this.failing = false;
          if (result !== 'failed') result = 'saved';
        } catch (e) {
          if (!this.failing) this.store.getState().notify('warning', t('prefs.autosave.failed', { error: errorMessage(e) }));
          this.failing = true;
          result = 'failed';
          continue;
        }
        // Si mientras se escribía se guardó (o cerró) la pestaña, la copia ya no sirve.
        const still = tabViews(this.store.getState()).find((v) => v.id === view.id);
        if (this.sessions.get(view.id) !== sess || !still?.dirty) await this.discard(sess.id);
      }
    } finally {
      this.writing = false;
    }
    return result;
  }

  /** Borra la copia de una pestaña y empieza otra sesión (después de guardar o de volver al estado guardado). */
  async endSession(tabId: string = this.store.getState().activeTabId): Promise<void> {
    const old = this.session(tabId).id;
    this.sessions.set(tabId, { id: this.newId(), lastWritten: null });
    await this.discard(old);
  }

  /** La pestaña se cerró: se borra su copia y se olvida su sesión. */
  private async dropTab(tabId: string): Promise<void> {
    const old = this.sessions.get(tabId);
    this.sessions.delete(tabId);
    this.wasDirty.delete(tabId);
    if (old) await this.discard(old.id);
  }

  /** Cerrar la ventana descartando o guardando todo: ninguna copia se necesita. */
  async endAll(): Promise<void> {
    const ids = [...this.sessions.keys()];
    await Promise.all(ids.map((id) => this.endSession(id)));
  }

  private async discard(id: string): Promise<void> {
    try {
      await this.api.autosaveDiscard({ id });
    } catch {
      /* si no se puede borrar, la limpieza del próximo arranque lo resolverá (la copia estará superada) */
    }
  }
}

let active: Autosaver | null = null;

/** Autoguardador de la app (lo crea `RecoveryHost`). */
export function activeAutosaver(): Autosaver | null {
  return active;
}

/** Arranca el autoguardado de la app sobre el store y la API reales. Devuelve la función que lo detiene. */
export function startAutosave(): () => void {
  const saver = new Autosaver(getApi(), useStore);
  active = saver;
  const stop = saver.start();
  return () => {
    stop();
    if (active === saver) active = null;
  };
}

/** Al cerrar la ventana (guardando o descartando): las copias de recuperación de todas las pestañas ya no se necesitan. */
export async function discardOnClose(): Promise<void> {
  await active?.endAll();
}
