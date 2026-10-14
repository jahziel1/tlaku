import { create } from 'zustand';
import { getApiKind } from '@/api';
import tauriConf from '../../../src-tauri/tauri.conf.json';
import { useHelpUi } from './helpStore';

/**
 * Actualizaciones automáticas (plugin `tauri-plugin-updater`). Todo funciona sin llave: si la
 * compilación no trae `pubkey` y `endpoints` en `tauri.conf.json` (los llena
 * `scripts/configure-updater.sh`, ver `docs/publicar.md`), buscar actualizaciones solo informa que no
 * están configuradas y nunca llama al plugin (que en esa compilación ni siquiera está registrado;
 * ver `src-tauri/src/updater.rs`, que aplica la misma regla).
 */

/** La parte de `plugins.updater` que importa para decidir si hay actualizador. */
export interface UpdaterConfig {
  pubkey?: unknown;
  endpoints?: unknown;
}

/** ¿Trae la configuración una llave pública y al menos una dirección de actualizaciones? */
export function isUpdaterConfigured(cfg: UpdaterConfig | undefined | null = tauriConf.plugins?.updater): boolean {
  if (!cfg) return false;
  const key = typeof cfg.pubkey === 'string' ? cfg.pubkey.trim() : '';
  const urls = Array.isArray(cfg.endpoints) ? cfg.endpoints.filter((e) => typeof e === 'string' && e.trim() !== '') : [];
  return key !== '' && urls.length > 0;
}

/** Lo que usamos de `Update` (`@tauri-apps/plugin-updater`); permite simularlo en las pruebas. */
export interface UpdateLike {
  version: string;
  currentVersion: string;
  date?: string;
  body?: string;
  downloadAndInstall(onEvent?: (event: DownloadEvent) => void): Promise<void>;
}

export type DownloadEvent =
  | { event: 'Started'; data: { contentLength?: number } }
  | { event: 'Progress'; data: { chunkLength: number } }
  | { event: 'Finished' };

export interface UpdateInfo {
  version: string;
  current: string;
  notes: string;
  date: string | null;
}

export type UpdatePhase = 'idle' | 'checking' | 'unconfigured' | 'current' | 'available' | 'downloading' | 'installed' | 'error';

interface UpdateUi {
  phase: UpdatePhase;
  info: UpdateInfo | null;
  /** Bytes bajados y total (si el servidor lo dice). */
  progress: { done: number; total: number | null };
  error: string;
  /** Busca una actualización. `silent`: arranque automático; solo abre el diálogo si hay una nueva. */
  check: (silent?: boolean) => Promise<void>;
  install: () => Promise<void>;
  restart: () => Promise<void>;
  reset: () => void;
}

/** Cómo llega al plugin; las pruebas lo reemplazan con `vi.mock` de los dos módulos. */
export interface UpdaterEnv {
  /** ¿La compilación tiene actualizador configurado y corre dentro de Tauri? */
  available: () => boolean;
}

export const updaterEnv: UpdaterEnv = {
  available: () => isUpdaterConfigured() && getApiKind() === 'tauri',
};

let pending: UpdateLike | null = null;

function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export const useUpdateUi = create<UpdateUi>()((set, get) => ({
  phase: 'idle',
  info: null,
  progress: { done: 0, total: null },
  error: '',

  async check(silent = false) {
    const { phase } = get();
    if (phase === 'checking' || phase === 'downloading') return;
    if (!updaterEnv.available()) {
      // En silencio no se molesta a nadie; a mano se explica.
      if (!silent) set({ phase: 'unconfigured', info: null, error: '' });
      return;
    }
    set({ phase: 'checking', info: null, error: '' });
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const update = (await check()) as UpdateLike | null;
      pending = update;
      if (!update) {
        set({ phase: 'current' });
        return;
      }
      set({
        phase: 'available',
        info: { version: update.version, current: update.currentVersion, notes: (update.body ?? '').trim(), date: update.date ?? null },
      });
      if (silent && useHelpUi.getState().dialog === null) useHelpUi.getState().open('update');
    } catch (e) {
      pending = null;
      set({ phase: 'error', error: message(e) });
    }
  },

  async install() {
    const update = pending;
    if (!update || get().phase !== 'available') return;
    set({ phase: 'downloading', progress: { done: 0, total: null }, error: '' });
    try {
      await update.downloadAndInstall((ev) => {
        if (ev.event === 'Started') set({ progress: { done: 0, total: ev.data.contentLength ?? null } });
        else if (ev.event === 'Progress') set((s) => ({ progress: { ...s.progress, done: s.progress.done + ev.data.chunkLength } }));
      });
      pending = null;
      set({ phase: 'installed' });
    } catch (e) {
      set({ phase: 'error', error: message(e) });
    }
  },

  async restart() {
    try {
      const { relaunch } = await import('@tauri-apps/plugin-process');
      await relaunch();
    } catch (e) {
      set({ phase: 'error', error: message(e) });
    }
  },

  reset() {
    pending = null;
    set({ phase: 'idle', info: null, error: '', progress: { done: 0, total: null } });
  },
}));

/** Comando Ayuda > «Buscar actualizaciones…»: abre el diálogo y empieza a buscar. */
export function openUpdateCheck(): void {
  useHelpUi.getState().open('update');
  void useUpdateUi.getState().check(false);
}

/** Retraso del chequeo de arranque: da tiempo a que carguen las preferencias y no compite con el inicio. */
export const STARTUP_CHECK_DELAY_MS = 8000;

/**
 * Chequeo al iniciar si la preferencia lo pide (apagada de fábrica). Solo en la ventana principal.
 * Devuelve la función que lo cancela.
 */
export function scheduleStartupCheck(isEnabled: () => boolean, isMainWindow: () => Promise<boolean>, delay = STARTUP_CHECK_DELAY_MS): () => void {
  if (!updaterEnv.available()) return () => undefined;
  const timer = setTimeout(() => {
    if (!isEnabled()) return;
    void isMainWindow().then((main) => {
      if (main) void useUpdateUi.getState().check(true);
    });
  }, delay);
  return () => clearTimeout(timer);
}

/** ¿Es la ventana principal (no una de documento `doc-*`)? Fuera de Tauri, sí. */
export async function isMainWindow(): Promise<boolean> {
  if (getApiKind() !== 'tauri') return true;
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    return getCurrentWindow().label === 'main';
  } catch {
    return false;
  }
}
