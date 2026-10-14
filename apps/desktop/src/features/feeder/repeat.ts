import { create } from 'zustand';
import type { DeviceProfile } from '@/api/types';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { validateFeeder } from '@/features/machine/DeviceFeederModel';
import { selectActiveDevice, useStore } from '@/store';
import { sendBlockedReason } from '@/store/machine';
import { sendFeed, waitIdle, watchJobEnd } from './jobFlow';

/**
 * Repetir un trabajo N veces con el avance del alimentador entre una repetición y la siguiente
 * (M47, la función Feeder de los controladores Ruida y Trocen, aquí con el G-code del alimentador
 * del perfil). La primera repetición la confirma el diálogo de inicio, como cualquier trabajo; las
 * siguientes salen solas cuando el material terminó de avanzar y la máquina volvió a reposo.
 */

/** Más repeticiones que esto casi seguro es un número mal escrito. */
export const MAX_REPEATS = 99;
export const MIN_REPEATS = 2;

export interface RepeatConfig {
  /** Casilla del diálogo de inicio. */
  enabled: boolean;
  /** Cuántas veces sale el trabajo, de `MIN_REPEATS` a `MAX_REPEATS`. */
  count: number;
  /** Cuánto avanza el material entre una repetición y la siguiente, mm. */
  feedMm: number;
}

/**
 * - `idle`: no hay nada en marcha;
 * - `sending`: la repetición `index` está en la máquina;
 * - `feeding`: el alimentador avanza el material antes de la repetición `index`;
 * - `finished`, `failed`, `cancelled`: terminó (todas, por un error o porque se detuvo).
 */
export type RepeatPhase = 'idle' | 'sending' | 'feeding' | 'finished' | 'failed' | 'cancelled';

export interface RepeatState {
  config: RepeatConfig;
  phase: RepeatPhase;
  /** Repetición en curso (0 = la primera). */
  index: number;
  /** Cuántas repeticiones salieron completas. */
  done: number;
  /** Cuántas se pidieron en la tanda en curso o la última. */
  total: number;
  /** Mensaje del error cuando `phase` es `failed`. */
  message: string | null;
  /** Se pidió detener: se corta el trabajo en curso y no sale ninguna más. */
  stopRequested: boolean;
  set: (patch: Partial<Omit<RepeatState, 'set' | 'patchConfig'>>) => void;
  patchConfig: (patch: Partial<RepeatConfig>) => void;
}

export const DEFAULT_REPEAT: RepeatConfig = { enabled: false, count: 2, feedMm: 300 };

export const useRepeat = create<RepeatState>()((set, get) => ({
  config: DEFAULT_REPEAT,
  phase: 'idle',
  index: 0,
  done: 0,
  total: 0,
  message: null,
  stopRequested: false,
  set: (patch) => set(patch),
  patchConfig: (patch) => set({ config: { ...get().config, ...patch } }),
}));

const state = () => useRepeat.getState();

/** ¿Hay una tanda en marcha? */
export const isRepeating = (): boolean => state().phase === 'sending' || state().phase === 'feeding';

/** Máquinas cuyo avance es G-code que se manda por la línea: las de tipo GRBL. Ruida, K40 y galvo tienen otra vía. */
export function usesGcodeFeeder(kind: DeviceProfile['kind']): boolean {
  return kind !== 'ruida' && kind !== 'lhy_micro' && kind !== 'galvo';
}

export type RepeatUnavailable = 'kind' | 'off' | 'invalid';

/** Por qué no se puede repetir con el alimentador en esta máquina, o `null` si se puede. */
export function repeatUnavailable(device: Pick<DeviceProfile, 'kind' | 'feeder'>): RepeatUnavailable | null {
  if (!usesGcodeFeeder(device.kind)) return 'kind';
  if (!device.feeder.enabled) return 'off';
  if (Object.keys(validateFeeder(device.feeder)).length > 0) return 'invalid';
  return null;
}

/** Número de repeticiones válido (entero entre `MIN_REPEATS` y `MAX_REPEATS`) o `null`. */
export function cleanCount(n: number): number | null {
  return Number.isInteger(n) && n >= MIN_REPEATS && n <= MAX_REPEATS ? n : null;
}

