import type {
  ConnectionEvent,
  DeviceProfile,
  EventMap,
  EventName,
  JobEvent,
  JobPreview,
  LogEvent,
  MachineState,
  Point,
  ProgressEvent,
  RealtimeCommand,
  StatusReport,
} from '../types';

type Handler<K extends EventName> = (payload: EventMap[K]) => void;

export interface MockMachineOptions {
  /** Cuántas veces más rápido que el tiempo real corre el trabajo simulado. */
  jobSpeedup: number;
  /** Duración mínima de un trabajo simulado, en ms. */
  minJobMs: number;
  statusIntervalMs: number;
  /** Latencia artificial de cada comando, en ms. */
  latencyMs: number;
}

export const DEFAULT_MOCK_OPTIONS: MockMachineOptions = {
  jobSpeedup: 12,
  minJobMs: 6000,
  statusIntervalMs: 250,
  latencyMs: 60,
};

/** Máquina GRBL simulada en memoria: emite estado, progreso, registro y conexión. */
export class MockMachine {
  private handlers = new Map<EventName, Set<(p: never) => void>>();
  private statusTimer: ReturnType<typeof setInterval> | null = null;
  private jobTimer: ReturnType<typeof setInterval> | null = null;
  private frameTimer: ReturnType<typeof setInterval> | null = null;
  private jogTimer: ReturnType<typeof setTimeout> | null = null;

  connected = false;
  port: string | null = null;
  state: MachineState = 'idle';
  /** Código de ALARM:N activo; null si no hay alarma. */
  alarmCode: number | null = null;
  pos: Point = [0, 0];
  feed = 0;
  spindle = 0;

  private job: {
    preview: JobPreview;
    device: DeviceProfile;
    total: number;
    elapsedMs: number;
    durationMs: number;
    paused: boolean;
  } | null = null;

  constructor(readonly opts: MockMachineOptions = DEFAULT_MOCK_OPTIONS) {}

  // ---- eventos ----------------------------------------------------------

  on<K extends EventName>(event: K, handler: Handler<K>): () => void {
    let set = this.handlers.get(event);
    if (!set) this.handlers.set(event, (set = new Set()));
    set.add(handler as (p: never) => void);
    return () => set.delete(handler as (p: never) => void);
  }

  private emit<K extends EventName>(event: K, payload: EventMap[K]): void {
    this.handlers.get(event)?.forEach((h) => h(payload as never));
  }

  private log(dir: LogEvent['dir'], text: string): void {
    this.emit('machine://log', { dir, text });
  }

  // ---- conexión ---------------------------------------------------------

  connect(port: string): string {
    if (this.connected) throw new Error('Ya hay una máquina conectada. Desconéctala antes de abrir otro puerto.');
    this.connected = true;
    this.port = port;
    this.state = 'idle';
    this.alarmCode = null;
    this.pos = [0, 0];
    const evt: ConnectionEvent = { connected: true, port, error: null };
    this.emit('machine://connection', evt);
    this.log('in', "Grbl 1.1h ['$' for help]");
    this.statusTimer = setInterval(() => this.emitStatus(), this.opts.statusIntervalMs);
    this.emitStatus();
    return 'Grbl 1.1h';
  }

  disconnect(): void {
    this.stopTimers();
    this.job = null;
    this.alarmCode = null;
    if (!this.connected) return;
    this.connected = false;
    const port = this.port;
    this.port = null;
    this.state = 'idle';
    this.emit('machine://connection', { connected: false, port, error: null });
  }

  dispose(): void {
    this.stopTimers();
    this.handlers.clear();
  }

  private stopTimers(): void {
    for (const t of [this.statusTimer, this.jobTimer, this.frameTimer]) if (t) clearInterval(t);
    if (this.jogTimer) clearTimeout(this.jogTimer);
    this.statusTimer = this.jobTimer = this.frameTimer = this.jogTimer = null;
  }

  requireConnection(): void {
    if (!this.connected) throw new Error('No hay máquina conectada. Conéctala desde la pestaña Máquina.');
  }

  // ---- estado -----------------------------------------------------------

  status(): StatusReport {
    return {
      state: this.state,
      machine_pos: [...this.pos],
      work_pos: [...this.pos],
      feed: this.feed,
      spindle: this.spindle,
      buffer: { planner: 15, rx: 127 },
    };
  }

  emitStatus(): void {
    if (this.connected) this.emit('machine://status', this.status());
  }

  // ---- comandos de línea ------------------------------------------------

