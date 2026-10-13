import type { LaserApi } from '../api';
import type { MaterialLibrary } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

// TODO(fase4): biblioteca en memoria y cuadrícula de prueba simulada.
export function mockMaterials(
  lag: Lag,
): Pick<
  LaserApi,
  'loadMaterialLibrary' | 'saveMaterialLibrary' | 'importMaterialLibrary' | 'exportMaterialLibrary' | 'materialTest'
> {
  let library: MaterialLibrary = { version: 1, presets: [] };
  return {
    loadMaterialLibrary: () => lag(() => structuredClone(library)),
    saveMaterialLibrary: ({ library: next }) =>
      lag(() => {
        library = structuredClone(next);
      }),
    importMaterialLibrary: () =>
      lag(() => {
        throw new Error('aún no implementado');
      }),
    exportMaterialLibrary: () => lag(() => undefined),
    materialTest: () =>
      lag(() => {
        throw new Error('aún no implementado');
      }),
  };
}
