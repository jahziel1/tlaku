import { getApi } from '@/api';
import { clampJogSpeed } from '@/features/machine/jogMath';
import { planMacro, replyOk } from '@/features/machine/macros';
import { upsertPosition, workTarget } from '@/features/machine/positions';
import { t } from '@/i18n';
import { ENGINE_ERROR, errorCode, errorMessage } from '@/lib/errors';
import { readPref, writePref } from '@/lib/platform';
import { isJobBusy, selectActiveDevice } from './selectors';
import { jogBlockedReason } from './machine';
import type { SliceCreator } from './types';

/** Ejes extra de la ventana Mover. */
export type ExtraAxis = 'z' | 'u';
export const U_LETTERS = ['U', 'A', 'B', 'C'] as const;
export type ULetter = (typeof U_LETTERS)[number];

/** Límites del disparo de prueba: los mismos que hace cumplir el motor (`machine/extras.rs`). */
export const FIRE_LIMITS = { maxPct: 20, minMs: 10, maxMs: 2000 } as const;
export const DEFAULT_FIRE = { powerPct: 2, durationMs: 200 } as const;
/** Potencia máxima del encuadre con el láser encendido (`machine/frame.rs`). */
export const FRAME_MAX_PCT = 20;
export const DEFAULT_FRAME_PCT = 1;

/**
 * Líneas que el motor deja en el registro cuando termina o se cancela un encuadre. Llevan un código
 * estable (`[frame_done] ...`, `tlaku_i18n::coded`): el texto cambia con el idioma, el código no.
 */
export const FRAME_DONE_LOG = `[${ENGINE_ERROR.frameDone}] ; frame finished`;
export const FRAME_CANCELLED_LOG = `[${ENGINE_ERROR.frameCancelled}] ; frame cancelled`;

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
const round3 = (n: number) => Math.round(n * 1000) / 1000;
const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

function readNum(key: string, fallback: number, lo: number, hi: number): number {
  const n = Number(readPref(key));
  return readPref(key) !== null && Number.isFinite(n) ? clamp(n, lo, hi) : fallback;
}

export interface MoveControlSlice {
  /** Altura Z de enfoque (coordenadas de trabajo, mm). */
  focusZ: number;
  /** Letra del eje U en el G-code de la máquina. */
  uLetter: ULetter;
  fire: { powerPct: number; durationMs: number };
  /** Hay un disparo de prueba en curso. */
  firing: boolean;
  /** Encuadre con el láser a baja potencia: nunca se recuerda encendido entre sesiones. */
  frameLaser: { on: boolean; powerPct: number };
  /** Repetir el encuadre hasta detener. */
  frameLoop: boolean;
  /** Hay un encuadre en bucle activo. */
  frameLooping: boolean;
  frameKind: 'rect' | 'hull';

  setFocusZ: (z: number) => void;
  setULetter: (letter: ULetter) => void;
  setFire: (patch: Partial<MoveControlSlice['fire']>) => void;
  setFrameLaser: (patch: Partial<MoveControlSlice['frameLaser']>) => void;
  setFrameLoop: (on: boolean) => void;
  /** Un paso del eje extra: `dir` 1 sube (aleja el cabezal del material), -1 baja. */
  jogExtra: (axis: ExtraAxis, dir: 1 | -1) => Promise<void>;
  /** «Enfocar»: lleva Z a la altura de enfoque. */
  goFocus: () => Promise<void>;
  /** Disparo de prueba (el diálogo de doble confirmación ya se mostró). El láser queda apagado pase lo que pase. */
  fireTest: () => Promise<boolean>;
  /** Apaga ya el láser: reinicio en tiempo real (llega aunque el disparo siga) y un `M5`. */
  cancelFire: () => Promise<void>;
  savePosition: (name: string) => Promise<boolean>;
  deletePosition: (name: string) => Promise<void>;
  goToPosition: (name: string) => Promise<boolean>;
  /** Ejecuta una macro del perfil activo: pide confirmación si hace falta; nunca durante un trabajo. */
  runMacro: (index: number) => Promise<void>;
  /** Envía las líneas de la macro (ya confirmadas), una a una, deteniéndose en el primer rechazo. */
  sendMacro: (index: number) => Promise<boolean>;
  /** Llamado tras pedir un encuadre: arma o desarma el bucle. */
  afterFrameStart: (kind: 'rect' | 'hull') => void;
  /** Mira las líneas del registro de entrada para encadenar el encuadre en bucle. */
  noteFrameLog: (text: string) => void;
  stopFrameLoop: () => void;
}