  command(line: string): string[] {
    this.requireConnection();
    if (this.job) throw new Error(BUSY_MESSAGE);
    const trimmed = line.trim();
    if (/[?!~]/.test(trimmed)) {
      throw new Error(`'${trimmed}' contiene un carácter de tiempo real. Para pausar, reanudar o reiniciar usa los botones de la pestaña Máquina.`);
    }
    if (trimmed === '') throw new Error('La línea está vacía.');
    this.log('out', trimmed);
    let reply: string[];
    if (trimmed === '$I') reply = ['[VER:1.1h.20190825:LaserSoft simulada]', '[OPT:V,15,128]', 'ok'];
    else if (trimmed === '$$') reply = ['$22=1', '$30=1000', '$31=0', '$32=1', '$100=80.000', '$101=80.000', '$130=400.000', '$131=415.000', 'ok'];
    else if (trimmed === '$X') {
      this.setAlarm(null);
      reply = ['[MSG:Caution: Unlocked]', 'ok'];
    } else if (trimmed === '$H') {
      this.pos = [0, 0];
      this.setAlarm(null);
      reply = ['ok'];
    } else if (this.alarmCode !== null && /^[GMgm]/.test(trimmed)) reply = [describeLine('error:9')];
    else if (/^[$GMgm]/.test(trimmed)) reply = ['ok'];
    else reply = [describeLine('error:20')];
    for (const r of reply) this.log('in', r);
    return reply;
  }

  /** Simula una alarma de la controladora (para desarrollar la interfaz de recuperación). */
  triggerAlarm(code = 1): void {
    this.requireConnection();
    this.clearJob();
    this.setAlarm(code);
    this.log('in', describeLine(`ALARM:${code}`));
  }

  private setAlarm(code: number | null): void {
    this.alarmCode = code;
    this.state = code === null ? 'idle' : 'alarm';
    this.feed = 0;
    this.spindle = 0;
    this.emitStatus();
  }

  realtime(cmd: RealtimeCommand): void {
    this.requireConnection();
    if (cmd === 'status') this.emitStatus();
    else if (cmd === 'hold') this.setPaused(true);
    else if (cmd === 'resume') this.setPaused(false);
    else if (cmd === 'reset') this.cancel('Reinicio de la controladora.');
    else this.cancel();
  }

  jog(dx: number, dy: number, bed: Point, speed = 1500): void {
    this.requireConnection();
    if (this.job) throw new Error(BUSY_MESSAGE);
    if (!Number.isFinite(dx) || !Number.isFinite(dy)) throw new Error('El desplazamiento no es un número.');
    if (!(speed > 0)) throw new Error('La velocidad del movimiento debe ser mayor que 0.');
    this.log('out', `$J=G91 G21 X${dx} Y${dy} F${speed}`);
    if (this.alarmCode !== null) {
      const reply = describeLine('error:9');
      this.log('in', reply);
      throw new Error(`La máquina rechazó el movimiento (${reply}). Si la máquina está en alarma, desbloquéala con $X o haz homing ($H) desde la consola.`);
    }
    this.pos = [Math.min(bed[0], Math.max(0, this.pos[0] + dx)), Math.min(bed[1], Math.max(0, this.pos[1] + dy))];
    this.state = 'jog';
    this.emitStatus();
    this.jogTimer = setTimeout(() => {
      this.jogTimer = null;
      if (this.state === 'jog') {
        this.state = 'idle';
        this.emitStatus();
      }
    }, 200);
    this.log('in', 'ok');
  }

  /** Misma regla que el motor: solo se transmite con la máquina en reposo. */
  private preflight(): void {
    const why: Partial<Record<MachineState, string>> = {
      alarm:
        'la máquina está en ALARMA: GRBL no acepta G-code hasta que se resuelva. No se transmitió nada.\n' +
        'Qué hacer (la app nunca ejecuta estos comandos por su cuenta; los envías tú desde la consola):\n' +
        '  - si tu máquina tiene finales de carrera (homing): $H (escribe $H en la consola de la pestaña Máquina)\n' +
        '  - si no los tiene: $X (escribe $X en la consola de la pestaña Máquina)',
      hold: 'la máquina no está en reposo, está en pausa (Hold): hay un movimiento detenido con feed hold. Reanúdalo o cancélalo con un soft reset antes de enviar otro trabajo. No se transmitió nada.',
      door: 'la máquina no está en reposo, la puerta o tapa de seguridad está abierta. No se transmitió nada.',
      run: 'la máquina no está en reposo, está ejecutando un movimiento: ya hay un trabajo en curso. No se transmitió nada.',
      jog: 'la máquina no está en reposo, está en jog (movimiento manual); espera a que termine. No se transmitió nada.',
    };
    const msg = why[this.state];
    if (msg) throw new Error(msg.charAt(0).toUpperCase() + msg.slice(1));
  }

  // ---- trabajos ---------------------------------------------------------

  startJob(preview: JobPreview, device: DeviceProfile): void {
    this.requireConnection();
    if (this.job) throw new Error(BUSY_MESSAGE);
    this.preflight();
    if (preview.moves.length === 0) throw new Error('El trabajo está vacío: no hay nada que enviar.');
    const total = preview.moves.length / 4;
    this.job = {
      preview,
      device,
      total,
      elapsedMs: 0,
      durationMs: Math.max(this.opts.minJobMs, (preview.total_s * 1000) / this.opts.jobSpeedup),
      paused: false,
    };
    this.state = 'run';
    this.emit('machine://job', { state: 'started', message: `${total} líneas` } satisfies JobEvent);
    const tick = 100;
    this.jobTimer = setInterval(() => this.tickJob(tick), tick);
  }

