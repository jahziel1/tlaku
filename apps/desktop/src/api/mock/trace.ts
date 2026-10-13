import type { LaserApi } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

/** Métodos simulados del área `trace`. Pendiente: implementación plausible para desarrollar sin el motor. */
export function mockTrace(lag: Lag): Pick<LaserApi, 'traceImage'> {
  const pending = () => lag<never>(() => {
    throw new Error('Todavía no disponible en el modo simulado.');
  });
  return {
    traceImage: pending,
  };
}
