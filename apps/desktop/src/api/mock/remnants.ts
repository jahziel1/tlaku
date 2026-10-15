import type { RemnantLibrary } from '@/features/nest/remnants';
import { emptyRemnants } from '@/features/nest/remnants';
import type { LaserApi } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

/** Inventario de retazos simulado: vive en memoria mientras dure la sesión del mock. */
export function mockRemnants(lag: Lag): Pick<LaserApi, 'loadRemnants' | 'saveRemnants'> {
  let library: RemnantLibrary = emptyRemnants();
  return {
    loadRemnants: () => lag(() => structuredClone(library)),
    saveRemnants: ({ library: next }) =>
      lag(() => {
        const ids = new Set<string>();
        for (const r of next.remnants) {
          if (!r.id.trim() || ids.has(r.id)) throw new Error(`El id de retazo «${r.id}» está repetido o vacío.`);
          ids.add(r.id);
        }
        library = structuredClone(next);
      }),
  };
}
