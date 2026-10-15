import type { LaserApi } from '../api';
import type { DeviceProfile, InterruptedJob, JobPreview, ResumeView } from '../types';
import type { MockMachine } from './machine';

type Lag = <T>(value: () => T) => Promise<T>;

interface Checkpoint {
  id: string;
  preview: JobPreview;
  device: DeviceProfile;
  document: string | null;
  startedAt: number;
  acked: number;
  /** Líneas ya hechas antes del tramo que se está enviando (una reanudación). */
  offset: number;
}

/** Cuánto retrocede la propuesta del simulador (movimientos). */
const BACK = 4;

/**
 * Punto de control simulado («continuar un trabajo interrumpido»): se crea al iniciar un trabajo, se borra al
 * terminar o cancelar a propósito y se conserva si la conexión se cae, hay una falla o se corta la luz. Usa el
 * recorrido de la vista previa como si fuera el G-code guardado (un movimiento por línea).
 */
export function mockResume(lag: Lag, machine: MockMachine): Pick<LaserApi, 'interruptedJob' | 'interruptedJobView' | 'discardInterruptedJob' | 'continueCheck' | 'continueFrame' | 'continueJob'> & {
  begin(preview: JobPreview, device: DeviceProfile, document: string | null): void;
} {
  let ckpt: Checkpoint | null = null;
  let seq = 0;

  machine.on('machine://progress', (p) => {
    if (ckpt) ckpt.acked = ckpt.offset + p.acked;
  });
  machine.on('machine://job', (j) => {
    // Terminar bien o cancelar a propósito lo borra; una falla o perder la conexión, no.
    if (j.state === 'finished' || j.state === 'cancelled') ckpt = null;
  });

  const total = (c: Checkpoint) => c.preview.moves.length / 4;
  const summary = (c: Checkpoint): InterruptedJob => ({
    id: c.id,
    kind: 'gcode',
    started_at_ms: c.startedAt,
    updated_at_ms: Date.now(),
    device_id: c.device.id,
    device_name: c.device.name,
    device_kind: c.device.kind,
    port: machine.port ?? 'virtual:falcon2',
    document: c.document,
    total_lines: total(c),
    done_lines: Math.min(c.acked, total(c)),
    percent: total(c) === 0 ? 0 : (Math.min(c.acked, total(c)) / total(c)) * 100,
    progress_known: true,
    mpos: null,
    modal: null,
    can_resume: true,
    note: null,
    resumed: false,
    machine: 'not_checked',
    ahead_lines: 0,
    ahead_mm: 0,
    recommend_discard: false,
    warning: null,
  });
  const need = () => {
    if (!ckpt) throw new Error('No hay un trabajo interrumpido que continuar.');
    return ckpt;
  };

  return {
    begin: (preview, device, document) => {
      ckpt = { id: `mock-${++seq}`, preview: structuredClone(preview), device, document, startedAt: Date.now(), acked: 0, offset: 0 };
    },
    interruptedJob: () => lag(() => (ckpt && !machine.hasJob() ? summary(ckpt) : null)),
    interruptedJobView: ({ back_lines }) =>
      lag((): ResumeView => {
        const c = need();
        const n = total(c);
        const last = Math.max(0, Math.min(n - 1, c.acked - 1));
        const move_index = Math.max(1, last - (back_lines ?? BACK));
        return {
          job: summary(c),
          moves: [...c.preview.moves],
          move_lines: Array.from({ length: n }, (_, i) => i),
          bounds: c.preview.bounds,
          last_acked_move: last,
          suggested: { line: move_index, move_index, at: null, executing_line: last, distance_mm: 0, mismatch: false, row_restart: false, percent: (move_index / n) * 100, z_delta: 0 },
          warnings: [],
        };
      }),
    discardInterruptedJob: () =>
      lag(() => {
        ckpt = null;
      }),
    continueCheck: () =>
      lag(() => {
        machine.requireConnection();
        const busy = machine.state !== 'idle' && machine.state !== 'alarm';
        return { homing: busy ? null : machine.homingEnabled(), settings_read: !busy, homed: machine.homed, state: machine.state };
      }),
    continueFrame: () =>
      lag(() => {
        const c = need();
        machine.frame(c.preview.bounds);
      }),
    continueJob: ({ args }) =>
      lag(() => {
        const c = need();
        machine.requireConnection();
        if (machine.state !== 'idle' && machine.state !== 'alarm') throw new Error(`La máquina está ocupada (${machine.state}): espera a que termine el encuadre o el movimiento antes de continuar. Nada se envió.`);
        if (machine.homingEnabled() && !machine.homed) throw new Error('Haz primero el homing: la máquina perdió su posición y el trabajo no puede continuar sin referenciarla. Nada se envió.');
        if (!machine.homingEnabled() && !args.confirm_unmoved) throw new Error('Esta máquina no tiene homing: confirma que la cabeza y el material no se movieron y que el origen es el mismo. Nada se envió.');
        if (!args.framed_ok) throw new Error('Encuadra el trabajo completo y comprueba que el material sigue en su lugar antes de continuar. Nada se envió.');
        if (!args.checklist_ok) throw new Error('Completa la lista de comprobación antes de continuar. Nada se envió.');
        const from = Math.max(0, args.line);
        const times = c.preview.move_times;
        const t0 = from > 0 ? (times[from - 1] ?? 0) : 0;
        const tail: JobPreview = {
          ...c.preview,
          moves: c.preview.moves.slice(from * 4),
          move_times: times.slice(from).map((t) => t - t0),
          total_s: Math.max(0.5, c.preview.total_s - t0),
        };
        machine.startJob(tail, c.device, ` · desde la línea ${from + 1}`);
        c.acked = from;
        c.offset = from;
      }),
  };
}
