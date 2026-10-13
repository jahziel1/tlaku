import { getApi } from '@/api';
import type { ConnectionEvent, JobEvent, LogEvent, OverrideKind, Point, ProgressEvent, RealtimeCommand, SimulatedFault, StatusReport } from '@/api/types';
import { firmwareFamily, parseAlarmLine, type AlarmAction, type FirmwareFamily } from '@/features/machine/grblCodes';
import { clampJogSpeed, continuousDelta, DEFAULT_JOG_SPEED, DEFAULT_JOG_STEP, stepDelta, type JogDir } from '@/features/machine/jogMath';
import { readFinish, readJogSteps, sanitizeFinish, sanitizeJogSteps, writeFinish, writeJogSteps, type FinishPosition } from '@/features/machine/moveSettings';
import { advanceAfterJob } from '@/features/variables/advance';
import { resolveJobArgs } from '@/features/variables/jobResolve';
import { needsConfirmation, parseCommand, type CommandError } from '@/features/machine/commandSafety';
import { DEFAULT_VIRTUAL_SPEED, isVirtualPort, portKey, VIRTUAL_SPEEDS, withVirtualSpeed } from '@/features/machine/virtualPort';
import { t, type I18nKey } from '@/i18n';
import { docToWork, parseGrblSettings, workToDoc, type GrblSettings } from '@/lib/device';
import { errorMessage } from '@/lib/errors';
import { fmt, readPref, writePref } from '@/lib/platform';
import { buildJobArgs, hasOutput, isJobBusy, selectActiveDevice } from './selectors';
import type { AppState, SliceCreator } from './types';

export type JobUiState = 'idle' | 'running' | 'paused' | 'finished' | 'cancelled' | 'error';

export const LOG_LIMIT = 500;
/** Velocidad de encuadre de respaldo; normalmente se usa la velocidad de jog elegida. */
export const FRAME_SPEED = 6000;

/** Línea del registro con un id estable (la lista se recorta por delante). */
export interface LogEntry extends LogEvent {
  id: number;
}

/** Diálogos de la máquina. Los muestra `MachineDialogHost` y los abren comandos y paneles. */
export type MachineDialog =
  /** "¿Láser listo y área despejada?" antes de iniciar. */
  | { kind: 'start' }
  /** Confirmación de `$X` (desbloquear) o `$H` (home). */
  | { kind: 'alarm'; action: AlarmAction }
  /** Gestor de dispositivos. */
  | { kind: 'devices' }
  /** Ajustes de optimización del orden de corte (`features/preview/OptimizationDialog.tsx`). */
  | { kind: 'optimization' };

// ---------------------------------------------------------------------------
// Reducer: función pura que aplica un evento `machine://*` a los datos de la máquina
// ---------------------------------------------------------------------------

export interface MachineData {
  connection: ConnectionEvent;
  firmware: string | null;
  status: StatusReport | null;
  progress: ProgressEvent | null;
  jobState: JobUiState;
  jobMessage: string;
  /** Mensaje del motor cuando rechazó iniciar o encuadrar (por ejemplo el preflight), tal cual. */
  jobError: string | null;
  /** Último `ALARM:N` recibido (null si no se conoce la causa). Solo vale mientras el estado sea alarma. */
  alarmCode: number | null;
  /** Ajustes de GRBL leídos con `$$` (null = todavía no). */
  settings: GrblSettings | null;
  /**
   * La posición puede haberse perdido (un reset con la máquina en movimiento, `ALARM:3`). Sobrevive
   * a la desconexión para avisar al reconectar a la misma máquina; se borra tras un homing, al
   * fijar el origen de nuevo o al descartar el aviso.
   */
  positionLost: boolean;
  /** Máquina (puerto sin la velocidad virtual) a la que corresponde `positionLost`. */
  positionLostPort: string | null;
  log: LogEntry[];
  logSeq: number;
  jobStartedAt: number | null;
  jobPausedAt: number | null;
  jobPausedMs: number;
  jobEndedAt: number | null;
}

export type MachineEvent =
  | { type: 'status'; report: StatusReport }
  | { type: 'progress'; progress: ProgressEvent }
  | { type: 'log'; log: LogEvent }
  | { type: 'job'; job: JobEvent }
  | { type: 'connection'; connection: ConnectionEvent }
  | { type: 'settings'; settings: GrblSettings | null };

const DISCONNECTED: ConnectionEvent = { connected: false, port: null, error: null };

