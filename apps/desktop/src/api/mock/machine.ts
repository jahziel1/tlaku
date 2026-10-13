import { convexHull } from '@/lib/hull';
import type {
  ConnectionEvent,
  DeviceProfile,
  EventMap,
  EventName,
  JobEvent,
  JobPreview,
  LogEvent,
  MachineState,
  OverrideKind,
  Point,
  ProgressEvent,
  RealtimeCommand,
  SimulatedFault,
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

/** Puertos virtuales que ofrece `list_ports` (los mismos ids que el motor real). */
export const MOCK_VIRTUAL_PORTS = ['virtual:falcon2', 'virtual:grbl11', 'virtual:grblhal', 'virtual:fluidnc', 'virtual:marlin', 'virtual:smoothie'];

/** Máquinas virtuales por red (`virtualnet:...`): el mismo emulador detrás de un socket TCP local. */
/** Máquina virtual Ruida (el emulador UDP de `tlaku-ruida`); solo se ofrece con un perfil Ruida. */
export const MOCK_RUIDA_PORT = 'virtual:ruida';
export const RUIDA_FIRMWARE = 'Ruida RDC6442S · tarjeta 0x65106510';
const RUIDA_GRBL_ONLY = 'Esta función es solo de GRBL: la controladora Ruida no usa G-code ni overrides en vivo.';

export const MOCK_VIRTUAL_NET_PORTS = MOCK_VIRTUAL_PORTS.map((p) => p.replace('virtual:', 'virtualnet:'));

const VIRTUAL_FIRMWARE: Record<string, string> = {
  falcon2: 'Grbl 1.1f',
  grbl11: 'Grbl 1.1h',
  grblhal: 'grblHAL 1.1f',
  fluidnc: 'FluidNC v3.9.7',
  marlin: 'Marlin 2.1.2.5',
  smoothie: 'Smoothieware',
};

const VIRTUAL_PREFIX = 'virtual:';
const VIRTUAL_NET_PREFIX = 'virtualnet:';

/** Ajustes de partida de la máquina simulada (un láser de diodo típico, sin los de velocidad ni aceleración). */
const MOCK_GRBL_SETTINGS: [number, string][] = [
  [0, '10'], [1, '25'], [2, '0'], [3, '0'], [4, '0'], [5, '0'], [6, '0'], [10, '1'], [11, '0.010'], [12, '0.002'], [13, '0'],
  [20, '0'], [21, '0'], [22, '1'], [23, '0'], [24, '25.000'], [25, '500.000'], [26, '250'], [27, '1.000'],
  [30, '1000'], [31, '0'], [32, '1'], [100, '80.000'], [101, '80.000'], [130, '400.000'], [131, '415.000'],
];

/** Máquina GRBL simulada en memoria: emite estado, progreso, registro y conexión. */
export class MockMachine {
  private handlers = new Map<EventName, Set<(p: never) => void>>();
  private statusTimer: ReturnType<typeof setInterval> | null = null;
  private jobTimer: ReturnType<typeof setInterval> | null = null;
  private frameTimer: ReturnType<typeof setInterval> | null = null;
  private jogTimer: ReturnType<typeof setTimeout> | null = null;

  connected = false;
  port: string | null = null;
  /** La conexión es una máquina virtual (puerto `virtual:...`). */
  virtual = false;
  /** La conexión es con una controladora Ruida (sin G-code ni overrides). */
  ruida = false;
  state: MachineState = 'idle';
  /** Código de ALARM:N activo; null si no hay alarma. */
  alarmCode: number | null = null;
  pos: Point = [0, 0];
  feed = 0;
  spindle = 0;
  /** Overrides en vivo (%); solo GRBL, como en el motor real. */
  ovr = { feed: 100, rapid: 100, spindle: 100 };
  /** Ajustes de GRBL (`$n`) en memoria: `$$` los lista y `$n=v` los cambia (como la EEPROM de la controladora). */
  private grblSettings = new Map<number, string>(MOCK_GRBL_SETTINGS);

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

  connect(port: string, ruida = false): string {
    if (this.connected) throw new Error('Ya hay una máquina conectada. Desconéctala antes de abrir otro puerto.');
    this.connected = true;
    this.port = port;
    this.virtual = port.startsWith(VIRTUAL_PREFIX) || port.startsWith(VIRTUAL_NET_PREFIX);
    this.ruida = ruida || port.startsWith(MOCK_RUIDA_PORT);
    this.state = 'idle';
    this.alarmCode = null;
    this.pos = [0, 0];
    this.ovr = { feed: 100, rapid: 100, spindle: 100 };
    const firmware = this.ruida ? RUIDA_FIRMWARE : this.virtual ? (VIRTUAL_FIRMWARE[port.slice(port.indexOf(':') + 1).split('@')[0]!] ?? 'Grbl 1.1h') : 'Grbl 1.1h';
    const evt: ConnectionEvent = { connected: true, port, error: null };
    this.emit('machine://connection', evt);
    if (this.virtual) this.log('in', '; Máquina virtual (emulador): no hay láser ni hardware reales.');
    this.log('in', this.ruida ? 'Ruida RDC6442S (tarjeta 0x65106510), magic 0x88 (detectado), cama 600.0 × 400.0 mm' : "Grbl 1.1h ['$' for help]");
    this.statusTimer = setInterval(() => this.emitStatus(), this.opts.statusIntervalMs);
    this.emitStatus();
    return firmware;
  }

  disconnect(error: string | null = null): void {
    this.stopTimers();
    this.job = null;
    this.alarmCode = null;
    if (!this.connected) return;
    this.connected = false;
    const port = this.port;
    this.port = null;
    this.virtual = false;
    this.ruida = false;
    this.state = 'idle';
    this.emit('machine://connection', { connected: false, port, error });
  }

  /** Provoca una falla, solo con una máquina virtual (igual que el motor real). */
  simulateFault(fault: SimulatedFault): void {
    this.requireConnection();
    if (!this.virtual) throw new Error('Simular fallas solo es posible con una máquina virtual; esta es una máquina real.');
    if (fault === 'disconnect') {
      this.disconnect('se perdió la conexión con la máquina: la controladora emulada se desconectó');
      return;
    }
    const wasRunning = this.job !== null;
    if (fault === 'alarm3') {
      this.triggerAlarm(3);
      if (wasRunning) this.emit('machine://job', { state: 'error', message: 'ALARM:3: reinicio durante un movimiento: se perdió la posición.', position_lost: true });
      return;
    }
    const [code, text] = fault === 'flame' ? ['05', 'llama detectada: la máquina se detuvo'] : ['01', 'ángulo anormal: el sensor de inclinación o vibración se activó'];
    this.log('in', `ERROR:${code}: ${text}`);
    this.clearJob();
    this.emit('machine://job', { state: 'error', message: `Falla de la máquina ERROR:${code}: ${text}. El trabajo se detuvo (feed hold y soft reset).`, position_lost: false });
    this.emitStatus();
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
      ...(this.hasOverrides() ? { overrides: { ...this.ovr } } : {}),
    };
  }

  /** Marlin y Smoothieware no tienen overrides en tiempo real. */
  private hasOverrides(): boolean {
    return !this.ruida && !/^virtual:(marlin|smoothie)/.test(this.port ?? '');
  }

  /** Lleva el override de velocidad o potencia a `percent` (10 a 200), como los bytes 0x90-0x9D de GRBL. */
  setOverride(kind: OverrideKind, percent: number): void {
    this.requireConnection();
    if (this.ruida) throw new Error(RUIDA_GRBL_ONLY);
    if (!this.hasOverrides()) {
      throw new Error('Los ajustes en vivo de velocidad y potencia solo existen en controladoras GRBL: Marlin y Smoothieware no los tienen.');
    }
    const value = Math.min(200, Math.max(10, Math.round(percent)));
    this.ovr[kind] = value;
    this.log('out', `; override de ${kind === 'feed' ? 'velocidad (feed)' : 'potencia (spindle)'} al ${value} %`);
    this.emitStatus();
  }

  emitStatus(): void {
    if (this.connected) this.emit('machine://status', this.status());
  }

  // ---- comandos de línea ------------------------------------------------

  command(line: string): string[] {
    this.requireConnection();
    if (this.ruida) throw new Error(RUIDA_GRBL_ONLY);
    if (this.job) throw new Error(BUSY_MESSAGE);
    const trimmed = line.trim();
    if (/[?!~]/.test(trimmed)) {
      throw new Error(`'${trimmed}' contiene un carácter de tiempo real. Para pausar, reanudar o reiniciar usa los botones de la pestaña Máquina.`);
    }
    if (trimmed === '') throw new Error('La línea está vacía.');
    this.log('out', trimmed);
    let reply: string[];
    if (trimmed === '$I') reply = ['[VER:1.1h.20190825:Tlaku simulada]', '[OPT:V,15,128]', 'ok'];
    else if (trimmed === '$$') reply = [...[...this.grblSettings].sort((a, b) => a[0] - b[0]).map(([n, v]) => `$${n}=${v}`), 'ok'];
    else if (/^\$\d+=/.test(trimmed)) reply = [this.writeSetting(trimmed)];
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

  /** `$n=v`: guarda el ajuste si existe y el valor es un número no negativo; si no, contesta como GRBL. */
  private writeSetting(line: string): string {
    const m = /^\$(\d+)=(.*)$/.exec(line)!;
    const n = Number(m[1]);
    if (!this.grblSettings.has(n)) return describeLine('error:3');
    if (!/^-?\d+(\.\d+)?$/.test(m[2]!)) return describeLine('error:2');
    const v = Number(m[2]);
    if (v < 0) return describeLine('error:4');
    // Los de paso/avance/recorrido y los decimales se informan con tres decimales, como GRBL.
    this.grblSettings.set(n, n >= 100 || [11, 12, 24, 25, 27].includes(n) ? v.toFixed(3) : String(v));
    return 'ok';
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

  /** Eje extra (Z, U...): relativo (`delta`) o absoluto. La posición de estos ejes no se informa en el estado. */
  jogAxis(axis: string, value: number, speed: number, relative: boolean): void {
    this.requireConnection();
    if (this.job) throw new Error(BUSY_MESSAGE);
    if (!/^[ZUABCzuabc]$/.test(axis)) throw new Error(`El eje '${axis}' no es válido; usa uno de ZUABC.`);
    if (!Number.isFinite(value)) throw new Error(relative ? 'El desplazamiento no es un número.' : 'La posición de destino no es un número.');
    if (!(speed > 0)) throw new Error('La velocidad del movimiento debe ser mayor que 0.');
    this.log('out', `$J=${relative ? 'G91' : 'G90'} G21 ${axis.toUpperCase()}${value} F${speed}`);
    if (this.alarmCode !== null) {
      const reply = describeLine('error:9');
      this.log('in', reply);
      throw new Error(`La máquina rechazó el movimiento del eje ${axis.toUpperCase()} (${reply}).`);
    }
    this.log('in', 'ok');
  }

  /** Disparo de prueba: M3, pausa y M5 (siempre). */
  fire(powerPct: number, sMax: number, durationMs: number): void {
    this.requireConnection();
    if (this.job) throw new Error(BUSY_MESSAGE);
    if (!(powerPct > 0 && powerPct <= 20)) throw new Error('La potencia del disparo de prueba debe estar entre 0 y 20 %.');
    if (!(durationMs >= 10 && durationMs <= 2000)) throw new Error('La duración del disparo debe estar entre 10 y 2000 ms.');
    this.log('out', `M3 S${Math.round((sMax * powerPct) / 10) / 10}`);
    this.log('out', `G4 P${durationMs / 1000}`);
    this.log('out', 'M5');
    this.log('in', 'ok');
  }

  /** Movimiento absoluto en coordenadas de trabajo (`$J=G90`), con el láser apagado. */
  jogTo(x: number, y: number, bed: Point, speed = 1500): void {
    this.requireConnection();
    if (this.job) throw new Error(BUSY_MESSAGE);
    if (!Number.isFinite(x) || !Number.isFinite(y)) throw new Error('La posición de destino no es un número.');
    if (!(speed > 0)) throw new Error('La velocidad del movimiento debe ser mayor que 0.');
    this.log('out', `$J=G90 G21 X${x} Y${y} F${speed}`);
    if (this.alarmCode !== null) {
      const reply = describeLine('error:9');
      this.log('in', reply);
      throw new Error(`La máquina rechazó el movimiento (${reply}). Si la máquina está en alarma, desbloquéala con $X o haz homing ($H) desde la consola.`);
    }
    this.pos = [Math.min(bed[0], Math.max(0, x)), Math.min(bed[1], Math.max(0, y))];
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

  // ---- Ruida ------------------------------------------------------------

  /** Documentos guardados en la memoria de la controladora Ruida simulada. */
  readonly storedDocs: string[] = [];

  /** Ruida sin «ejecutar al subir»: el trabajo queda como documento y no arranca. */
  storeDocument(): void {
    this.requireConnection();
    if (this.job) throw new Error(BUSY_MESSAGE);
    const name = `TLAKU${String(this.storedDocs.length + 1).padStart(3, '0')}`;
    this.storedDocs.push(name);
    this.emit('machine://job', { state: 'started', message: 'Enviando el trabajo (1 paquetes).', position_lost: false });
    this.emit('machine://progress', { acked: 1, total: 1 });
    this.emit('machine://job', {
      state: 'finished',
      message: `Documento «${name}» guardado en la memoria de la controladora. No se ejecutó: selecciónalo e inícialo desde el panel de la máquina.`,
      position_lost: false,
    });
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
    this.emit('machine://job', { state: 'started', message: `${total} líneas`, position_lost: false } satisfies JobEvent);
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
    this.emit('machine://job', { state: 'finished', message: 'Trabajo terminado.', position_lost: false });
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
    this.emit('machine://job', { state: paused ? 'paused' : 'started', message: paused ? 'En pausa.' : 'Reanudado.', position_lost: false });
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
    this.emit('machine://job', { state: 'cancelled', message, position_lost: false });
    this.emitStatus();
  }

  /** Recorre el rectángulo del trabajo con el láser apagado. */
  frame(bounds: [Point, Point]): void {
    const [[x0, y0], [x1, y1]] = bounds;
    this.walk([[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]], `encuadre ${x0.toFixed(1)},${y0.toFixed(1)} a ${x1.toFixed(1)},${y1.toFixed(1)}`);
  }

  /** Encuadre de banda elástica: recorre el casco convexo de los movimientos `[kind, x, y, power]`. */
  frameHull(moves: number[]): void {
    const points: Point[] = [];
    for (let i = 0; i + 3 < moves.length; i += 4) points.push([moves[i + 1]!, moves[i + 2]!]);
    const hull = convexHull(points);
    if (hull.length === 0) {
      this.requireConnection();
      throw new Error('No hay nada que encuadrar: el trabajo está vacío.');
    }
    this.walk([...hull, hull[0]!], `encuadre del contorno (${hull.length} vértices)`);
  }

  private walk(corners: Point[], note: string): void {
    this.requireConnection();
    if (this.job) throw new Error(BUSY_MESSAGE);
    this.preflight();
    let step = 0;
    this.state = 'run';
    this.feed = 6000;
    this.log('out', `; ${note}`);
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
