import { create } from 'zustand';
import { getApi } from '@/api';
import { t } from '@/i18n';
import { getPref } from '@/store';
import { readPref } from '@/lib/platform';

/**
 * Historial de trabajos (M51 + mejora 4 del plan de usuarios): lo que se envió a una máquina, con fecha,
 * máquina, documento y pestaña de origen, tiempo estimado y real, resultado, las capas con sus ajustes,
 * el material de la biblioteca, el área, una miniatura y las notas del usuario.
 *
 * Vive en un archivo JSONL de la carpeta de datos de la app (`job-history.jsonl`, comandos `job_history_*`
 * de `src-tauri/src/job_history.rs`), no en `localStorage`: sobrevive a limpiar el WebView, lo comparten
 * las ventanas y no sale del equipo. Aquí vive la copia en memoria (`useJobLog`), que se escribe sin
 * esperar al disco (las escrituras van en cola, en orden). El registro viejo de `localStorage`
 * (`tlaku.jobLog`) se migra una vez al cargar. Lo alimenta el slice de la máquina con los eventos
 * `machine://job` que ya existen (ver `noteJobEvent`).
 */

export type JobResult = 'finished' | 'cancelled' | 'error' | 'interrupted' | 'running';

/**
 * Un trabajo que no terminó y del que el motor guardó un punto de control («continuar un trabajo
 * interrumpido», `store/machine/resume.ts`). `id` es el del punto de control: mientras siga siendo el
 * que existe, el registro ofrece «Continuar».
 */
export interface JobInterruption {
  id: string;
  /** 0 a 100, según los `ok` recibidos. */
  percent: number;
  /** Líneas confirmadas y total. */
  line: number;
  total: number;
}

/** Ajustes de una capa que el historial recuerda y «Repetir con estos ajustes» vuelve a aplicar (nombres de `LayerSettings`). */
export const LAYER_LOG_FIELDS = ['speed', 'power', 'min_power', 'passes', 'interval', 'angle', 'crosshatch', 'bidirectional', 'air', 'kerf'] as const;
export type LayerLogField = (typeof LAYER_LOG_FIELDS)[number];
export type LayerLogSettings = Partial<Record<LayerLogField, number | boolean>>;

/** Una capa del trabajo con los ajustes con que salió. */
export interface JobLayerLog {
  color: string;
  name: string;
  mode: string;
  /** Material de la biblioteca cuyos ajustes coinciden con los de la capa, si hay uno. */
  material?: string;
  settings: LayerLogSettings;
}

export interface JobLogEntry {
  id: string;
  /** Cuándo empezó el trabajo (ms desde 1970). */
  at: number;
  /** Nombre de la máquina (perfil) a la que se envió. */
  machine: string;
  /** Nombre del archivo del proyecto; `null` si no estaba guardado. */
  document: string | null;
  /** Ruta completa del proyecto al enviar; `null` si no estaba guardado. */
  documentPath?: string | null;
  /** Nombre de la pestaña desde la que se lanzó. */
  tab?: string | null;
  /** Tiempo estimado por la vista previa al empezar (s); `null` si no había una al día. */
  estimatedS: number | null;
  /** Tiempo real, sin contar las pausas (s). */
  actualS: number;
  result: JobResult;
  /** Texto del motor al terminar (el motivo, si falló). */
  message: string;
  /** Solo si quedó punto de control para continuarlo. */
  interrupted?: JobInterruption;
  /** Capas que salieron, con sus ajustes. */
  layers?: JobLayerLog[];
  /** Materiales de la biblioteca reconocidos en las capas (nombres, sin repetir). */
  material?: string | null;
  /** Área real de las formas cerradas (mm²) y longitud de corte (mm). */
  areaMm2?: number | null;
  lengthMm?: number | null;
  /** Miniatura: un SVG pequeño con el recorrido. */
  thumb?: string;
  /** Notas del usuario. */
  notes?: string;
}