export const INITIAL_MACHINE_DATA: MachineData = {
  connection: DISCONNECTED,
  firmware: null,
  status: null,
  progress: null,
  jobState: 'idle',
  jobMessage: '',
  jobError: null,
  alarmCode: null,
  settings: null,
  positionLost: false,
  positionLostPort: null,
  log: [],
  logSeq: 0,
  jobStartedAt: null,
  jobPausedAt: null,
  jobPausedMs: 0,
  jobEndedAt: null,
};

const JOB_STATE: Record<JobEvent['state'], JobUiState> = {
  started: 'running',
  paused: 'paused',
  finished: 'finished',
  cancelled: 'cancelled',
  error: 'error',
};

const busy = (j: JobUiState) => j === 'running' || j === 'paused';

/** Marca la posición como posiblemente perdida en la máquina conectada. */
function lostHere(m: MachineData): Pick<MachineData, 'positionLost' | 'positionLostPort'> {
  return { positionLost: true, positionLostPort: portKey(m.connection.port) ?? m.positionLostPort };
}

/** Aplica un evento a los datos de la máquina. `now` (ms) se pasa de fuera para que sea determinista. */
export function reduceMachine(m: MachineData, ev: MachineEvent, now: number): MachineData {
  switch (ev.type) {
    case 'status': {
      const state = ev.report.state;
      // Una alarma con un trabajo en curso lo da por fallido aunque el motor no avise.
      const failed = state === 'alarm' && busy(m.jobState);
      // La causa solo se olvida al ver que la alarma terminó (no con un reporte suelto).
      const prev = m.status?.state;
      const left = (prev === 'alarm' || prev === 'home') && state !== 'alarm' && state !== 'home';
      // Un homing que termina sin alarma vuelve a dar la posición de máquina.
      const homed = prev === 'home' && state !== 'alarm' && state !== 'home';
      return {
        ...m,
        status: ev.report,
        alarmCode: left ? null : m.alarmCode,
        ...(homed ? { positionLost: false, positionLostPort: null } : {}),
        ...(failed ? { jobState: 'error' as const, jobPausedAt: null, jobEndedAt: now } : {}),
      };
    }
    case 'progress':
      return { ...m, progress: ev.progress };
    case 'log': {
      const id = m.logSeq + 1;
      const code = ev.log.dir === 'in' ? parseAlarmLine(ev.log.text) : null;
      return {
        ...m,
        log: [...m.log, { ...ev.log, id }].slice(-LOG_LIMIT),
        logSeq: id,
        alarmCode: code ?? m.alarmCode,
        // ALARM:3 es justo eso: reinicio con la máquina en movimiento.
        ...(code === 3 ? lostHere(m) : {}),
      };
    }
    case 'job': {
      const state = JOB_STATE[ev.job.state];
      const next: MachineData = { ...m, jobState: state, jobMessage: ev.job.message, ...(ev.job.position_lost ? lostHere(m) : {}) };
      if (ev.job.state === 'started') {
        if (m.jobState === 'paused') {
          // Reanudar: la pausa no cuenta como tiempo de trabajo.
          next.jobPausedMs = m.jobPausedMs + (m.jobPausedAt === null ? 0 : now - m.jobPausedAt);
          next.jobPausedAt = null;
        } else if (!busy(m.jobState)) {
          Object.assign(next, { progress: null, jobError: null, jobStartedAt: now, jobPausedAt: null, jobPausedMs: 0, jobEndedAt: null });
        }
      } else if (ev.job.state === 'paused') {
        next.jobPausedAt = now;
      } else {
        next.jobPausedAt = null;
        next.jobEndedAt = now;
      }
      return next;
    }
    case 'connection': {
      if (ev.connection.connected) {
        // El aviso de posición perdida solo vale para la misma máquina.
        const same = m.positionLost && m.positionLostPort === portKey(ev.connection.port);
        return {
          ...m,
          connection: ev.connection,
          alarmCode: null,
          settings: null,
          jobError: null,
          positionLost: same,
          positionLostPort: same ? m.positionLostPort : null,
        };
      }
      // Sin conexión no queda nada vivo: se conserva el registro y la posición perdida.
      return {
        ...INITIAL_MACHINE_DATA,
        connection: ev.connection,
        log: m.log,
        logSeq: m.logSeq,
        positionLost: m.positionLost,
        positionLostPort: m.positionLostPort,
      };
    }
    case 'settings':
      return { ...m, settings: ev.settings };
  }
}

