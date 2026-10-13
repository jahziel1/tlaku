import type { Affine, DesignObject } from '@/api/types';

/**
 * Contrato del nesting con el motor (acomodar piezas en la cama o en un material irregular).
 * Los nombres de campo van en snake_case porque viajan tal cual a Rust.
 */

/** none = solo 0°, quarter = 0/90/180/270, step = cada N°, free = continuo (el motor usa pasos finos). */
export type NestRotationMode = 'none' | 'quarter' | 'step' | 'free';

export interface NestRotation {
  mode: NestRotationMode;
  /** Solo cuenta con `step`. */
  step_deg: number;
}

export type NestContainer =
  /** La cama: origen (0,0), en mm. */
  | { type: 'rect'; width: number; height: number }
  /** Material irregular: contorno cerrado (con huecos) de un objeto del documento. */
  | { type: 'object'; object: DesignObject };

export type NestGravity = 'gravity' | 'bottom_left';

export interface NestPieceInput {
  /** Id de la unidad (objeto suelto o grupo completo = una pieza). */
  id: string;
  objects: DesignObject[];
  copies: number;
}

export interface NestRequest {
  pieces: NestPieceInput[];
  container: NestContainer;
  /** mm mínimos entre piezas. */
  spacing: number;
  /** mm mínimos entre piezas y el borde del contenedor. */
  margin: number;
  rotation: NestRotation;
  allow_mirror: boolean;
  /** Permitir piezas dentro de los huecos de otras. */
  use_holes: boolean;
  gravity: NestGravity;
  /** Tope de tiempo del motor, 1..600 s. */
  time_limit_s: number;
  /** Entero >= 0. */
  seed: number;
}

export interface NestRef {
  piece: string;
  copy: number;
}

export interface NestPlacement {
  piece: string;
  /** 0 = el objeto original; >= 1 = copia que la interfaz debe crear como objetos nuevos. */
  copy: number;
  rotation_deg: number;
  mirror: boolean;
  /** Afín en coordenadas de documento que va POR ENCIMA del transform actual de cada objeto de la pieza. */
  transform: Affine;
}

export interface NestStatus {
  running: boolean;
  elapsed_ms: number;
  evaluations: number;
  /** 0..100: área neta colocada / área neta del contenedor. */
  utilization: number;
  placed: number;
  total: number;
  /** Mejor solución hasta ahora (solo las piezas colocadas). */
  placements: NestPlacement[];
  /** Las que no cupieron. */
  unplaced: NestRef[];
  /** Terminó por el tope de tiempo. */
  timed_out: boolean;
  /** Mensaje en español si la petición no es válida. */
  error: string | null;
}