/** Cuántos trabajos se guardan por omisión; la preferencia `jobHistoryMax` lo cambia. */
export const JOB_LOG_MAX = 1000;
/** Clave vieja de `localStorage` (con el prefijo `tlaku.` que pone `readPref`): se migra y se borra. */
export const JOB_LOG_KEY = 'jobLog';
/** Cuánto después de terminar un trabajo se le puede marcar como interrumpido. */
const INTERRUPTION_WINDOW_MS = 30 * 60 * 1000;
/** Cuánto vale la señal de «sigo vivo» de un trabajo en curso al decidir si otra ventana lo dejó a medias. */
const LIVE_BEAT_MS = 30_000;
const LIVE_KEY = 'tlaku.jobLog.live';

const RESULTS: readonly JobResult[] = ['finished', 'cancelled', 'error', 'interrupted', 'running'];
const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const str = (v: unknown, max: number): string | undefined => (typeof v === 'string' ? v.slice(0, max) : undefined);

/** Tope de trabajos que se guardan según la preferencia. */
export function jobLogMax(): number {
  try {
    const v = getPref('jobHistoryMax');
    return num(v) ? Math.round(v) : JOB_LOG_MAX;
  } catch {
    return JOB_LOG_MAX;
  }
}

function sanitizeInterruption(raw: unknown): JobInterruption | undefined {
  const r = raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : null;
  if (!r || typeof r.id !== 'string' || !r.id || !num(r.percent) || !num(r.line) || !num(r.total)) return undefined;
  return { id: r.id.slice(0, 80), percent: Math.min(100, Math.max(0, r.percent)), line: Math.max(0, r.line), total: Math.max(0, r.total) };
}

function sanitizeSettings(raw: unknown): LayerLogSettings {
  const r = raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : {};
  const out: LayerLogSettings = {};
  for (const k of LAYER_LOG_FIELDS) {
    const v = r[k];
    if (num(v) || typeof v === 'boolean') out[k] = v;
  }
  return out;
}

function sanitizeLayers(raw: unknown): JobLayerLog[] | undefined {
  if (!Array.isArray(raw)) return undefined;
  const out: JobLayerLog[] = [];
  for (const item of raw.slice(0, 40)) {
    const r = item && typeof item === 'object' ? (item as Record<string, unknown>) : null;
    if (!r || typeof r.color !== 'string') continue;
    const material = str(r.material, 200);
    out.push({ color: r.color.slice(0, 20), name: str(r.name, 100) ?? '', mode: str(r.mode, 20) ?? 'line', ...(material ? { material } : {}), settings: sanitizeSettings(r.settings) });
  }
  return out;
}

/** Una miniatura válida es un SVG corto generado por nosotros; cualquier otra cosa se descarta. */
export const THUMB_MAX_CHARS = 12_000;
function sanitizeThumb(raw: unknown): string | undefined {
  return typeof raw === 'string' && raw.startsWith('<svg') && raw.length <= THUMB_MAX_CHARS && !/<script|on\w+\s*=/i.test(raw) ? raw : undefined;
}

/** Entradas válidas a partir de cualquier dato (lo dañado se descarta), de la más nueva a la más vieja. */
export function sanitizeJobLog(raw: unknown, max = JOB_LOG_MAX): JobLogEntry[] {
  if (!Array.isArray(raw)) return [];
  const out: JobLogEntry[] = [];
  for (const item of raw) {
    const r = item && typeof item === 'object' ? (item as Record<string, unknown>) : null;
    if (!r || !num(r.at) || !num(r.actualS) || typeof r.machine !== 'string' || !RESULTS.includes(r.result as JobResult)) continue;
    const interrupted = sanitizeInterruption(r.interrupted);
    const layers = sanitizeLayers(r.layers);
    const thumb = sanitizeThumb(r.thumb);
    const notes = str(r.notes, 2000);
    const material = r.material === null ? null : str(r.material, 300);
    out.push({
      id: typeof r.id === 'string' && r.id ? r.id.slice(0, 120) : `${r.at}`,
      at: r.at,
      machine: r.machine.slice(0, 200),
      document: typeof r.document === 'string' ? r.document.slice(0, 300) : null,
      ...(r.documentPath === null || typeof r.documentPath === 'string' ? { documentPath: r.documentPath === null ? null : r.documentPath.slice(0, 1000) } : {}),
      ...(r.tab === null || typeof r.tab === 'string' ? { tab: r.tab === null ? null : r.tab.slice(0, 200) } : {}),
      estimatedS: num(r.estimatedS) && r.estimatedS >= 0 ? r.estimatedS : null,
      actualS: Math.max(0, r.actualS),
      result: r.result as JobResult,
      message: typeof r.message === 'string' ? r.message.slice(0, 500) : '',
      ...(interrupted ? { interrupted } : {}),
      ...(layers ? { layers } : {}),
      ...(material !== undefined ? { material } : {}),
      ...(num(r.areaMm2) ? { areaMm2: Math.max(0, r.areaMm2) } : r.areaMm2 === null ? { areaMm2: null } : {}),
      ...(num(r.lengthMm) ? { lengthMm: Math.max(0, r.lengthMm) } : r.lengthMm === null ? { lengthMm: null } : {}),
      ...(thumb ? { thumb } : {}),
      ...(notes ? { notes } : {}),
    });
  }
  return out.sort((a, b) => b.at - a.at).slice(0, Math.max(1, max));
}