/** Segundos de trabajo transcurridos sin contar las pausas. */
export function jobElapsedSeconds(m: Pick<MachineData, 'jobStartedAt' | 'jobPausedAt' | 'jobPausedMs' | 'jobEndedAt'>, now: number): number {
  if (m.jobStartedAt === null) return 0;
  const end = m.jobEndedAt ?? m.jobPausedAt ?? now;
  return Math.max(0, (end - m.jobStartedAt - m.jobPausedMs) / 1000);
}

// ---------------------------------------------------------------------------
// Selectores
// ---------------------------------------------------------------------------

/** Posición del cabezal en coordenadas del documento (mm, abajo a la izquierda, Y arriba), o null. */
export function selectHeadDoc(s: Pick<AppState, 'status' | 'connection' | 'devices' | 'activeDeviceId'>): Point | null {
  if (!s.connection.connected || !s.status) return null;
  return workToDoc(selectActiveDevice(s), s.status.work_pos);
}

/**
 * Causa de la alarma en curso: `undefined` si no hay alarma, `code: null` si no se conoce.
 * Devuelve un objeto nuevo: en componentes selecciona `status?.state` y `alarmCode` por separado.
 */
export function selectAlarm(s: Pick<AppState, 'status' | 'alarmCode'>): { code: number | null } | undefined {
  return s.status?.state === 'alarm' ? { code: s.alarmCode } : undefined;
}

/** Familia del firmware conectado: decide cómo se leen los `error:N` y `ALARM:N` extendidos. */
export function selectFirmwareFamily(s: Pick<AppState, 'firmware'>): FirmwareFamily {
  return firmwareFamily(s.firmware);
}

/** ¿La conexión actual es una máquina virtual (emulador)? Nada se graba de verdad. */
export function selectIsVirtual(s: Pick<AppState, 'connection'>): boolean {
  return s.connection.connected && isVirtualPort(s.connection.port);
}

/** Valor de `$22` (homing) si se leyó: true = activado. */
export function homingEnabled(settings: GrblSettings | null): boolean | null {
  const v = settings?.[22];
  return v === undefined ? null : v !== 0;
}

/** `null` si se puede mover el cabezal a mano; si no, la clave del motivo. */
export function jogBlockedReason(s: Pick<AppState, 'connection' | 'jobState' | 'status'>): I18nKey | null {
  if (!s.connection.connected) return 'reason.needMachine';
  if (isJobBusy(s)) return 'reason.jobRunning';
  const state = s.status?.state;
  if (state === 'alarm') return 'machine.reason.alarm';
  if (state === 'hold' || state === 'door') return 'machine.reason.paused';
  if (state === 'run' || state === 'home' || state === 'check' || state === 'sleep' || state === 'other') return 'machine.reason.notIdle';
  return null;
}

/**
 * `null` si se puede posicionar el láser con un clic; si no, la clave del motivo. Además de poder
 * mover el cabezal, hace falta una cama plana: con rotativo la Y es el giro de la pieza.
 */
export function positionLaserBlockedReason(s: Pick<AppState, 'connection' | 'jobState' | 'status' | 'devices' | 'activeDeviceId'>): I18nKey | null {
  const jog = jogBlockedReason(s);
  if (jog) return jog;
  return selectActiveDevice(s).rotary.enabled ? 'machine.position.reason.rotary' : null;
}

/** ¿La controladora conectada admite overrides en tiempo real? Solo GRBL y derivados (Marlin y Smoothieware no). */
export function overridesSupported(s: Pick<AppState, 'firmware' | 'devices' | 'activeDeviceId'>): boolean {
  if (/marlin|smoothie/i.test(s.firmware ?? '')) return false;
  const kind = selectActiveDevice(s).kind;
  return kind !== 'marlin' && kind !== 'smoothie';
}

/** `null` si se pueden ajustar velocidad y potencia en vivo; si no, la clave del motivo. */
export function overrideBlockedReason(s: Pick<AppState, 'connection' | 'firmware' | 'devices' | 'activeDeviceId'>): I18nKey | null {
  if (!s.connection.connected) return 'reason.needMachine';
  return overridesSupported(s) ? null : 'machine.override.reason.family';
}

/** `null` si se puede iniciar o encuadrar; si no, la clave del motivo. */
export function sendBlockedReason(s: AppState): I18nKey | null {
  if (!s.connection.connected) return 'reason.needMachine';
  if (isJobBusy(s)) return 'reason.jobRunning';
  if (!hasOutput(s)) return 'reason.nothingToSend';
  const state = s.status?.state;
  if (state === 'alarm') return 'machine.reason.alarm';
  if (state === 'hold' || state === 'door') return 'machine.reason.paused';
  if (state && state !== 'idle') return 'machine.reason.notIdle';
  return null;
}

