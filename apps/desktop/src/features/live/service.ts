import { create } from 'zustand';
import type { StoreApi } from 'zustand';
import { getApi } from '@/api';
import type { DeviceProfile, JobArgs, JobPreview } from '@/api/types';
import { selectActiveDevice } from '@/store/selectors';
import type { AppState } from '@/store/types';
import { takeJobStart } from './capture';
import { reportToDoc } from './frame';
import { jobFraction } from './progress';
import { buildLiveRoute, RouteLocator, type LiveRoute } from './route';
import { emittingAt, frameAt, type LiveFrame } from './follow';
import { LiveTracker, type TrackerMode } from './tracker';

/**
 * Servicio de la vista en vivo (sin React). Sigue la posición REAL que reporta la máquina
 * (`machine://status`) sobre la ruta de la vista previa del trabajo que corre, y la interpola
 * entre reportes para dibujar un cabezal fluido. Es SOLO LECTURA: lee el store y nunca envía nada a
 * la máquina.
 *
 * Fuente del avance, en orden:
 *  1. `position`: la MPos reportada ubicada en la ruta (sin retroceder; ver `route.ts`);
 *  2. `progress`: el progreso del streamer (líneas con `ok`) si la posición no encaja con la ruta;
 *  3. `clock`: tiempo transcurrido sobre el estimado, solo en máquinas que no transmiten ni
 *     reportan posición mientras corren (K40, galvo). Se avisa en el texto como «estimado».
 *
 * Sin consumidores no hace nada: ni ubica ni anima. Los consumidores son la preferencia
 * «Vista en vivo» de la ventana y los celulares conectados al control remoto.
 */

export type LiveSourceKind = 'position' | 'progress' | 'clock';
export type RouteState = 'none' | 'computing' | 'ready' | 'unavailable';

/** Estado de baja frecuencia (porcentaje, fuente...) para la interfaz; el cabezal se pide con `frame()`. */
export interface LiveInfo {
  /** Número del trabajo seguido (sube con cada inicio). */
  jobSeq: number;
  route: RouteState;
  source: LiveSourceKind | null;
  /** 0..1 */
  fraction: number;
  emitting: boolean;
  /** El trabajo seguido terminó así (null = sigue en curso o no hay). */
  ended: 'finished' | 'cancelled' | 'error' | null;
}

const INITIAL: LiveInfo = { jobSeq: 0, route: 'none', source: null, fraction: 0, emitting: false, ended: null };

export const useLive = create<LiveInfo>()(() => INITIAL);

export type { LiveFrame };

/** Lo que se le cuenta al celular en cada informe (ver `RemoteSnapshot.live`). */
export interface LiveReport {
  t: number;
  running: boolean;
  /** Sube cada vez que la máquina retrocedió de verdad. */
  retreat_seq: number;
  src: LiveSourceKind;
}

/**
 * Dónde está la máquina según el último reporte, para quien dibuja en otra ventana (la ventana aparte
 * de la vista en vivo): con esto y la ruta arma su propio `LiveTracker` y se mueve igual de fluido.
 */
export interface LiveReading {
  jobSeq: number;
  /** Tiempo de trabajo (s) del último reporte ubicado. */
  t: number;
  mode: TrackerMode;
  /** Sube cada vez que la máquina retrocedió de verdad. */
  retreatSeq: number;
  spindle: number;
}

type StoreLike = Pick<StoreApi<AppState>, 'getState' | 'subscribe'>;

export interface LiveOptions {
  /** Reloj en ms. */
  now?: () => number;
  /** Calcula la vista previa de unos argumentos (solo cuando la en caché no sirve). */
  compute?: (args: JobArgs) => Promise<JobPreview>;
}

interface Cur {
  seq: number;
  device: DeviceProfile;
  preview: JobPreview | null;
  args: JobArgs | null;
  route: LiveRoute | null;
  /** Dónde estaba la cabeza (mm del documento) al empezar: de ahí sale el primer desplazamiento. */
  start: [number, number] | null;
  computing: boolean;
  locator: RouteLocator | null;
  tracker: LiveTracker | null;
  ended: LiveInfo['ended'];
  prior: number | null;
  source: LiveSourceKind | null;
  spindle: number;
  retreatSeq: number;
  /** Reportes seguidos sin ubicar la posición. */
  lostRun: number;
}