/** Registro viejo de `localStorage` (clave `tlaku.jobLog`): sus entradas válidas, o `[]`. */
export function readLegacyJobLog(): JobLogEntry[] {
  try {
    return sanitizeJobLog(JSON.parse(readPref(JOB_LOG_KEY) ?? '[]'));
  } catch {
    return [];
  }
}

function dropLegacyJobLog(): void {
  try {
    localStorage.removeItem(`tlaku.${JOB_LOG_KEY}`);
  } catch {
    /* sin almacenamiento */
  }
}

// ---- «sigo vivo» de un trabajo en curso ----------------------------------------------------------------

function writeLive(id: string, now: number): void {
  try {
    localStorage.setItem(LIVE_KEY, JSON.stringify({ id, beat: now }));
  } catch {
    /* sin almacenamiento */
  }
}

function clearLive(): void {
  try {
    localStorage.removeItem(LIVE_KEY);
  } catch {
    /* sin almacenamiento */
  }
}

/** ¿Hay una ventana que sigue ejecutando el trabajo `id`? (latido reciente en el almacenamiento compartido). */
function isLive(id: string, now: number): boolean {
  try {
    const raw = localStorage.getItem(LIVE_KEY);
    const v = raw ? (JSON.parse(raw) as { id?: unknown; beat?: unknown }) : null;
    return !!v && v.id === id && num(v.beat) && now - v.beat < LIVE_BEAT_MS;
  } catch {
    return false;
  }
}

// ---- almacén ---------------------------------------------------------------------------------------------

/** Cola de escrituras: van en orden y un fallo no corta las siguientes. */
let writes: Promise<unknown> = Promise.resolve();
/** Cuenta los cambios hechos en esta ventana (para que `load` sepa si su lectura quedó vieja). */
let changes = 0;
function persist(job: () => Promise<unknown>): void {
  changes++;
  writes = writes.then(job).then(
    () => useJobLog.setState({ saveError: null }),
    (e: unknown) => useJobLog.setState({ saveError: e instanceof Error ? e.message : String(e) }),
  );
}

interface JobLogStore {
  /** De la más nueva a la más vieja. */
  entries: JobLogEntry[];
  /** Ya se leyó el archivo (y se migró el registro viejo). */
  loaded: boolean;
  /** Por qué falló la última escritura en disco (o `null`). */
  saveError: string | null;
  /** Lee el archivo (migrando antes el registro viejo de `localStorage`). Otra ventana pudo anotar algo. */
  load: () => Promise<void>;
  /** Igual que `load`: vuelve a leer. */
  refresh: () => Promise<void>;
  /** Agrega o reemplaza (por `id`) una entrada. */
  append: (entry: JobLogEntry) => void;
  /** Cambia campos de una entrada (notas, marca de interrupción). `null` borra el campo. */
  patch: (id: string, patch: Partial<Record<keyof JobLogEntry, unknown>>) => void;
  /** Marca el trabajo más reciente (si no terminó bien y acaba de pasar) como interrumpido con punto de control. */
  markLatestInterrupted: (info: JobInterruption, now?: number) => void;
  remove: (ids: string[]) => void;
  clear: () => void;
}

