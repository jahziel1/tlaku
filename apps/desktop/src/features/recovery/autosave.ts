import { getApi, type LaserApi } from '@/api';
import { baseName } from '@/features/history/paths';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore, type AppState } from '@/store';

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

/**
 * Autoguardado: cada N segundos (preferencia `autosaveSecs`), y solo si hay cambios sin guardar
 * desde la última copia, manda el documento a `autosaveWrite`. Una copia pertenece a una «sesión de
 * documento»: cuando el documento deja de tener cambios (se guardó, se abrió otro, se deshizo todo)
 * la copia se borra y la siguiente sesión usa otro id.
 */
export class Autosaver {
  private id: string;
  private lastWritten: AppState['doc'] | null = null;
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
    this.id = this.newId();
  }

  /** Id de la copia de la sesión actual del documento. */
  currentId(): string {
    return this.id;
  }

  start(): () => void {
    this.rearm();
    this.off = this.store.subscribe((s, prev) => {
      if (s.prefs.autosave !== prev.prefs.autosave || s.prefs.autosaveSecs !== prev.prefs.autosaveSecs) this.rearm();
      // Sin cambios pendientes ya no hace falta la copia.
      if (prev.dirty && !s.dirty) void this.endSession();
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

  private rearm(): void {
    const { autosave, autosaveSecs } = this.store.getState().prefs;
    const key = autosave ? String(autosaveSecs) : 'off';
    if (key === this.armedFor) return;
    if (this.handle !== null) this.clearTimer(this.handle);
    this.handle = autosave ? this.setTimer(() => void this.tick(), autosaveSecs * 1000) : null;
    this.armedFor = key;
  }

  /** Una vuelta del temporizador. Con `force` guarda aunque el autoguardado esté apagado o no haya cambios nuevos. */
  async tick(force = false): Promise<TickResult> {
    const s = this.store.getState();
    if (!force && (!s.prefs.autosave || !s.dirty)) return 'skipped';
    if (!force && s.doc === this.lastWritten) return 'skipped';
    if (this.writing) return 'skipped';
    this.writing = true;
    const id = this.id;
    const doc = s.doc;
    try {
      await this.api.autosaveWrite({ id, name: s.filePath ? baseName(s.filePath) : t('doc.untitled'), path: s.filePath, doc });
      this.lastWritten = doc;
      this.failing = false;
    } catch (e) {
      if (!this.failing) this.store.getState().notify('warning', t('prefs.autosave.failed', { error: errorMessage(e) }));
      this.failing = true;
      return 'failed';
    } finally {
      this.writing = false;
    }
    // Si mientras se escribía se guardó el proyecto, la copia ya no sirve.
    if (this.id !== id || !this.store.getState().dirty) await this.discard(id);
    return 'saved';
  }

  /** Borra la copia actual y empieza otra sesión (después de guardar, abrir otro documento o cerrar descartando). */
  async endSession(): Promise<void> {
    const old = this.id;
    this.id = this.newId();
    this.lastWritten = null;
    await this.discard(old);
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

/** Al cerrar la ventana descartando los cambios: la copia de recuperación ya no se necesita. */
export async function discardOnClose(): Promise<void> {
  await active?.endSession();
}