let instance: LiveService | null = null;

/** Quién más (un celular conectado) quiere seguir el trabajo aunque la preferencia de la ventana esté apagada. */
const consumers = new Set<() => boolean>();
const routeListeners = new Set<(route: LiveRoute | null) => void>();
const readingListeners = new Set<() => void>();

/** Avisa cada vez que llega un reporte de la máquina o cambia el final del trabajo (ver `LiveService.reading`). */
export function onLiveReading(fn: () => void): () => void {
  readingListeners.add(fn);
  return () => readingListeners.delete(fn);
}

/** Registra un consumidor externo; `wants()` dice si ahora quiere el seguimiento. Devuelve cómo quitarlo. */
export function addLiveConsumer(wants: () => boolean): () => void {
  consumers.add(wants);
  return () => consumers.delete(wants);
}

/** Avisa cuando la ruta del trabajo seguido cambia (llega, o se descarta con `null`). Llama de inmediato si ya hay una. */
export function onLiveRoute(fn: (route: LiveRoute | null) => void): () => void {
  routeListeners.add(fn);
  const now = instance?.route ?? null;
  if (now) fn(now);
  return () => routeListeners.delete(fn);
}

/** Vuelve a evaluar si alguien quiere el seguimiento (un celular entró o salió). */
export function pokeLive(): void {
  instance?.poke();
}

const busy = (j: AppState['jobState']) => j === 'running' || j === 'paused';
const STREAMS = (k: DeviceProfile['kind']) => k !== 'ruida' && k !== 'lhy_micro' && k !== 'galvo';
/** Máquinas que mientras corren no reportan una posición que siga al cabezal. */
const NO_LIVE_POSITION = (k: DeviceProfile['kind']) => k === 'lhy_micro' || k === 'galvo';
/** Reportes seguidos fuera de la ruta a partir de los cuales se usa el progreso del streamer. */
const LOST_BEFORE_FALLBACK = 3;

export class LiveService {
  private cur: Cur | null = null;
  private seq = 0;
  private wasWanted = false;
  private readonly now: () => number;
  private readonly compute: (args: JobArgs) => Promise<JobPreview>;

  constructor(
    private readonly store: StoreLike,
    opts: LiveOptions = {},
  ) {
    this.now = opts.now ?? (() => performance.now());
    this.compute = opts.compute ?? ((args) => getApi().computeJob(args));
  }

  /** Empieza a seguir el store. Devuelve cómo parar. */
  start(): () => void {
    const unsub = this.store.subscribe((s, prev) => this.onChange(s, prev));
    // Un trabajo que ya corre al montarse (recarga de la interfaz): se sigue desde aquí.
    const s = this.store.getState();
    if (busy(s.jobState)) {
      this.begin(s);
      this.feed(s);
    }
    return () => {
      unsub();
      this.cur = null;
      useLive.setState(INITIAL);
    };
  }

  /** Cambios de «alguien más quiere seguir» (un celular entró o salió). */
  poke(): void {
    const s = this.store.getState();
    this.feed(s);
  }

  /** Quién quiere el seguimiento ahora. */
  wanted(s: AppState = this.store.getState()): boolean {
    if (s.prefs.liveView) return true;
    for (const wants of consumers) if (wants()) return true;
    return false;
  }

  /** La ruta del trabajo seguido, si ya se calculó. */
  get route(): LiveRoute | null {
    return this.cur?.route ?? null;
  }

  get device(): DeviceProfile | null {
    return this.cur?.device ?? null;
  }

  // ---- eventos del store ----

  private onChange(s: AppState, prev: AppState): void {
    if (s.jobState !== prev.jobState) this.onJobState(s, prev);
    const cur = this.cur;
    // Con el trabajo ya terminado, editar el diseño deja la vista en vivo sin sentido.
    if (cur?.ended && s.doc !== prev.doc) return this.clear();
    if (cur && s.progress !== prev.progress) {
      cur.prior = s.progress && s.progress.total > 0 ? Math.min(1, s.progress.acked / s.progress.total) : null;
    }
    const wanted = this.wanted(s);
    const turnedOn = wanted && !this.wasWanted;
    this.wasWanted = wanted;
    if (!cur || !wanted) return;
    if (turnedOn || s.status !== prev.status || s.jobState !== prev.jobState) this.feed(s);
  }