// ---------------------------------------------------------------------------
// Slice
// ---------------------------------------------------------------------------

export interface MachineSlice extends MachineData {
  connecting: boolean;
  ports: string[];
  selectedPort: string | null;
  baud: number;
  /** Veces el tiempo real con que corre una máquina virtual (se recuerda entre sesiones). */
  virtualSpeed: number;
  /** Paso del jog en mm y velocidad en mm/min (se recuerdan entre sesiones). */
  jogStep: number;
  jogSpeed: number;
  /** Pasos que ofrece la ventana Mover (configurables, se recuerdan entre sesiones). */
  jogSteps: number[];
  /** Qué hace el cabezal al terminar un trabajo (se recuerda entre sesiones). */
  finish: FinishPosition;
  /**
   * Modo "Posicionar el láser": el siguiente clic en la cama manda allí la cabeza, con el láser
   * apagado. Lo prende el comando `machine.positionLaser` y el lienzo, mientras esté activo, debe
   * llamar a `positionLaserAt` en cada clic (ver el informe de integración con el lienzo).
   */
  positioningLaser: boolean;
  /** Hay un jog continuo en marcha (se mantiene pulsado). */
  jogging: boolean;
  dialog: MachineDialog | null;
  /** Instancias montadas de `MachineDialogHost`; solo la primera dibuja. */
  dialogHosts: string[];

  refreshPorts: () => Promise<void>;
  setSelectedPort: (port: string | null) => void;
  setBaud: (baud: number) => void;
  setVirtualSpeed: (speed: number) => void;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  /** Lee `$$` y guarda los ajustes. Devuelve null si no se pudo. */
  readSettings: (opts?: { silent?: boolean }) => Promise<GrblSettings | null>;
  /**
   * Envía una línea con las mismas reglas que `tlaku cmd`: se normaliza (mayúsculas, sin
   * espacios ni comentarios) y todo lo que no sea de solo lectura exige `confirmed: true`, que
   * solo da quien ya mostró al usuario exactamente qué se enviará. Devuelve si se envió.
   */
  sendLine: (line: string, opts?: { confirmed?: boolean }) => Promise<boolean>;
  realtime: (cmd: RealtimeCommand) => Promise<void>;

  setJogStep: (step: number) => void;
  setJogSpeed: (speed: number) => void;
  /** Cambia los pasos de la ventana Mover; si el paso actual ya no está, pasa al más cercano. */
  setJogSteps: (steps: number[]) => void;
  setFinishPosition: (patch: Partial<FinishPosition>) => void;
  /** Lleva la cabeza a la posición final elegida (la usa el fin del trabajo; también se puede pedir a mano). */
  goToFinishPosition: () => Promise<void>;
  /** Mueve la cabeza a una posición del documento (mm, Y arriba), con el láser apagado. */
  goToDocPoint: (x: number, y: number) => Promise<boolean>;
  setPositioningLaser: (on: boolean) => void;
  /** Un clic en la cama en modo "Posicionar el láser": `x`, `y` en mm del documento (Y arriba). */
  positionLaserAt: (x: number, y: number) => Promise<void>;
  /** Lleva el override en vivo de velocidad o potencia a `percent` por ciento (10 a 200). */
  setOverride: (kind: OverrideKind, percent: number) => Promise<void>;
  jog: (dx: number, dy: number, speed?: number) => Promise<void>;
  /** Un paso de jog en una dirección del plano del documento. */
  jogDir: (dir: JogDir, step?: number) => Promise<void>;
  jogContinuousStart: (dir: JogDir) => Promise<void>;
  jogStop: () => Promise<void>;
  goToOrigin: () => Promise<void>;
  setUserOriginHere: () => void;
  clearUserOrigin: () => void;

  /** Abre el diálogo de confirmación de inicio (el trabajo no sale hasta confirmarlo). */
  requestStart: () => void;
  confirmStart: () => Promise<void>;
  startJob: () => Promise<void>;
  pauseJob: () => Promise<void>;
  resumeJob: () => Promise<void>;
  cancelJob: () => Promise<void>;
  frameJob: () => Promise<void>;
  /** Encuadre de banda elástica: sigue el casco convexo del trabajo en vez del rectángulo. */
  frameHull: () => Promise<void>;
  dismissJobError: () => void;
  /** Descarta el aviso de posición posiblemente perdida. */
  dismissPositionLost: () => void;
  /** Provoca una falla en la máquina virtual conectada. */
  simulateFault: (fault: SimulatedFault) => Promise<void>;
  clearLog: () => void;

