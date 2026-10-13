/**
 * Tipos del contrato de importar con opciones, exportar e imprimir (`src-tauri/src/edit/files.rs`
 * y `crates/tlaku-export`). Los campos van en snake_case, como el resto del contrato.
 */

export type ExportFormat = 'svg' | 'dxf' | 'png' | 'pdf';

export type ImageHandling = 'skip' | 'link' | 'embed';

export interface SvgOptions {
  images: ImageHandling;
  /** Grosor del trazo en mm. */
  stroke_mm: number;
  margin_mm: number;
  /** Conserva las coordenadas del documento en vez de ajustar la página al diseño. */
  keep_position: boolean;
  fill_layers: boolean;
}

export type DxfCurves = 'splines' | 'flatten';

export interface DxfOptions {
  curves: DxfCurves;
  tolerance_mm: number;
  true_color: boolean;
}

export interface PngOptions {
  dpi: number;
  margin_mm: number;
  transparent: boolean;
  stroke_mm: number;
  fill_layers: boolean;
}

export type PageSize = 'fit' | 'a4' | 'a3' | 'letter' | 'legal';

export interface PdfOptions {
  page: PageSize;
  margin_mm: number;
  stroke_mm: number;
  images: boolean;
  fill_layers: boolean;
}

/** Qué formato exportar y con qué opciones; las que falten toman el valor por defecto del motor. */
export type ExportRequest =
  | ({ format: 'svg' } & Partial<SvgOptions>)
  | ({ format: 'dxf' } & Partial<DxfOptions>)
  | ({ format: 'png' } & Partial<PngOptions>)
  | ({ format: 'pdf' } & Partial<PdfOptions>);

export interface ExportSummary {
  path: string;
  bytes: number;
  /** Tamaño del dibujo en mm, `[ancho, alto]`. */
  size_mm: [number, number];
  warnings: string[];
}

/** Opciones del diálogo de importación (D91). */
export interface ImportDialogOptions {
  /** Escala uniforme en porcentaje (100 = sin cambio). */
  scale_percent: number;
  /** Resolución (ppp) del SVG sin unidades y de las imágenes; `null` = 96. */
  dpi: number | null;
  /** DXF sin unidades: mm que vale una unidad del dibujo; `null` = milímetros. */
  dxf_unit_mm: number | null;
  group: boolean;
  /**
   * Agrupa por contención al importar (D31): cada pieza con los huecos y grabados que quedan dentro de
   * su contorno. Lo hace la interfaz después de importar (`lib/autogroup.ts`); el motor no lo recibe.
   */
  auto_group: boolean;
  /** Capa destino de los vectores (`#rrggbb`); `null` = una capa por color. */
  target_layer: string | null;
  /** PDF y AI: página a importar, desde 1. */
  pdf_page: number;
  /** G-code: una capa por nivel de potencia (`true`) o una sola capa. */
  gcode_by_power: boolean;
}

export const DEFAULT_IMPORT_OPTIONS: ImportDialogOptions = {
  scale_percent: 100,
  dpi: null,
  dxf_unit_mm: null,
  group: false,
  auto_group: false,
  target_layer: null,
  pdf_page: 1,
  gcode_by_power: true,
};

// Los mismos valores por defecto que `Default` de cada estructura en Rust.
export const DEFAULT_SVG: SvgOptions = { images: 'embed', stroke_mm: 0.1, margin_mm: 0, keep_position: false, fill_layers: false };
export const DEFAULT_DXF: DxfOptions = { curves: 'splines', tolerance_mm: 0.005, true_color: true };
export const DEFAULT_PNG: PngOptions = { dpi: 300, margin_mm: 2, transparent: false, stroke_mm: 0.1, fill_layers: true };
export const DEFAULT_PDF: PdfOptions = { page: 'fit', margin_mm: 5, stroke_mm: 0.1, images: true, fill_layers: false };

/** Extensión del archivo de cada formato. */
export const EXPORT_EXTENSION: Record<ExportFormat, string> = { svg: 'svg', dxf: 'dxf', png: 'png', pdf: 'pdf' };