export const createMoveControlSlice: SliceCreator<MoveControlSlice> = (set, get) => {
  const guard = async (fn: () => Promise<unknown>): Promise<boolean> => {
    try {
      await fn();
      return true;
    } catch (e) {
      get().notify('error', errorMessage(e));
      return false;
    }
  };
  const speedNow = () => clampJogSpeed(get().jogSpeed, selectActiveDevice(get()));

  return {
    focusZ: readNum('focusZ', 0, -1000, 1000),
    uLetter: (U_LETTERS as readonly string[]).includes(readPref('uLetter') ?? '') ? (readPref('uLetter') as ULetter) : 'U',
    fire: {
      powerPct: readNum('firePct', DEFAULT_FIRE.powerPct, 0.1, FIRE_LIMITS.maxPct),
      durationMs: Math.round(readNum('fireMs', DEFAULT_FIRE.durationMs, FIRE_LIMITS.minMs, FIRE_LIMITS.maxMs)),
    },
    firing: false,
    frameLaser: { on: false, powerPct: readNum('framePct', DEFAULT_FRAME_PCT, 0.1, FRAME_MAX_PCT) },
    frameLoop: false,
    frameLooping: false,
    frameKind: 'rect',

    setFocusZ: (z) => {
      const focusZ = Number.isFinite(z) ? clamp(round3(z), -1000, 1000) : 0;
      writePref('focusZ', String(focusZ));
      set({ focusZ });
    },
    setULetter: (uLetter) => {
      writePref('uLetter', uLetter);
      set({ uLetter });
    },
    setFire: (patch) => {
      const cur = get().fire;
      const fire = {
        powerPct: clamp(round3(patch.powerPct ?? cur.powerPct), 0.1, FIRE_LIMITS.maxPct),
        durationMs: Math.round(clamp(patch.durationMs ?? cur.durationMs, FIRE_LIMITS.minMs, FIRE_LIMITS.maxMs)),
      };
      writePref('firePct', String(fire.powerPct));
      writePref('fireMs', String(fire.durationMs));
      set({ fire });
    },
    setFrameLaser: (patch) => {
      const cur = get().frameLaser;
      const frameLaser = { on: patch.on ?? cur.on, powerPct: clamp(round3(patch.powerPct ?? cur.powerPct), 0.1, FRAME_MAX_PCT) };
      writePref('framePct', String(frameLaser.powerPct));
      set({ frameLaser });
    },
    setFrameLoop: (frameLoop) => set({ frameLoop }),

    jogExtra: async (axis, dir) => {
      const s = get();
      const why = jogBlockedReason(s);
      if (why) return void s.notify('warning', t(why));
      const device = selectActiveDevice(s);
      if (axis === 'z' ? !device.z_axis : !device.u_axis) return void s.notifyKey('warning', 'machine.axis.notEnabled');
      // En Z, «subir» aleja el cabezal; con `z_invert` el +Z del G-code lo acerca.
      const sign = axis === 'z' && device.z_invert ? -dir : dir;
      await guard(() =>
        getApi().jogAxis({ kind: device.kind, axis: axis === 'z' ? 'Z' : s.uLetter, delta: round3(sign * s.jogStep), speed: speedNow() }),
      );
    },
    goFocus: async () => {
      const s = get();
      const why = jogBlockedReason(s);
      if (why) return void s.notify('warning', t(why));
      const device = selectActiveDevice(s);
      if (!device.z_axis) return void s.notifyKey('warning', 'machine.axis.notEnabled');
      await guard(() => getApi().jogAxisTo({ kind: device.kind, axis: 'Z', pos: s.focusZ, speed: speedNow() }));
    },

    fireTest: async () => {
      const s = get();
      if (s.firing) return false;
      if (!s.connection.connected) {
        s.notifyKey('warning', 'reason.needMachine');
        return false;
      }
      if (isJobBusy(s)) {
        s.notifyKey('warning', 'reason.jobRunning');
        return false;
      }
      const why = jogBlockedReason(s);
      if (why) {
        s.notify('warning', t(why));
        return false;
      }
      const device = selectActiveDevice(s);
      set({ firing: true });
      let ok = false;
      try {
        await getApi().fireTest({ kind: device.kind, power_pct: s.fire.powerPct, s_max: device.s_max, duration_ms: s.fire.durationMs });
        ok = true;
      } catch (e) {
        s.notify('error', errorMessage(e));
      } finally {
        // El motor ya manda M5 siempre; esto es un segundo cerrojo por si la llamada se cortó a medias.
        if (!ok) {
          try {
            await getApi().machineCommand({ line: 'M5' });
          } catch {
            /* sin conexión: ya no hay nada que apagar desde aquí */
          }
        }
        set({ firing: false });
      }
      return ok;
    },
    cancelFire: async () => {
      if (!get().firing) return;
      try {
        await getApi().machineRealtime({ cmd: 'reset' });
      } catch (e) {
        get().notify('error', errorMessage(e));
      }
      try {
        await getApi().machineCommand({ line: 'M5' });
      } catch {
        /* la conexión puede haber caído; el reinicio ya apagó el láser */
      }
    },

    savePosition: async (name) => {
      const s = get();
      const st = s.status;
      if (!s.connection.connected || !st) {
        s.notifyKey('warning', 'reason.needMachine');
        return false;
      }
      const device = selectActiveDevice(s);
      const positions = upsertPosition(device.positions, name, st.machine_pos[0], st.machine_pos[1]);
      if (!positions) {
        s.notifyKey('warning', 'machine.positions.cannotSave');
        return false;
      }
      const saved = await s.saveDevice({ ...device, positions });
      if (saved) s.notify('success', t('machine.positions.saved', { name: name.trim() }));
      return saved;
    },
    deletePosition: async (name) => {
      const s = get();
      const device = selectActiveDevice(s);
      await s.saveDevice({ ...device, positions: device.positions.filter((p) => p.name !== name) });
    },
    goToPosition: async (name) => {
      const s = get();
      const why = jogBlockedReason(s);
      if (why) {
        s.notify('warning', t(why));
        return false;
      }
      const pos = selectActiveDevice(s).positions.find((p) => p.name === name);
      if (!pos || !s.status) return false;
      const [x, y] = workTarget(pos, s.status);
      return guard(() => getApi().jogTo({ x, y, speed: speedNow() }));
    },

    runMacro: async (index) => {
      const s = get();
      const macro = selectActiveDevice(s).macros[index];
      if (!macro) return;
      if (!s.connection.connected) return void s.notifyKey('warning', 'reason.needMachine');
      if (isJobBusy(s)) return void s.notifyKey('warning', 'machine.console.jobBusy');
      const plan = planMacro(macro, selectActiveDevice(s).rx_buffer);
      if (!plan.ok) return void s.notify('error', t('machine.macros.invalid', { name: macro.name }));
      if (plan.confirm) return s.openDialog({ kind: 'macro', index });
      await s.sendMacro(index);
    },
    sendMacro: async (index) => {
      const s = get();
      const device = selectActiveDevice(s);
      const macro = device.macros[index];
      if (!macro) return false;
      if (!s.connection.connected) {
        s.notifyKey('warning', 'reason.needMachine');
        return false;
      }
      if (isJobBusy(s)) {
        s.notifyKey('warning', 'machine.console.jobBusy');
        return false;
      }
      const plan = planMacro(macro, device.rx_buffer);
      if (!plan.ok) {
        s.notify('error', t('machine.macros.invalid', { name: macro.name }));
        return false;
      }
      // Marlin y Smoothie separan parámetros con espacios: se manda la línea tal cual, sin comentarios.
      const keepSpaces = /marlin|smoothie/i.test(s.firmware ?? '');
      let n = 0;
      for (const line of plan.lines) {
        try {
          const reply = await getApi().machineCommand({ line: keepSpaces ? line.raw : line.text });
          if (!replyOk(reply)) {
            s.notify('error', t('machine.macros.rejected', { name: macro.name, line: line.text, reply: reply[reply.length - 1] ?? '' }));
            return false;
          }
        } catch (e) {
          s.notify('error', t('machine.macros.failed', { name: macro.name, line: line.text, error: errorMessage(e) }));
          return false;
        }
        n++;
        // Un reinicio o una pérdida de conexión a mitad: no se sigue mandando.
        if (!get().connection.connected) return false;
      }
      s.notify('success', t('machine.macros.done', { name: macro.name, n }));
      return true;
    },

    afterFrameStart: (kind) => {
      if (get().jobError) return void set({ frameLooping: false });
      // El galvo repite el encuadre solo (en el motor) hasta Detener: el bucle de la interfaz no aplica.
      if (selectActiveDevice(get()).kind === 'galvo') return void set({ frameKind: kind, frameLooping: false });
      set({ frameKind: kind, frameLooping: get().frameLoop });
    },
    noteFrameLog: (text) => {
      const s = get();
      if (!s.frameLooping) return;
      const code = errorCode(text);
      if (code === ENGINE_ERROR.frameCancelled || !s.connection.connected || isJobBusy(s)) return void set({ frameLooping: false });
      if (code !== ENGINE_ERROR.frameDone) return;
      // Un respiro para que la máquina vuelva a reposo antes de pedir la vuelta siguiente.
      void sleep(400).then(() => {
        const cur = get();
        if (!cur.frameLooping || !cur.connection.connected || isJobBusy(cur)) return;
        void (cur.frameKind === 'hull' ? cur.frameHull() : cur.frameJob());
      });
    },
    stopFrameLoop: () => set({ frameLooping: false }),
  };
};
