/** Rebanada del store con la conexión, el trabajo, el jog y las acciones de la máquina. */
import { getApi } from '@/api';
import type { ConnectionEvent, GalvoSafetyAck, JobArgs, JobEvent, LogEvent, OverrideKind, Point, ProgressEvent, RealtimeCommand, SimulatedFault, StatusReport } from '@/api/types';
import { type AlarmAction } from '@/features/machine/grblCodes';
import { clampJogSpeed, continuousDelta, DEFAULT_JOG_SPEED, DEFAULT_JOG_STEP, jogOutsideBed, outsideBedMessage, stepDelta, type JogDir } from '@/features/machine/jogMath';
import { readFinish, readJogSteps, sanitizeFinish, sanitizeJogSteps, writeFinish, writeJogSteps, type FinishPosition } from '@/features/machine/moveSettings';
import { advanceAfterJob } from '@/features/variables/advance';
import { resolveJobArgs } from '@/features/variables/jobResolve';
import { needsConfirmation, parseCommand, type CommandError } from '@/features/machine/commandSafety';
import { connectionPortId } from '@/features/machine/DeviceConnectionModel';
import { isGalvoUsbPort } from '@/features/machine/galvoPorts';
import { defaultPort, portForDevice } from '@/features/machine/ruidaPorts';
import { DEFAULT_VIRTUAL_SPEED, isVirtualPort, VIRTUAL_SPEEDS, withVirtualSpeed } from '@/features/machine/virtualPort';
import { noteJobAlarm, noteJobEvent, useJobLog, type JobLogContext } from '@/features/help/jobLog';
import { t } from '@/i18n';
import { docToWork, isGrblFamily, parseGrblSettings, type GrblSettings } from '@/lib/device';
import { errorMessage, isStopUnconfirmed } from '@/lib/errors';
import { fmt, readPref, writePref } from '@/lib/platform';
import { buildJobArgs, isJobBusy, selectActiveDevice } from '../selectors';
import type { AppState, SliceCreator } from '../types';
import { INITIAL_MACHINE_DATA, jobElapsedSeconds, reduceMachine, type MachineData, type MachineDialog, type MachineEvent } from './reducer';
import { createResumeActions, type ResumeSlice } from './resume';
import { selectIsVirtual, jogBlockedReason, positionLaserBlockedReason, overrideBlockedReason, frameMarkOf, sendBlockedReason, type FrameMark } from './selectors';

// ---------------------------------------------------------------------------
// Slice
// ---------------------------------------------------------------------------

export interface MachineSlice extends MachineData, ResumeSlice {
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
  /** «Posicionar»: lleva el puntero rojo (en vez del rayo) al punto clicado, usando el desfase del perfil. */
  positionAtPointer: boolean;
  setPositionAtPointer: (on: boolean) => void;
  /** Lleva la cabeza a donde el puntero rojo cae en (`x`, `y`) del documento (rayo = punto - desfase). */
  goToPointerDocPoint: (x: number, y: number) => Promise<boolean>;
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
  /** `tweak` ajusta los argumentos antes de enviar (el asistente de tiling pone el tramo y las marcas). */
  startJob: (tweak?: (args: JobArgs) => JobArgs) => Promise<void>;
  pauseJob: () => Promise<void>;
  resumeJob: () => Promise<void>;
  cancelJob: () => Promise<void>;
  frameJob: () => Promise<void>;
  /** Encuadre de banda elástica: sigue el casco convexo del trabajo en vez del rectángulo. */
  frameHull: () => Promise<void>;
  /** Marca de "encuadrado tras el último cambio" (ver `selectFramed`); null = nunca se encuadró. */
  framedMark: FrameMark | null;
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
    stopUnconfirmed: s.stopUnconfirmed,
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

/** Argumento de potencia del encuadre: solo si el usuario activó el láser a baja potencia. */
const framePower = (s: Pick<AppState, 'frameLaser'>) => (s.frameLaser.on ? { power_pct: s.frameLaser.powerPct } : {});

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
  /**
   * Un trabajo acaba de terminar sin llegar al final: si el motor guardó un punto de control, se anota en el
   * registro de trabajos (con su porcentaje) para poder continuarlo desde ahí.
   */
  const noteInterruption = () => {
    void get()
      .checkInterrupted()
      .then((job) => {
        if (job) useJobLog.getState().markLatestInterrupted({ id: job.id, percent: job.percent, line: job.done_lines, total: job.total_lines });
      });
  };
  const apply = (ev: MachineEvent) => set((s) => reduceMachine(pickData(s), ev, Date.now()));

