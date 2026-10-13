/** Opciones de "Convertir a mapa de bits" (mismas que `BitmapOptions` de src-tauri/src/edit/trace.rs). */
export interface BitmapOptions {
  /** Puntos por pulgada del PNG. */
  dpi: number;
  /** Rellena las formas cerradas; si no, solo dibuja el contorno. */
  filled: boolean;
  /** Grosor del contorno en mm (con `filled` apagado). */
  stroke_mm: number;
  /** Negativo: las formas en blanco sobre fondo negro. */
  invert: boolean;
}

export interface BitmapResult {
  path: string;
  width_px: number;
  height_px: number;
  /** Tamaño de la imagen en mm. */
  size_mm: [number, number];
  /** Esquina inferior izquierda de la imagen en el documento (mm). */
  origin_mm: [number, number];
  /** El proyecto no estaba guardado y el PNG quedó en la carpeta temporal. */
  in_temp_dir: boolean;
}

export interface ProcessedSummary {
  path: string;
  bytes: number;
  width_px: number;
  height_px: number;
}