  openDialog: (dialog: MachineDialog) => void;
  closeDialog: () => void;
  registerDialogHost: (id: string) => () => void;
  /** Envía `$X` o `$H` después de que el usuario lo confirmó en el diálogo. */
  confirmAlarmAction: (action: AlarmAction) => Promise<void>;

  // Entrada de eventos `machine://*` (los conecta `bindMachineEvents`).
  onStatus: (s: StatusReport) => void;
  onProgress: (p: ProgressEvent) => void;
  onLog: (l: LogEvent) => void;
  onJob: (j: JobEvent) => void;
  onConnection: (c: ConnectionEvent) => void;
}

function pickData(s: MachineData): MachineData {
  return {
    connection: s.connection,
    firmware: s.firmware,
    status: s.status,
    progress: s.progress,
    jobState: s.jobState,
    jobMessage: s.jobMessage,
    jobError: s.jobError,
    alarmCode: s.alarmCode,
    settings: s.settings,
    positionLost: s.positionLost,
    positionLostPort: s.positionLostPort,
    log: s.log,
    logSeq: s.logSeq,
    jobStartedAt: s.jobStartedAt,
    jobPausedAt: s.jobPausedAt,
    jobPausedMs: s.jobPausedMs,
    jobEndedAt: s.jobEndedAt,
  };
}

/** Texto en español de un comando de consola rechazado. */
export function commandErrorText(e: CommandError): string {
  switch (e.code) {
    case 'realtimeChar':
      return t('machine.console.err.realtimeChar', { ch: e.ch });
    case 'empty':
      return t('machine.console.err.empty');
    case 'tooLong':
      return t('machine.console.err.tooLong', { text: e.text, max: e.max });
  }
}

function initialVirtualSpeed(): number {
  const n = Number(readPref('virtualSpeed'));
  return (VIRTUAL_SPEEDS as readonly number[]).includes(n) ? n : DEFAULT_VIRTUAL_SPEED;
}

/** El paso guardado, si sigue en la lista; si no, el más cercano al de fábrica. */
function nearestStep(steps: readonly number[], wanted: number): number {
  return steps.includes(wanted) ? wanted : steps.reduce((best, s) => (Math.abs(s - wanted) < Math.abs(best - wanted) ? s : best), steps[0] ?? DEFAULT_JOG_STEP);
}

function initialJogStep(steps: readonly number[]): number {
  const n = Number(readPref('jogStep'));
  return nearestStep(steps, Number.isFinite(n) && n > 0 ? n : DEFAULT_JOG_STEP);
}

function initialJogSpeed(): number {
  const n = Number(readPref('jogSpeed'));
  return Number.isFinite(n) && n > 0 ? n : DEFAULT_JOG_SPEED;
}

const round3 = (n: number) => Math.round(n * 1000) / 1000;

/** Espera `ms` milisegundos. */
const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

/** Cuánto se espera a que la máquina quede en reposo para moverla a la posición final. */
const FINISH_WAIT = { tries: 25, everyMs: 200 } as const;

/** Espera a que la máquina quede en reposo (o se agote la espera) y entonces ejecuta `action`. */
async function waitIdleThen(get: () => AppState, action: () => Promise<void>): Promise<void> {
  for (let i = 0; i < FINISH_WAIT.tries; i++) {
    const s = get();
    if (!s.connection.connected || get().jobState !== 'finished') return;
    if (!s.status || s.status.state === 'idle') return action();
    await sleep(FINISH_WAIT.everyMs);
  }
}