  private onJobState(s: AppState, prev: AppState): void {
    const next = s.jobState;
    if (next === 'running' && !busy(prev.jobState)) {
      this.begin(s);
      return;
    }
    const cur = this.cur;
    if (!cur || busy(next)) return;
    if (!busy(prev.jobState)) return;
    cur.ended = next === 'finished' ? 'finished' : next === 'cancelled' || next === 'error' ? next : 'cancelled';
    if (cur.ended === 'finished') cur.tracker?.finish();
    else cur.tracker?.freeze();
    this.publish(cur);
  }

  private begin(s: AppState): void {
    const note = takeJobStart(Date.now());
    const args = note?.args ?? null;
    const startFrom = args?.options.start_from ?? s.jobOptions.start_from;
    // La vista previa en caché sirve si es del mismo trabajo: sin ajustes del llamador, al día y
    // sin depender de dónde está la cabeza.
    const cached = !note?.tweaked && !s.previewStale && s.previewStatus === 'ready' && startFrom !== 'current_position' ? s.preview : null;
    const device = args?.device ?? selectActiveDevice(s);
    this.cur = {
      seq: ++this.seq,
      device,
      start: s.status ? reportToDoc(device, s.status.work_pos) : null,
      preview: cached,
      args,
      route: null,
      computing: false,
      locator: null,
      tracker: null,
      ended: null,
      prior: s.progress && s.progress.total > 0 ? Math.min(1, s.progress.acked / s.progress.total) : null,
      source: null,
      spindle: 0,
      retreatSeq: 0,
      lostRun: 0,
    };
    useLive.setState({ ...INITIAL, jobSeq: this.seq, route: 'none' });
    this.wasWanted = this.wanted(s);
    routeListeners.forEach((fn) => fn(null));
    if (this.wasWanted) this.ensureRoute(this.cur);
  }

  // ---- ruta ----

  private ensureRoute(cur: Cur): void {
    if (cur.route || cur.computing) return;
    if (cur.preview) return this.setRoute(cur, cur.preview);
    if (!cur.args) return void this.setRouteState(cur, 'unavailable');
    cur.computing = true;
    this.setRouteState(cur, 'computing');
    this.compute(cur.args)
      .then((p) => {
        cur.computing = false;
        if (this.cur === cur) {
          this.setRoute(cur, p);
          this.feed(this.store.getState());
        }
      })
      .catch(() => {
        cur.computing = false;
        if (this.cur === cur) this.setRouteState(cur, 'unavailable');
      });
  }

  private setRoute(cur: Cur, preview: JobPreview): void {
    const route = buildLiveRoute(preview, cur.device.rapid_rate, cur.start);
    cur.route = route;
    if (!route) return this.setRouteState(cur, 'unavailable');
    cur.locator = new RouteLocator(route);
    cur.tracker = new LiveTracker(route.total);
    if (cur.ended === 'finished') cur.tracker.finish();
    this.setRouteState(cur, 'ready');
    routeListeners.forEach((fn) => fn(route));
  }

  private setRouteState(cur: Cur, route: RouteState): void {
    if (this.cur === cur && useLive.getState().route !== route) useLive.setState({ route });
  }

  // ---- seguimiento ----

  private feed(s: AppState): void {
    const cur = this.cur;
    if (!cur || !this.wanted(s)) return;
    if (!cur.route) return this.ensureRoute(cur);
    const { route, tracker, locator } = cur;
    if (!tracker || !locator || cur.ended) return;
    const wall = this.now();
    const st = s.status;
    if (st) cur.spindle = st.spindle;
    let tLoc: number | null = null;
    let retreat = false;
    if (st && !NO_LIVE_POSITION(cur.device.kind)) {
      const doc = reportToDoc(cur.device, st.work_pos);
      const res = doc ? locator.update(doc, wall, cur.prior) : null;
      if (res?.pos && res.status !== 'lost') {
        tLoc = res.pos.t;
        retreat = res.retreat;
        cur.lostRun = 0;
        cur.source = 'position';
        if (retreat) cur.retreatSeq++;
      } else {
        cur.lostRun++;
      }
    }
    if (tLoc === null) {
      if (cur.source === 'position' && cur.lostRun < LOST_BEFORE_FALLBACK && locator.pos) {
        // Un reporte suelto que no encaja: se conserva la última ubicación buena.
        tLoc = locator.pos.t;
      } else if (NO_LIVE_POSITION(cur.device.kind) || cur.lostRun >= LOST_BEFORE_FALLBACK) {
        // Sin posición que encaje: el progreso del streamer, o el reloj en máquinas que no reportan.
        tLoc = jobFraction(s, Date.now()) * route.total;
        cur.source = s.progress && STREAMS(cur.device.kind) ? 'progress' : 'clock';
      } else {
        return; // todavía no llegó un reporte que encaje: no se inventa el avance
      }
    }
    const state = st?.state;
    const running = s.jobState === 'running' && state !== 'hold' && state !== 'door' && state !== 'alarm';
    tracker.report({ t: tLoc, running, retreat }, wall / 1000);
    this.publish(cur);
  }