  /** Lo que el registro de trabajos (`features/help/jobLog.ts`) anota de un trabajo: máquina, archivo y tiempos. */
  const jobLogContext = (): JobLogContext => {
    const s = get();
    const now = Date.now();
    return {
      now,
      machine: selectActiveDevice(s).name,
      document: s.filePath ? (s.filePath.split(/[\\/]/).pop() ?? s.filePath) : null,
      estimatedS: s.previewStatus === 'ready' && s.preview && !s.previewStale ? s.preview.total_s : null,
      elapsedS: jobElapsedSeconds(s, now),
    };
  };

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
    set({ jobError: null, stopUnconfirmed: false, positioningLaser: false });
    try {
      await fn();
    } catch (e) {
      const message = errorMessage(e);
      set({ jobError: message, stopUnconfirmed: isStopUnconfirmed(e) });
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
    ...createResumeActions(set, get),
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
    positionAtPointer: false,
    jogging: false,
    dialog: null,
    dialogHosts: [],
    framedMark: null,

    refreshPorts: async () => {
      await guard(async () => {
        const ports = await getApi().listPorts();
        set((s) => ({ ports, selectedPort: s.selectedPort && ports.includes(s.selectedPort) ? s.selectedPort : defaultPort(ports) }));
      });
    },
    setSelectedPort: (selectedPort) => set({ selectedPort }),
    setBaud: (baud) => set({ baud }),
    setVirtualSpeed: (virtualSpeed) => {
      writePref('virtualSpeed', String(virtualSpeed));
      set({ virtualSpeed });
    },

    connect: async () => {
      const { baud, virtualSpeed } = get();
      // Una máquina por red (TCP o WebSocket) se conecta a su dirección, sin elegir puerto serie.
      const device = selectActiveDevice(get());
      const networkPort = connectionPortId(device.connection);
      // Ruida (UDP) usa la IP del perfil; el «puerto» solo distingue la máquina real de la virtual.
      const selectedPort = networkPort ?? portForDevice(device, get().selectedPort, get().ports);
      if (!selectedPort) {
        get().notifyKey('warning', 'machine.noPort');
        return;
      }
      // Galvo con una tarjeta REAL: antes de abrir el USB el usuario confirma la seguridad láser
      // (gafas, recinto, paro de emergencia). Se pide en cada conexión; no se recuerda.
      let galvoAck: GalvoSafetyAck | undefined;
      if (device.kind === 'galvo' && isGalvoUsbPort(selectedPort)) {
        const ack = await get().requestGalvoAck();
        if (!ack) return;
        galvoAck = ack;
      }
      set({ connecting: true });
      try {
        // La velocidad de una máquina virtual viaja en el id del puerto (`virtual:falcon2@5`).
        const port = isVirtualPort(selectedPort) ? withVirtualSpeed(selectedPort, virtualSpeed) : selectedPort;
        const { firmware } = await getApi().connect({ port, baud, device, ...(galvoAck ? { galvoAck } : {}) });
        set({ firmware });
        // Mejor esfuerzo: sirve para el aviso de modo láser y para sugerir $H o $X ante una alarma.
        // Ruida y el K40 no tienen ajustes `$`: Ruida lee su información desde el panel Máquina.
        if (device.kind !== 'ruida' && device.kind !== 'lhy_micro' && device.kind !== 'galvo') void get().readSettings({ silent: true });
        // Auto-home al conectar (opción del perfil): nunca sale solo; se pide la misma confirmación que el homing a mano.
        if (device.home_on_connect && isGrblFamily(device.kind)) get().openDialog({ kind: 'alarm', action: 'home' });
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
      if (get().positionAtPointer) await get().goToPointerDocPoint(x, y);
      else await get().goToDocPoint(x, y);
    },
    setPositionAtPointer: (on) => set({ positionAtPointer: on }),
    goToPointerDocPoint: async (x, y) => {
      const [ox, oy] = selectActiveDevice(get()).pointer_offset ?? [0, 0];
      return get().goToDocPoint(round3(x - ox), round3(y - oy));
    },
    setOverride: async (kind, percent) => {
      const why = overrideBlockedReason(get());
      if (why) return void get().notify('warning', t(why));
      await guard(() => getApi().setOverride({ kind, percent: Math.round(percent) }));
    },

    jog: async (dx, dy, speed) => {
      const why = jogBlockedReason(get());
      if (why) return void get().notify('warning', t(why));
      // El destino fuera de la cama se rechaza con el motivo (el motor lo vuelve a comprobar con la MPos real).
      const out = jogOutsideBed(selectActiveDevice(get()), get().status?.work_pos ?? null, [dx, dy]);
      if (out.length > 0) return void get().notify('warning', outsideBedMessage(out));
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
      // El K40 no tiene jog continuo cancelable (detener la placa pierde la posición): da un paso. El galvo salta a puntos.
      if (selectActiveDevice(s).kind === 'lhy_micro' || selectActiveDevice(s).kind === 'galvo') return void (await s.jogDir(dir));
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
    // `document` (la ruta del proyecto) la anota el motor en el punto de control del trabajo.
    startJob: (tweak) => guardJob(async () => getApi().startJob({ ...(await resolveJobArgs(tweak ? tweak(buildJobArgs(get())) : buildJobArgs(get()))), document: get().filePath ?? null })),
    pauseJob: async () => {
      await guard(() => getApi().pauseJob());
    },
    resumeJob: async () => {
      await guard(() => getApi().resumeJob());
    },
    cancelJob: async () => {
      get().stopFrameLoop();
      if (get().jogging) await get().jogStop();
      const ok = await guard(() => getApi().cancelJob());
      // Detener en el galvo apaga también el puntero rojo.
      if (ok) set({ galvoRedOn: false });
    },
    frameJob: async () => {
      // La marca se toma ANTES de esperar al motor: si algo cambia mientras tanto, el encuadre ya no vale.
      const mark = frameMarkOf(get());
      await guardJob(async () => getApi().frameJob({ ...(await resolveJobArgs(buildJobArgs(get()))), speed: jogSpeedNow(), ...framePower(get()) }));
      if (!get().jobError) set({ framedMark: mark });
      get().afterFrameStart('rect');
    },
    frameHull: async () => {
      const mark = frameMarkOf(get());
      await guardJob(async () => getApi().frameHull({ ...(await resolveJobArgs(buildJobArgs(get()))), speed: jogSpeedNow(), ...framePower(get()) }));
      if (!get().jobError) set({ framedMark: mark });
      get().afterFrameStart('hull');
    },
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
      // K40 (M2 Nano): el home es la orden IPP de la placa; `$X` no existe en ella.
      if (selectActiveDevice(get()).kind === 'lhy_micro') {
        if (action === 'home' && (await guard(() => getApi().lhyHome()))) get().dismissPositionLost();
        return;
      }
      await get().sendLine(action === 'unlock' ? '$X' : '$H', { confirmed: true });
    },

    onStatus: (report) => {
      const was = isJobBusy(get());
      const rebooted = report.position_unconfirmed === true && get().status?.position_unconfirmed !== true;
      apply({ type: 'status', report });
      if (rebooted) get().notifyKey('warning', 'machine.positionUnconfirmed.notice');
      // Una alarma en pleno trabajo lo da por fallido sin un evento propio: se anota aquí.
      if (was && get().jobState === 'error') {
        noteJobAlarm(report.state, jobLogContext());
        noteInterruption();
      }
    },
    onProgress: (progress) => apply({ type: 'progress', progress }),
    onLog: (log) => {
      apply({ type: 'log', log });
      // El texto crudo trae el código de la línea (`[frame_done] ...`): el bucle del encuadre lo lee.
      if (log.dir === 'in') get().noteFrameLog(log.text);
    },
    onJob: (raw) => {
      const wasBusy = isJobBusy(get());
      apply({ type: 'job', job: raw });
      // El mensaje del motor puede traer un código estable al inicio: lo que se muestra y se anota va limpio.
      const job = { ...raw, message: errorMessage(raw.message) };
      noteJobEvent(wasBusy, job, jobLogContext());
      get().onGalvoJob(raw);
      if (job.state === 'finished') {
        get().notify('success', job.message);
        // Texto variable: la serie y la fila del CSV pasan al siguiente trabajo (solo si terminó bien).
        advanceAfterJob({ getState: get });
        // Solo si el trabajo terminó bien: tras una cancelación o un error la cabeza no se mueve sola.
        if (get().finish.mode !== 'stay') void waitIdleThen(get, () => get().goToFinishPosition());
      }
      if (job.state === 'error') get().notify('error', job.message);
      // Error, o cancelado por desconectar con el trabajo en marcha: puede quedar punto de control.
      if (job.state === 'error' || job.state === 'cancelled') noteInterruption();
    },
    onConnection: (connection) => {
      const pending = get().positionLost && !get().connection.connected;
      // El motor da el trabajo por fallido justo antes de avisar que la conexión se cayó: vale un trabajo en marcha
      // o uno que acaba de terminar con error.
      const s0 = get();
      const lostMidJob = !connection.connected && connection.error !== null && (isJobBusy(s0) || (s0.jobState === 'error' && s0.jobEndedAt !== null && Date.now() - s0.jobEndedAt < 10_000));
      apply({ type: 'connection', connection });
      // La conexión se perdió en pleno trabajo (sin cerrar la app): se ofrece continuarlo al volver.
      if (lostMidJob) void get().checkInterrupted({ open: true });
      get().onGalvoConnection(connection);
      // Reconectó a la misma máquina que quedó con la posición dudosa: conviene hacer homing.
      if (connection.connected && pending && get().positionLost) {
        get().notifyKey('warning', 'machine.positionLost.reconnect');
        apply({ type: 'log', log: { dir: 'in', text: `; ${t('machine.positionLost.reconnect')}` } });
      }
      if (!connection.connected) set((s) => ({ jogging: false, frameLooping: false, positioningLaser: false, dialog: s.dialog?.kind === 'devices' || s.dialog?.kind === 'optimization' ? s.dialog : null }));
      if (connection.error) get().notify('error', errorMessage(connection.error));
    },
  };
};
