import type { DeviceKind } from '@/bindings/DeviceKind';
import type { Origin } from '@/bindings/Origin';
import type { I18nKey } from '@/i18n';

/**
 * Tipos y constructores del catálogo de máquinas (el catálogo está en `catalog.ts` y `catalog.*.ts`).
 *
 * Reglas del catálogo:
 * - Sala limpia: los datos salen de fichas públicas de fabricantes, tiendas y foros, nunca de la
 *   lista de dispositivos de otro programa. Cada dato lleva su URL en `source`.
 * - Solo se escribe lo que tiene fuente. Lo que no se conoce se deja sin definir (`undefined`) y el
 *   asistente le dice al usuario que lo lea de la máquina.
 * - `kind` de la fuente: `oficial` (fabricante o proyecto del firmware), `tienda` (ficha de tienda,
 *   reseña o comparativa) y `foro` (mensaje de un usuario: puede tener la máquina modificada).
 * - Las cifras de tamaño, velocidad y aceleración son las que anuncia el fabricante o un usuario, no
 *   las que la máquina tenga hoy: el asistente las compara con `$$` y deja elegir.
 */

export type SourceKind = 'oficial' | 'tienda' | 'foro' | 'medido';

/** Un dato con su origen. `note` explica un conflicto entre fuentes u otra salvedad. */
export interface Fact<T> {
  value: T;
  source: string;
  kind: SourceKind;
  note?: I18nKey;
}

/**
 * Firma del firmware: texto que la placa dice en su saludo o en `$I` y que delata este modelo.
 * Solo se escribe cuando hay una fuente con la cadena exacta (una salida de consola publicada o medida).
 */
export interface Signature {
  /** Fragmentos (en minúsculas) que deben aparecer TODOS en el saludo o en `$I`. */
  contains: string[];
  /** Fragmentos (en minúsculas) que, si aparecen, se enseñan como evidencia extra pero no se exigen. */
  show?: string[];
  /** Cama (`$130/$131`) del modelo: solo desempata cuando varias firmas coinciden. */
  bed?: [number, number];
  /** URL de donde sale la cadena. */
  source: string;
  kind: SourceKind;
}

export interface CatalogEntry {
  /** Identificador estable (sin espacios). */
  id: string;
  /** Marca y modelo son nombres propios, no se traducen. */
  brand: string;
  model: string;
  kind: DeviceKind;
  /** Otros tipos de firmware con los que esta máquina también puede aparecer al detectarla. */
  alsoKinds?: DeviceKind[];
  /** Entrada de familia (no es un modelo concreto). */
  generic?: boolean;
  /** Otros nombres con los que se busca. */
  aliases?: string[];
  /** Fragmentos (en minúsculas) del saludo o de `$I` que delatan esta placa. */
  detect?: Fact<string[]>;
  /** Firmas exactas del firmware para preseleccionar el modelo al detectar (ver `signature.ts`). */
  signatures?: Signature[];

  /** Controladora o firmware tal como lo dice la fuente (texto libre, para mostrarlo). */
  controller?: Fact<string>;

  /** Área de trabajo en mm. */
  bed?: Fact<[number, number]>;
  /** Velocidad máxima de movimiento, mm/min (el mismo valor en X e Y). */
  maxRate?: Fact<number>;
  /** Aceleración, mm/s² (la misma en X e Y). */
  accel?: Fact<number>;
  /** `S` para el 100 % de potencia (`$30` en GRBL). */
  sMax?: Fact<number>;
  /** Potencia dinámica con `M4` (true) o solo potencia constante con `M3` (false). */
  m4?: Fact<boolean>;
  /** Tiene bomba o salida de aire asistido. */
  air?: Fact<boolean>;
  baud?: Fact<number>;
  origin?: Fact<Origin>;
  /** Tiene finales de carrera y hace homing al encender. */
  homing?: Fact<boolean>;
  zAxis?: Fact<boolean>;
  notes: I18nKey[];
}

// ---------------------------------------------------------------------------
// Constructores de datos con fuente
// ---------------------------------------------------------------------------

export const fact = <T>(kind: SourceKind, value: T, source: string, note?: I18nKey): Fact<T> => ({
  value,
  source,
  kind,
  ...(note ? { note } : {}),
});

export const official = <T>(value: T, source: string, note?: I18nKey) => fact('oficial', value, source, note);
export const shop = <T>(value: T, source: string, note?: I18nKey) => fact('tienda', value, source, note);
export const forum = <T>(value: T, source: string, note?: I18nKey) => fact('foro', value, source, note);
/** Leído de una máquina real por el proyecto (`$$`, `$I`), con la sesión documentada en el repositorio. */
export const measured = <T>(value: T, source: string, note?: I18nKey) => fact('medido', value, source, note);
