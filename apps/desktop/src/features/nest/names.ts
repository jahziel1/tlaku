import { unitLabel } from '@/features/arrange/unitLabel';
import type { Unit } from '@/features/arrange/units';

/** Nombre legible por id de pieza; los repetidos llevan " (2)", " (3)"... para distinguirlos. */
export function pieceNames(units: readonly Unit[]): Map<string, string> {
  const seen = new Map<string, number>();
  return new Map(
    units.map((u) => {
      const label = unitLabel(u);
      const n = (seen.get(label) ?? 0) + 1;
      seen.set(label, n);
      return [u.id, n > 1 ? `${label} (${n})` : label];
    }),
  );
}
