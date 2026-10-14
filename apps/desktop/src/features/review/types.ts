import type { Document } from '@/api/types';

/** Qué encontró «Revisar diseño». Coincide con `FindingKind` de `src-tauri/src/edit/review.rs`. */
export type FindingKind =
  | 'near_closed'
  | 'overlap'
  | 'joinable'
  | 'tiny'
  | 'extra_nodes'
  | 'outside_all'
  | 'outside_part'
  | 'text'
  | 'silent_layer'
  | 'hidden_layer'
  | 'image_dpi_high'
  | 'image_dpi_low';

export type Severity = 'serious' | 'warning' | 'info';

export interface Finding {
  /** Estable para el mismo contenido (tipo, capa y objetos): sirve para «Ignorar». */
  key: string;
  kind: FindingKind;
  severity: Severity;
  /** Color `#rrggbb` de la capa, o `null` si el hallazgo no es de una sola capa. */
  layer: string | null;
  /** Objetos afectados (los que se seleccionan). */
  object_ids: string[];
  /** Objetos que hay que pasarle a la operación de arreglo; ausente = `object_ids`. */
  fix_ids?: string[];
  /** Cuántos casos hay (huecos, uniones, objetos...), según el tipo. */
  count: number;
  /** Número principal del texto: mm, resolución real, nodos que sobran... */
  value: number;
  /** Número secundario: resolución pedida, textos sin contorno... */
  detail: number;
  /** Tolerancia (mm) con la que se detectó; el arreglo usa la misma. */
  tolerance: number | null;
}

/** Umbrales opcionales de la revisión (los de fábrica son los de las operaciones de limpieza). */
export interface ReviewOptions {
  close_mm?: number;
  join_mm?: number;
  dedupe_mm?: number;
  tiny_mm?: number;
  node_mm?: number;
  dpi_high?: number;
  dpi_low?: number;
}

export interface ReviewArgs {
  doc: Document;
  /** Ancho y alto de la cama, mm. */
  bed: [number, number];
  options?: ReviewOptions;
}
