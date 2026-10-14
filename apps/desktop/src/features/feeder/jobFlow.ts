import { useStore } from '@/store';
import { feedLines, type FeederSettings } from '@/features/machine/DeviceFeederModel';

/**
 * Piezas comunes de los flujos que mandan varios trabajos seguidos y mueven el material entre uno
 * y otro (el tiling y las repeticiones con alimentador): esperar a que acabe un trabajo, a que la
 * máquina vuelva a reposo y mandar el avance del alimentador.
 */

/** Espera a que el estado de la máquina sea `idle` (con tope), para no mandar lo siguiente en pleno movimiento. */
export const IDLE_WAIT = { tries: 600, everyMs: 200 } as const;
export const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

export type JobEnd = 'finished' | 'cancelled' | 'error';

/** Promesa del fin del trabajo en curso. Hay que crearla ANTES de iniciarlo. `cancel` suelta la espera. */
export function watchJobEnd(): { done: Promise<JobEnd | 'abandoned'>; cancel: () => void } {
  let unsub = () => {};
  let seen = false;
  let cancel = () => {};
  const done = new Promise<JobEnd | 'abandoned'>((resolve) => {
    cancel = () => {
      unsub();
      resolve('abandoned');
    };
    unsub = useStore.subscribe((s, prev) => {
      if (s.jobState === prev.jobState) return;
      if (s.jobState === 'running') seen = true;
      if (seen && (s.jobState === 'finished' || s.jobState === 'cancelled' || s.jobState === 'error')) {
        unsub();
        resolve(s.jobState);
      }
    });
  });
  return { done, cancel };
}

/** Espera a que la máquina esté en reposo; `false` si se desconectó o no volvió a reposo a tiempo. */
export async function waitIdle(): Promise<boolean> {
  // GRBL tarda un reporte en pasar de `run` a `idle`: se da un respiro antes de mirar.
  await sleep(IDLE_WAIT.everyMs);
  for (let i = 0; i < IDLE_WAIT.tries; i++) {
    const s = useStore.getState();
    if (!s.connection.connected) return false;
    if (!s.status || s.status.state === 'idle') return true;
    await sleep(IDLE_WAIT.everyMs);
  }
  return false;
}

/**
 * Manda el G-code de avance del alimentador (`mm` de material) y espera a que la máquina vuelva a
 * reposo. Devuelve `null` si salió bien, `'send'` si alguna línea no se pudo enviar o `'wait'` si
 * la máquina no volvió a reposo.
 */
export async function sendFeed(feeder: FeederSettings, mm: number): Promise<null | 'send' | 'wait'> {
  const app = useStore.getState();
  for (const line of feedLines(feeder, mm)) {
    if (!(await app.sendLine(line, { confirmed: true }))) return 'send';
  }
  return (await waitIdle()) ? null : 'wait';
}
