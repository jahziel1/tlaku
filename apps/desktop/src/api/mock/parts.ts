import type { LaserApi } from '../api';
import type { GeneratorInfo } from '@/bindings/GeneratorInfo';
import { mockGenerate } from './partsEngine';
import schema from './partsSchema.json';

type Lag = <T>(value: () => T) => Promise<T>;

/**
 * Generadores de piezas simulados. El formulario sale del mismo JSON que el motor
 * (`partsSchema.json`, que una prueba de Rust mantiene al día); la geometría la aproxima
 * `partsEngine.ts` (ver su cabecera). Real: `edit/parts.rs` + `crates/tlaku-parts`.
 */
export function mockParts(lag: Lag): Pick<LaserApi, 'listGenerators' | 'generatePart'> {
  return {
    listGenerators: () => lag(() => schema as unknown as GeneratorInfo[]),
    generatePart: (args) => lag(() => mockGenerate(args.generator, args.params, args.max_width_mm, args.gap_mm)),
  };
}
