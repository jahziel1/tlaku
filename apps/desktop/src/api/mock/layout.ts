import type { LaserApi } from '../api';
import { mockFitRegistration } from './registrationFit';
import { mockPlanTiles } from './tilePlan';

type Lag = <T>(value: () => T) => Promise<T>;

/** Print & Cut y tiling del modo simulado: maquetas del cálculo del motor (ver cada archivo). */
export function mockLayout(lag: Lag): Pick<LaserApi, 'fitRegistration' | 'planTiles'> {
  return {
    fitRegistration: ({ pairs, mode }) => lag(() => mockFitRegistration(pairs, mode)),
    planTiles: (args) => lag(() => mockPlanTiles(args)),
  };
}