const sortedNewestFirst = (entries: JobLogEntry[]) => entries.sort((a, b) => b.at - a.at);

/**
 * Estado del historial. La copia en memoria manda en esta ventana; cada cambio se encola hacia el archivo
 * y `load` vuelve a leerlo (otras ventanas, o el arranque).
 */
export const useJobLog = create<JobLogStore>()((set, get) => ({
  entries: [],
  loaded: false,
  saveError: null,
  load: async () => {
    try {
      await writes;
      const max = jobLogMax();
      const api = getApi();
      const legacy = readLegacyJobLog();
      if (legacy.length > 0) {
        await api.jobHistoryImport({ entries: legacy as unknown as Record<string, unknown>[], max });
        dropLegacyJobLog();
      }
      // Si la copia en memoria cambió mientras se leía el archivo, la lectura quedó vieja: se repite.
      let raw: Record<string, unknown>[] = [];
      for (let i = 0; i < 3; i++) {
        const seen = changes;
        raw = await api.jobHistoryLoad({ max });
        await writes;
        if (seen === changes) break;
      }
      const entries = sanitizeJobLog(raw, max);
      // Un «en curso» que ninguna ventana sigue ejecutando es un trabajo que la app dejó a medias (cierre, corte).
      const now = Date.now();
      const stale = entries.filter((e) => e.result === 'running' && e.id !== pending?.id && !isLive(e.id, now));
      for (const e of stale) {
        e.result = 'interrupted';
        e.message = e.message || t('help.jobLog.appClosed');
        persist(() => getApi().jobHistoryUpdate({ id: e.id, patch: { result: e.result, message: e.message } }));
      }
      set({ entries, loaded: true, saveError: null });
    } catch (e) {
      set({ loaded: true, saveError: e instanceof Error ? e.message : String(e) });
    }
  },
  refresh: () => get().load(),
  append: (entry) => {
    const max = jobLogMax();
    const entries = sanitizeJobLog([entry, ...get().entries.filter((e) => e.id !== entry.id)], max);
    set({ entries });
    const clean = sanitizeJobLog([entry], max)[0];
    if (clean) persist(() => getApi().jobHistoryAppend({ entry: clean as unknown as Record<string, unknown>, max }));
  },
  patch: (id, patch) => {
    const current = get().entries.find((e) => e.id === id);
    if (!current) return;
    const merged: Record<string, unknown> = { ...current };
    for (const [k, v] of Object.entries(patch)) {
      if (k === 'id' || k === 'at') continue;
      if (v === null || v === undefined || v === '') delete merged[k];
      else merged[k] = v;
    }
    const clean = sanitizeJobLog([merged])[0];
    if (!clean) return;
    set({ entries: sortedNewestFirst(get().entries.map((e) => (e.id === id ? clean : e)).slice()) });
    // Lo que la limpieza dejó fuera (notas vacías, por ejemplo) se borra del archivo con `null`.
    const filePatch: Record<string, unknown> = {};
    for (const k of Object.keys(patch)) {
      if (k === 'id' || k === 'at') continue;
      filePatch[k] = (clean as unknown as Record<string, unknown>)[k] ?? null;
    }
    persist(() => getApi().jobHistoryUpdate({ id, patch: filePatch }));
  },
  markLatestInterrupted: (info, now = Date.now()) => {
    const latest = get().entries.find((e) => e.result !== 'running');
    // Solo el último trabajo, solo si no terminó bien y solo si acaba de terminar (no uno viejo).
    if (!latest || latest.result === 'finished' || now - (latest.at + latest.actualS * 1000) > INTERRUPTION_WINDOW_MS) return;
    get().patch(latest.id, { interrupted: info });
  },
  remove: (ids) => {
    if (ids.length === 0) return;
    set({ entries: get().entries.filter((e) => !ids.includes(e.id)) });
    persist(() => getApi().jobHistoryRemove({ ids }));
  },
  clear: () => {
    set({ entries: [] });
    persist(() => getApi().jobHistoryClear());
  },
}));

/** Espera a que terminen las escrituras pendientes (pruebas y cierre ordenado). */
export function flushJobLog(): Promise<void> {
  return writes.then(() => undefined);
}