export const createMachineSlice: SliceCreator<MachineSlice> = (set, get) => {
  const apply = (ev: MachineEvent) => set((s) => reduceMachine(pickData(s), ev, Date.now()));

  /** Ejecuta una llamada al motor y convierte el rechazo en un aviso. Devuelve si salió bien. */
  const guard = async (fn: () => Promise<unknown>): Promise<boolean> => {
    try {
      await fn();
      return true;
    } catch (e) {
      get().notify('error', errorMessage(e));
      return false;
    }
  };

  /** Mensaje del motor al rechazar un inicio o un encuadre: se muestra tal cual, también en el panel. */
  const guardJob = async (fn: () => Promise<unknown>): Promise<void> => {
    // Salir del modo "Posicionar el láser": un clic en la cama no debe mover la cabeza durante el trabajo.
    set({ jobError: null, positioningLaser: false });
    try {
      await fn();
    } catch (e) {
      const message = errorMessage(e);
      set({ jobError: message });
      get().notify('error', message);
    }
  };

  const ensureDialogHost = () => {
    // Sin un `MachineDialogHost` montado (el AppShell lo monta siempre) el diálogo no se vería: se muestra la máquina.
    if (get().dialogHosts.length === 0) get().setDockTab('machine');
  };

  const jogSpeedNow = () => clampJogSpeed(get().jogSpeed, selectActiveDevice(get()));

  /** Corta el jog en curso con jog cancel (0x85): GRBL frena y descarta el movimiento pendiente. */
  const cancelJogMotion = async () => {
    try {
      await getApi().machineRealtime({ cmd: 'jog_cancel' });
    } catch (e) {
      get().notify('error', errorMessage(e));
    }
  };

  return {
    ...INITIAL_MACHINE_DATA,
    connecting: false,
    ports: [],
    selectedPort: null,
    baud: 115200,
    virtualSpeed: initialVirtualSpeed(),
    ...(() => {
      const jogSteps = readJogSteps();
      return { jogSteps, jogStep: initialJogStep(jogSteps) };
    })(),
    jogSpeed: initialJogSpeed(),
    finish: readFinish(),
    positioningLaser: false,
    jogging: false,
    dialog: null,
    dialogHosts: [],

    refreshPorts: async () => {
      await guard(async () => {
        const ports = await getApi().listPorts();
        set((s) => ({ ports, selectedPort: s.selectedPort && ports.includes(s.selectedPort) ? s.selectedPort : (ports[0] ?? null) }));
      });
    },
    setSelectedPort: (selectedPort) => set({ selectedPort }),
    setBaud: (baud) => set({ baud }),
    setVirtualSpeed: (virtualSpeed) => {
      writePref('virtualSpeed', String(virtualSpeed));
      set({ virtualSpeed });
    },

    connect: async () => {
      const { selectedPort, baud, virtualSpeed } = get();
      if (!selectedPort) {
        get().notifyKey('warning', 'machine.noPort');
        return;
      }
      set({ connecting: true });
      try {
        // La velocidad de una máquina virtual viaja en el id del puerto (`virtual:falcon2@5`).
        const port = isVirtualPort(selectedPort) ? withVirtualSpeed(selectedPort, virtualSpeed) : selectedPort;
        const { firmware } = await getApi().connect({ port, baud });
        set({ firmware });
        // Mejor esfuerzo: sirve para el aviso de modo láser y para sugerir $H o $X ante una alarma.
        void get().readSettings({ silent: true });
      } catch (e) {
        get().notify('error', errorMessage(e));
      } finally {
        set({ connecting: false });
      }
    },
    disconnect: async () => {
      if (get().jogging) await get().jogStop();
      await guard(() => getApi().disconnect());
    },

    readSettings: async (opts) => {
      if (!get().connection.connected) return null;
      if (isJobBusy(get())) {
        if (!opts?.silent) get().notifyKey('warning', 'reason.jobRunning');
        return null;
      }
      try {
        const settings = parseGrblSettings(await getApi().machineCommand({ line: '$$' }));
        if (Object.keys(settings).length === 0) throw new Error(t('machine.settings.empty'));
        apply({ type: 'settings', settings });
        return settings;
      } catch (e) {
        if (!opts?.silent) get().notify('error', errorMessage(e));
        return null;
      }
    },

    sendLine: async (line, opts) => {
      const s = get();
      if (!s.connection.connected) {
        s.notifyKey('warning', 'reason.needMachine');
        return false;
      }
      const parsed = parseCommand(line, selectActiveDevice(s).rx_buffer);
      if (!parsed.ok) {
        s.notify('error', commandErrorText(parsed.error));
        return false;
      }
      const { cmd } = parsed;
      // Misma regla que el CLI: sin confirmación explícita solo sale lo de solo lectura.
      if (needsConfirmation(cmd.risk) && !opts?.confirmed) {
        s.notifyKey('warning', 'machine.console.needsConfirm');
        return false;
      }
      if (cmd.kind === 'line' && isJobBusy(s)) {
        s.notifyKey('warning', 'machine.console.jobBusy');
        return false;
      }
      const api = getApi();
      // Marlin y Smoothie separan parámetros con espacios (`M3 S0 I`): se manda la línea tal cual
      // escribió el usuario y el motor quita solo los comentarios.
      const keepSpaces = /marlin|smoothie/i.test(s.firmware ?? '');
      const text = cmd.kind === 'line' && keepSpaces ? line.trim() : cmd.kind === 'line' ? cmd.text : '';
      return guard(() => (cmd.kind === 'realtime' ? api.machineRealtime({ cmd: cmd.name }) : api.machineCommand({ line: text })));
    },
    realtime: async (cmd) => {
      await guard(() => getApi().machineRealtime({ cmd }));
    },

    setJogStep: (jogStep) => {
      writePref('jogStep', String(jogStep));
      set({ jogStep });
    },
    setJogSpeed: (speed) => {
      const jogSpeed = clampJogSpeed(speed, selectActiveDevice(get()));
      writePref('jogSpeed', String(jogSpeed));
      set({ jogSpeed });
    },

    setJogSteps: (steps) => {
      const jogSteps = sanitizeJogSteps(steps);
      writeJogSteps(jogSteps);
      const jogStep = nearestStep(jogSteps, get().jogStep);
      writePref('jogStep', String(jogStep));
      set({ jogSteps, jogStep });
    },
    setFinishPosition: (patch) => {
      const finish = sanitizeFinish({ ...get().finish, ...patch });
      writeFinish(finish);
      set({ finish });
    },
    goToDocPoint: async (x, y) => {
      const s = get();
      const why = positionLaserBlockedReason(s);
      if (why) {
        s.notify('warning', t(why));
        return false;
      }
      const device = selectActiveDevice(s);
      if (!Number.isFinite(x) || !Number.isFinite(y) || x < 0 || y < 0 || x > device.bed[0] || y > device.bed[1]) {
        s.notify('warning', t('machine.position.outside', { x: fmt(x, 1), y: fmt(y, 1), w: fmt(device.bed[0], 0), h: fmt(device.bed[1], 0) }));
        return false;
      }
      const [wx, wy] = docToWork(device, [x, y]);
      return guard(() => getApi().jogTo({ x: round3(wx), y: round3(wy), speed: jogSpeedNow() }));
    },
    goToFinishPosition: async () => {
      const s = get();
      const { mode, x, y } = s.finish;
      if (mode === 'stay') return;
      if (mode === 'point') {
        await s.goToDocPoint(x, y);
        return;
      }
      // Volver al origen: el de usuario si lo hay; si no, el cero de trabajo de la máquina.
      const target = s.jobOptions.user_origin ?? [0, 0];
      const why = jogBlockedReason(s);
      if (why) return void s.notify('warning', t(why));
      await guard(() => getApi().jogTo({ x: target[0], y: target[1], speed: jogSpeedNow() }));
    },
    setPositioningLaser: (on) => {
      if (!on) return void set({ positioningLaser: false });
      const why = positionLaserBlockedReason(get());
      if (why) return void get().notify('warning', t(why));
      set({ positioningLaser: true });
    },
    positionLaserAt: async (x, y) => {
      if (!get().positioningLaser) return;
      await get().goToDocPoint(x, y);
    },
    setOverride: async (kind, percent) => {
      const why = overrideBlockedReason(get());
      if (why) return void get().notify('warning', t(why));
      await guard(() => getApi().setOverride({ kind, percent: Math.round(percent) }));
    },

    jog: async (dx, dy, speed) => {
      const why = jogBlockedReason(get());
      if (why) return void get().notify('warning', t(why));
      await guard(() => getApi().jog({ dx, dy, speed: speed ?? jogSpeedNow() }));
    },
    jogDir: async (dir, step) => {
      const [dx, dy] = stepDelta(selectActiveDevice(get()), dir, step ?? get().jogStep);
      await get().jog(dx, dy);
    },
    jogContinuousStart: async (dir) => {
      const s = get();
      if (s.jogging) return;
      const why = jogBlockedReason(s);
      if (why) return void s.notify('warning', t(why));
      const delta = continuousDelta(selectActiveDevice(s), dir, s.status?.work_pos ?? null);
      if (!delta) return void s.notifyKey('info', 'machine.jog.noRoom');
      set({ jogging: true });
      try {
        await getApi().jog({ dx: delta[0], dy: delta[1], speed: jogSpeedNow() });
      } catch (e) {
        set({ jogging: false });
        get().notify('error', errorMessage(e));
      }
    },
    jogStop: async () => {
      if (!get().jogging) return;
      set({ jogging: false });
      await cancelJogMotion();
    },
    goToOrigin: async () => {
      const s = get();
      const here = s.status?.work_pos;
      if (!here) return void s.notifyKey('warning', 'reason.needMachine');
      const target = s.jobOptions.user_origin ?? [0, 0];
      const dx = round3(target[0] - here[0]);
      const dy = round3(target[1] - here[1]);
      if (Math.abs(dx) < 0.01 && Math.abs(dy) < 0.01) return void s.notifyKey('info', 'machine.origin.already');
      await s.jog(dx, dy);
    },
    setUserOriginHere: () => {
      const s = get();
      const here = s.status?.work_pos;
      if (!s.connection.connected || !here) return void s.notifyKey('warning', 'reason.needMachine');
      const user_origin: Point = [round3(here[0]), round3(here[1])];
      s.setJobOptions({ user_origin });
      // Fijar el origen a mano es una de las dos salidas de una posición perdida.
      s.dismissPositionLost();
      s.notify('success', t('machine.origin.saved', { x: user_origin[0], y: user_origin[1] }));
    },
    clearUserOrigin: () => get().setJobOptions({ user_origin: null }),

    requestStart: () => {
      const s = get();
      const why = sendBlockedReason(s);
      if (why) return void s.notify('warning', t(why));
      ensureDialogHost();
      // El diálogo muestra tiempo, capas y potencia: hace falta una vista previa al día.
      if (s.previewStatus !== 'ready') void s.computePreview();
      set({ dialog: { kind: 'start' } });
    },
    confirmStart: async () => {
      set({ dialog: null });
      await get().startJob();
    },
    startJob: () => guardJob(async () => getApi().startJob(await resolveJobArgs(buildJobArgs(get())))),
    pauseJob: async () => {
      await guard(() => getApi().pauseJob());
    },
    resumeJob: async () => {
      await guard(() => getApi().resumeJob());
    },
    cancelJob: async () => {
      if (get().jogging) await get().jogStop();
      await guard(() => getApi().cancelJob());
    },
    frameJob: () => guardJob(async () => getApi().frameJob({ ...(await resolveJobArgs(buildJobArgs(get()))), speed: jogSpeedNow() })),
    frameHull: () => guardJob(async () => getApi().frameHull({ ...(await resolveJobArgs(buildJobArgs(get()))), speed: jogSpeedNow() })),
    dismissJobError: () => set({ jobError: null }),
    dismissPositionLost: () => set({ positionLost: false, positionLostPort: null }),
    simulateFault: async (fault) => {
      if (!selectIsVirtual(get())) return void get().notifyKey('warning', 'machine.virtual.onlyVirtual');
      if (await guard(() => getApi().simulateFault({ fault }))) get().notifyKey('info', 'machine.virtual.faultSent');
    },
    clearLog: () => set({ log: [] }),

    openDialog: (dialog) => {
      ensureDialogHost();
      set({ dialog });
    },
    closeDialog: () => set({ dialog: null }),
    registerDialogHost: (id) => {
      set((s) => ({ dialogHosts: [...s.dialogHosts, id] }));
      return () => set((s) => ({ dialogHosts: s.dialogHosts.filter((h) => h !== id) }));
    },
    confirmAlarmAction: async (action) => {
      set({ dialog: null });
      await get().sendLine(action === 'unlock' ? '$X' : '$H', { confirmed: true });
    },

    onStatus: (report) => apply({ type: 'status', report }),
    onProgress: (progress) => apply({ type: 'progress', progress }),
    onLog: (log) => apply({ type: 'log', log }),
    onJob: (job) => {
      apply({ type: 'job', job });
      if (job.state === 'finished') {
        get().notify('success', job.message);
        // Texto variable: la serie y la fila del CSV pasan al siguiente trabajo (solo si terminó bien).
        advanceAfterJob({ getState: get });
        // Solo si el trabajo terminó bien: tras una cancelación o un error la cabeza no se mueve sola.
        if (get().finish.mode !== 'stay') void waitIdleThen(get, () => get().goToFinishPosition());
      }
      if (job.state === 'error') get().notify('error', job.message);
    },
    onConnection: (connection) => {
      const pending = get().positionLost && !get().connection.connected;
      apply({ type: 'connection', connection });
      // Reconectó a la misma máquina que quedó con la posición dudosa: conviene hacer homing.
      if (connection.connected && pending && get().positionLost) {
        get().notifyKey('warning', 'machine.positionLost.reconnect');
        apply({ type: 'log', log: { dir: 'in', text: `; ${t('machine.positionLost.reconnect')}` } });
      }
      if (!connection.connected) set((s) => ({ jogging: false, positioningLaser: false, dialog: s.dialog?.kind === 'devices' || s.dialog?.kind === 'optimization' ? s.dialog : null }));
      if (connection.error) get().notify('error', connection.error);
    },
  };
};
