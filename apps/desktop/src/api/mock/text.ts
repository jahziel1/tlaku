import type { LaserApi } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

/** Métodos simulados del área `text`. Pendiente: implementación plausible para desarrollar sin el motor. */
export function mockText(lag: Lag): Pick<LaserApi, 'listFonts' | 'textOutline'> {
  const pending = () => lag<never>(() => {
    throw new Error('Todavía no disponible en el modo simulado.');
  });
  return {
    listFonts: pending,
    textOutline: pending,
  };
}