  private tickJob(dt: number): void {
    const job = this.job;
    if (!job || job.paused) return;
    job.elapsedMs += dt;
    const f = Math.min(1, job.elapsedMs / job.durationMs);
    // Tiempo simulado del trabajo: la posición sigue `move_times`, como la vista previa en tiempo real.
    const t = f * job.preview.total_s;
    const i = Math.min(job.total - 1, firstAtOrAfter(job.preview.move_times, t, job.total));
    const m = job.preview.moves;
    this.pos = [m[i * 4 + 1]!, m[i * 4 + 2]!];
    this.feed = m[i * 4]! === 1 ? 1500 : 12000;
    this.spindle = Math.round((m[i * 4 + 3]! / 100) * job.device.s_max);
    this.emit('machine://progress', { acked: Math.floor(f * job.total), total: job.total } satisfies ProgressEvent);
    this.emitStatus();
    if (f >= 1) this.finish();
  }

  private finish(): void {
    const job = this.job;
    this.clearJob();
    if (job?.device.return_to_origin) this.pos = [0, 0];
    this.emit('machine://job', { state: 'finished', message: 'Trabajo terminado.' });
    this.emitStatus();
  }

  private clearJob(): void {
    if (this.jobTimer) clearInterval(this.jobTimer);
    this.jobTimer = null;
    this.job = null;
    this.state = 'idle';
    this.feed = 0;
    this.spindle = 0;
  }

  setPaused(paused: boolean): void {
    this.requireConnection();
    if (!this.job) return;
    this.job.paused = paused;
    this.state = paused ? 'hold' : 'run';
    this.spindle = 0;
    this.emit('machine://job', { state: paused ? 'paused' : 'started', message: paused ? 'En pausa.' : 'Reanudado.' });
    this.emitStatus();
  }

  cancel(message = 'Cancelado por el usuario.'): void {
    this.requireConnection();
    if (this.frameTimer) clearInterval(this.frameTimer);
    this.frameTimer = null;
    if (!this.job) {
      // Sin trabajo: cancela un jog o un encuadre; una alarma sigue hasta $X o $H.
      if (this.state !== 'alarm') this.state = 'idle';
      this.feed = 0;
      this.emitStatus();
      return;
    }
    this.clearJob();
    this.emit('machine://job', { state: 'cancelled', message });
    this.emitStatus();
  }

  /** Recorre el rectángulo del trabajo con el láser apagado. */
  frame(bounds: [Point, Point]): void {
    this.requireConnection();
    if (this.job) throw new Error(BUSY_MESSAGE);
    this.preflight();
    const [[x0, y0], [x1, y1]] = bounds;
    const corners: Point[] = [[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]];
    let step = 0;
    this.state = 'run';
    this.feed = 6000;
    this.log('out', `; encuadre ${x0.toFixed(1)},${y0.toFixed(1)} a ${x1.toFixed(1)},${y1.toFixed(1)}`);
    this.frameTimer = setInterval(() => {
      this.pos = corners[Math.min(step, corners.length - 1)]!;
      this.emitStatus();
      if (++step >= corners.length) {
        if (this.frameTimer) clearInterval(this.frameTimer);
        this.frameTimer = null;
        this.state = 'idle';
        this.feed = 0;
        this.emitStatus();
      }
    }, 450);
  }
}

/** Mensaje cuando se pide algo con un trabajo en marcha (igual que el motor real). */
const BUSY_MESSAGE = 'Hay un trabajo en curso. Pausa o cancela antes de enviar otra cosa a la máquina.';

const ERRORS: Record<number, string> = {
  9: 'G-code bloqueado por alarma o jog',
  20: 'comando G-code no soportado o no válido',
};
const ALARMS: Record<number, string> = {
  1: 'límite físico activado',
  3: 'reinicio durante un movimiento: se perdió la posición',
};

/** `error:N` / `ALARM:N` con su explicación, como las devuelve el motor real. */
export function describeLine(line: string): string {
  const err = /^error:(\d+)$/.exec(line);
  if (err) return `${line}: ${ERRORS[Number(err[1])] ?? 'error desconocido'}`;
  const alarm = /^ALARM:(\d+)$/.exec(line);
  if (alarm) return `${line}: ${ALARMS[Number(alarm[1])] ?? 'alarma desconocida'}`;
  return line;
}

/** Índice del primer movimiento que termina en `t` segundos o después (búsqueda binaria). */
function firstAtOrAfter(times: number[], t: number, fallback: number): number {
  let lo = 0;
  let hi = times.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (times[mid]! < t) lo = mid + 1;
    else hi = mid;
  }
  return lo >= times.length ? fallback - 1 : lo;
}