  /** Actualiza el estado de baja frecuencia solo si algo visible cambió. */
  private publish(cur: Cur): void {
    if (this.cur !== cur) return;
    const total = cur.route?.total ?? 0;
    // Lo dibujado (`tracker.t`) solo avanza cuando alguien pide cuadros (`frame`); con la ventana flotante oculta
    // (la ventana aparte dibuja aparte) nadie los pide, así que el porcentaje no puede depender de eso: nunca queda
    // por detrás de lo último que ubicó la máquina.
    const t = cur.tracker && total > 0 ? Math.max(cur.tracker.t, cur.tracker.started ? cur.tracker.located : 0) : 0;
    const fraction = cur.ended === 'finished' ? 1 : total > 0 ? Math.min(1, t / total) : 0;
    const info = useLive.getState();
    const emitting = !!cur.route && cur.tracker?.mode === 'running' && emittingAt(cur.route, cur.spindle, t);
    const rounded = Math.round(fraction * 1000) / 1000;
    if (info.fraction !== rounded || info.source !== cur.source || info.ended !== cur.ended || info.emitting !== emitting) {
      useLive.setState({ fraction: rounded, source: cur.source, ended: cur.ended, emitting });
    }
    readingListeners.forEach((fn) => fn());
  }

  // ---- lo que piden quienes dibujan ----

  /** El cuadro a dibujar en el instante actual (ms del reloj del servicio). `null` si todavía no hay con qué. */
  frame(wallMs: number = this.now()): LiveFrame | null {
    const cur = this.cur;
    if (!cur?.route || !cur.tracker) return null;
    return frameAt(cur.route, cur.tracker, cur.spindle, wallMs);
  }

  /** Para el celular: dónde está la máquina según el último reporte (sin interpolar). `null` si no se sigue. */
  report(): LiveReport | null {
    const cur = this.cur;
    if (!cur?.tracker?.started || !cur.source) return null;
    const mode = cur.tracker.mode;
    return {
      t: Math.round((mode === 'done' ? cur.tracker.total : cur.tracker.located) * 100) / 100,
      running: mode === 'running',
      retreat_seq: cur.retreatSeq,
      src: cur.source,
    };
  }

  /** El último reporte ubicado en la ruta (sin interpolar); `null` si todavía no hay. */
  reading(): LiveReading | null {
    const cur = this.cur;
    if (!cur?.tracker?.started) return null;
    return { jobSeq: cur.seq, t: cur.tracker.located, mode: cur.tracker.mode, retreatSeq: cur.retreatSeq, spindle: cur.spindle };
  }

  /** ¿Hay un trabajo seguido (en curso o el último)? */
  get active(): boolean {
    return this.cur !== null;
  }

  /** Olvida el trabajo seguido (se cambió el documento o se apagó la vista). */
  clear(): void {
    this.cur = null;
    useLive.setState(INITIAL);
    routeListeners.forEach((fn) => fn(null));
  }
}

export function getLiveService(): LiveService | null {
  return instance;
}

/** Arranca el servicio de la app. Devuelve cómo pararlo. */
export function startLive(store: StoreLike, opts: LiveOptions = {}): () => void {
  instance?.clear();
  const svc = new LiveService(store, opts);
  instance = svc;
  const stop = svc.start();
  return () => {
    stop();
    if (instance === svc) instance = null;
  };
}
