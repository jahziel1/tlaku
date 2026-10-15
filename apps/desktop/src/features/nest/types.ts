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

/**
 * Giros que admite una pieza: los de las opciones (`inherit`), ninguno (`none`) o solo la veta de
 * la madera (`grain`: 0° y 180°, con la veta dibujada en horizontal).
 */
export type NestPieceRotation = 'inherit' | 'none' | 'grain';

export interface NestPieceInput {
  /** Id de la unidad (objeto suelto o grupo completo = una pieza). */
  id: string;
  objects: DesignObject[];
  copies: number;
  rotation: NestPieceRotation;
}

/** Un retazo guardado que se ofrece como hoja: contornos cerrados (exterior y huecos) en mm. */
export interface NestRemnantInput {
  id: string;
  paths: [number, number][][];
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
  /** Hojas que se pueden usar en total; 1 = una sola, más = seguir en hojas nuevas iguales al contenedor. */
  max_sheets: number;
  /** Retazos que se prueban antes que una hoja nueva (en este orden). */
  remnants: NestRemnantInput[];
  /** Ancho mínimo (mm) de los retazos que sobran; 0 = no calcular el sobrante. */
  leftover_min_mm: number;
}

export interface NestRef {
  piece: string;
  copy: number;
}

export interface NestPlacement {
  piece: string;
  /** Posición entre las hojas usadas (`NestStatus.sheets`). */
  sheet: number;
  /** 0 = el objeto original; >= 1 = copia que la interfaz debe crear como objetos nuevos. */
  copy: number;
  rotation_deg: number;
  mirror: boolean;
  /** Afín en coordenadas de documento que va POR ENCIMA del transform actual de cada objeto de la pieza. */
  transform: Affine;
}

/** Resumen de una hoja usada. */
export interface NestSheet {
  /** Posición entre las hojas usadas (la que llevan las colocaciones). */
  index: number;
  /** `remnant` = un retazo guardado; `container` = la hoja (la cama o el material). */
  source: 'remnant' | 'container';
  remnant_id: string | null;
  /** Caja `[x0, y0, x1, y1]` de la hoja en las coordenadas en que van las colocaciones. */
  bounds: [number, number, number, number];
  /** 0..100: área colocada / área de esta hoja. */
  utilization: number;
  placed: number;
  area_mm2: number;
  placed_area_mm2: number;
  /** Retazos que sobran (solo al terminar): cada uno son contornos cerrados (exterior y huecos), en las coordenadas de la hoja. */
  leftovers: [number, number][][][];
}

export interface NestStatus {
  running: boolean;
  elapsed_ms: number;
  evaluations: number;
  /** 0..100: área neta colocada / área neta de las hojas usadas. */
  utilization: number;
  placed: number;
  total: number;
  /** Mejor solución hasta ahora (solo las piezas colocadas). */
  placements: NestPlacement[];
  /** Hojas usadas, en orden. */
  sheets: NestSheet[];
  /** Las que no cupieron. */
  unplaced: NestRef[];
  /** Terminó por el tope de tiempo. */
  timed_out: boolean;
  /** Mensaje en español si la petición no es válida. */
  error: string | null;
}