/** Avance válido (mayor que 0 y dentro del tope del alimentador) o `null`. */
export function cleanFeed(mm: number, max: number): number | null {
  return Number.isFinite(mm) && mm > 0 && mm <= max ? mm : null;
}

/** Por qué no se puede empezar con la configuración actual, o `null`. */
export function repeatBlockedReason(device: Pick<DeviceProfile, 'kind' | 'feeder'>, config: RepeatConfig): string | null {
  const un = repeatUnavailable(device);
  if (un) return t(`tiling.repeat.unavailable.${un}`);
  if (cleanCount(config.count) === null) return t('tiling.repeat.err.count', { min: MIN_REPEATS, max: MAX_REPEATS });
  if (cleanFeed(config.feedMm, 5000) === null) return t('tiling.repeat.err.feed');
  return null;
}

/** Una repetición: manda el trabajo y espera a que termine. Devuelve cómo acabó. */
async function sendOnce(): Promise<'finished' | 'cancelled' | 'error' | 'rejected' | 'abandoned'> {
  const watcher = watchJobEnd();
  try {
    await useStore.getState().startJob();
  } catch (e) {
    watcher.cancel();
    state().set({ message: errorMessage(e) });
    return 'rejected';
  }
  // `startJob` no lanza: guarda el rechazo del motor en `jobError`.
  const rejected = useStore.getState().jobError;
  if (rejected) {
    watcher.cancel();
    state().set({ message: rejected });
    return 'rejected';
  }
  return watcher.done;
}

function stopWith(phase: 'failed' | 'cancelled' | 'finished', message: string | null = null): void {
  state().set({ phase, message, stopRequested: false });
}

/**
 * Empieza la tanda: cierra el diálogo de inicio y manda las repeticiones una tras otra. Se
 * resuelve al terminar la tanda (también si falla o se detiene). `onBegin` se llama cuando la
 * tanda ya está decidida (pasó las comprobaciones) y antes de mandar nada: ahí se abre el panel.
 */
export async function startRepeat(onBegin?: () => void): Promise<void> {
  if (isRepeating()) return;
  const app = useStore.getState();
  const device = selectActiveDevice(app);
  const { config } = state();
  const blocked = repeatBlockedReason(device, config);
  if (blocked) return void app.notify('warning', blocked);
  const why = sendBlockedReason(app);
  if (why) return void app.notify('warning', t(why));
  app.closeDialog();
  const total = config.count;
  state().set({ phase: 'sending', index: 0, done: 0, total, message: null, stopRequested: false });
  onBegin?.();
  for (let i = 0; i < total; i++) {
    if (i > 0) {
      // El material avanza antes de cada repetición salvo la primera.
      state().set({ phase: 'feeding', index: i });
      const failed = await sendFeed(selectActiveDevice(useStore.getState()).feeder, config.feedMm);
      if (failed) return stopWith('failed', t(failed === 'send' ? 'tiling.err.feed' : 'tiling.err.feedWait'));
      if (state().stopRequested) return stopWith('cancelled');
      const again = sendBlockedReason(useStore.getState());
      if (again) return stopWith('failed', t(again));
    }
    state().set({ phase: 'sending', index: i });
    const end = await sendOnce();
    if (end === 'abandoned') return;
    if (end === 'rejected') return stopWith('failed', state().message);
    if (end === 'cancelled' || state().stopRequested) return stopWith('cancelled');
    if (end === 'error') return stopWith('failed', useStore.getState().jobMessage || t('tiling.err.job'));
    state().set({ done: i + 1 });
    // La cabeza termina de moverse antes de pedir el avance o la repetición siguiente.
    await waitIdle();
    if (state().stopRequested) return stopWith('cancelled');
  }
  stopWith('finished');
}

/** Detiene la tanda: corta la repetición en curso (si la hay) y no deja que salga ninguna más. */
export async function stopRepeat(): Promise<void> {
  if (!isRepeating()) return;
  state().set({ stopRequested: true });
  if (state().phase === 'sending') await useStore.getState().cancelJob();
}

/** Deja el estado listo para otra tanda (al cerrar el panel de avance). */
export function resetRepeat(): void {
  if (isRepeating()) return;
  state().set({ phase: 'idle', message: null, index: 0, done: 0, total: 0, stopRequested: false });
}