// ---- anotar desde los eventos de la máquina -------------------------------------------------------

/** Lo que se sabe del trabajo al empezar y que el historial guarda aparte del tiempo (ver `jobLogSnapshot.ts`). */
export interface JobSnapshot {
  documentPath: string | null;
  tab: string | null;
  layers: JobLayerLog[];
  material: string | null;
  areaMm2: number | null;
  lengthMm: number | null;
  thumb: string | null;
}

/** Lo que el slice de la máquina sabe en el momento de un evento del trabajo. */
export interface JobLogContext {
  now: number;
  machine: string;
  document: string | null;
  estimatedS: number | null;
  /** Segundos de trabajo transcurridos (sin pausas); solo cuenta al terminar. */
  elapsedS: number;
  /** Ajustes del trabajo; solo hace falta al empezar. */
  snapshot?: JobSnapshot | null;
}

interface Pending {
  id: string;
  at: number;
  machine: string;
  document: string | null;
  estimatedS: number | null;
  snapshot: JobSnapshot | null;
}

let pending: Pending | null = null;
let seq = 0;
let lastBeat = 0;

/** Olvida el trabajo en curso (para las pruebas). */
export function resetJobLogRecorder(): void {
  pending = null;
  lastBeat = 0;
  clearLive();
}

/** Campos de la entrada que salen de lo que se sabe al empezar. */
function entryOf(p: Pending, extra: Pick<JobLogEntry, 'actualS' | 'result' | 'message'>): JobLogEntry {
  const s = p.snapshot;
  return {
    id: p.id,
    at: p.at,
    machine: p.machine,
    document: p.document,
    estimatedS: p.estimatedS,
    ...extra,
    ...(s
      ? {
          documentPath: s.documentPath,
          tab: s.tab,
          layers: s.layers,
          material: s.material,
          areaMm2: s.areaMm2,
          lengthMm: s.lengthMm,
          ...(s.thumb ? { thumb: s.thumb } : {}),
        }
      : {}),
  };
}

function finish(result: Exclude<JobResult, 'running' | 'interrupted'>, message: string, ctx: JobLogContext): void {
  const start = pending;
  if (!start) return;
  pending = null;
  clearLive();
  useJobLog.getState().append(entryOf(start, { actualS: Math.max(0, ctx.elapsedS), result, message }));
}

/** Señal de que el trabajo en curso sigue vivo (la manda `onStatus`, con un mínimo entre señales). */
export function touchJobLog(now: number): void {
  if (!pending || now - lastBeat < 5000) return;
  lastBeat = now;
  writeLive(pending.id, now);
}

/**
 * Anota un evento `machine://job`. `wasBusy` dice si ya había un trabajo en curso antes del evento
 * (un «started» tras una pausa es una reanudación, no un trabajo nuevo). Al empezar se guarda una entrada
 * «en curso» (si la app se cierra a medias, queda como interrumpida); al llegar un final (terminado,
 * cancelado, error) se completa; pausas y reanudaciones no anotan nada.
 */
export function noteJobEvent(wasBusy: boolean, job: { state: 'started' | 'paused' | 'finished' | 'cancelled' | 'error'; message: string }, ctx: JobLogContext): void {
  if (job.state === 'started') {
    if (!wasBusy) {
      pending = { id: `${ctx.now}-${++seq}`, at: ctx.now, machine: ctx.machine, document: ctx.document, estimatedS: ctx.estimatedS, snapshot: ctx.snapshot ?? null };
      lastBeat = ctx.now;
      writeLive(pending.id, ctx.now);
      useJobLog.getState().append(entryOf(pending, { actualS: 0, result: 'running', message: '' }));
    }
    return;
  }
  if (job.state === 'finished' || job.state === 'cancelled' || job.state === 'error') finish(job.state, job.message, ctx);
}

/** Un trabajo que falló sin evento propio (la máquina entró en alarma durante el trabajo). */
export function noteJobAlarm(message: string, ctx: JobLogContext): void {
  finish('error', message, ctx);
}

/** Id del trabajo en curso (para las pruebas y la cola). */
export function currentJobLogId(): string | null {
  return pending?.id ?? null;
}
