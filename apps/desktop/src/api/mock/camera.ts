import type { LaserApi } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

/** Métodos simulados del área `camera`. Pendiente: implementación plausible para desarrollar sin el motor. */
export function mockCamera(lag: Lag): Pick<LaserApi, 'detectBoard' | 'calibrateLens'> {
  const pending = () => lag<never>(() => {
    throw new Error('Todavía no disponible en el modo simulado.');
  });
  return {
    detectBoard: pending,
    calibrateLens: pending,
  };
}
