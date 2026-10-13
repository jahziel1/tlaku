import type { LaserApi } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

/** Métodos simulados del área `geometry`. Pendiente: implementación plausible para desarrollar sin el motor. */
export function mockGeometry(lag: Lag): Pick<LaserApi, 'geometryOp'> {
  const pending = () => lag<never>(() => {
    throw new Error('Todavía no disponible en el modo simulado.');
  });
  return {
    geometryOp: pending,
  };
}
