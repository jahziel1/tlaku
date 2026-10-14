/** Reductor puro de los eventos `machine://*` y los tipos de datos de la máquina. */
import type { ConnectionEvent, JobEvent, LogEvent, ProgressEvent, StatusReport } from '@/api/types';
import { parseAlarmLine, type AlarmAction } from '@/features/machine/grblCodes';
import { portKey } from '@/features/machine/virtualPort';
import { type GrblSettings } from '@/lib/device';

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
  | { kind: 'optimization' }
  /** Ventana "Ajustes de la máquina": leer, editar y respaldar los `$n` de GRBL (`features/machine/MachineSettingsDialog.tsx`). */
  | { kind: 'machineSettings' }
  /** Confirmación de una macro del perfil (índice en `DeviceProfile.macros`). */
  | { kind: 'macro'; index: number };

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
