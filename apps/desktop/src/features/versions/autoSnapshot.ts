import { getApi, type LaserApi } from '@/api';
import type { Document } from '@/api/types';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { tabDoc, tabViews, useStore, type AppState } from '@/store';
import { useVersionsUi } from './versionsUi';
import { DEFAULT_VERSIONS_CONFIG, type VersionsConfig } from './types';

export interface SnapshotStore {
  getState: () => AppState;
}

export type SnapshotApi = Pick<LaserApi, 'versionsSnapshot'>;

/**
 * Instantáneas automáticas: cada `auto_minutes`, y solo para las pestañas que tienen archivo, cambios sin
 * guardar y un documento distinto del último que se mandó. Las pestañas sin archivo no entran: las cubre la
 * copia de recuperación (`features/recovery`). Un fallo avisa una vez por racha y no detiene nada.
 */
export class AutoSnapshots {
  /** Último documento mandado por pestaña (por identidad: el store crea un objeto nuevo en cada edición). */
  private readonly last = new Map<string, Document>();
  private running = false;
  private failing = false;

  constructor(
    private readonly api: SnapshotApi,
    private readonly store: SnapshotStore,
  ) {}

  /** Una vuelta sobre todas las pestañas. Devuelve cuántas instantáneas se mandaron. */
  async tick(): Promise<number> {
    if (this.running) return 0;
    this.running = true;
    let sent = 0;
    try {
      const open = new Set(tabViews(this.store.getState()).map((v) => v.id));
      for (const id of [...this.last.keys()]) if (!open.has(id)) this.last.delete(id);
      for (const view of tabViews(this.store.getState())) {
        if (!view.filePath || !view.dirty) continue;
        const doc = tabDoc(this.store.getState(), view.id);
        if (!doc || this.last.get(view.id) === doc) continue;
        try {
          await this.api.versionsSnapshot({ path: view.filePath, doc, kind: 'auto' });
          this.last.set(view.id, doc);
          this.failing = false;
          sent += 1;
        } catch (e) {
          if (!this.failing) this.store.getState().notify('warning', t('versions.notice.autoFailed', { error: errorMessage(e) }));
          this.failing = true;
        }
      }
    } finally {
      this.running = false;
    }
    return sent;
  }
}

export interface AutoSnapshotOptions {
  setInterval?: (fn: () => void, ms: number) => unknown;
  clearInterval?: (handle: unknown) => void;
}

/**
 * Arranca las instantáneas automáticas de la app: lee la configuración, arma el temporizador y lo vuelve a
 * armar cuando la configuración cambia. Devuelve la función que lo detiene.
 */
export function startAutoSnapshots(opts: AutoSnapshotOptions = {}): () => void {
  const snap = new AutoSnapshots(getApi(), useStore);
  const setTimer = opts.setInterval ?? ((fn, ms) => setInterval(fn, ms));
  const clearTimer = opts.clearInterval ?? ((h) => clearInterval(h as ReturnType<typeof setInterval>));
  let handle: unknown = null;
  let armedFor = '';
  let stopped = false;

  const arm = (config: VersionsConfig) => {
    const key = config.enabled && config.auto_minutes > 0 ? String(config.auto_minutes) : 'off';
    if (key === armedFor) return;
    if (handle !== null) clearTimer(handle);
    handle = key === 'off' ? null : setTimer(() => void snap.tick(), config.auto_minutes * 60_000);
    armedFor = key;
  };

  const off = useVersionsUi.subscribe((s, prev) => {
    if (s.config !== prev.config) arm(s.config);
  });
  void getApi()
    .versionsConfigGet()
    .then((config) => {
      if (stopped) return;
      useVersionsUi.getState().setConfig(config);
      arm(config);
    })
    .catch(() => {
      if (!stopped) arm(DEFAULT_VERSIONS_CONFIG);
    });

  return () => {
    stopped = true;
    off();
    if (handle !== null) clearTimer(handle);
    handle = null;
    armedFor = '';
  };
}
