/**
 * Opciones de importación que la app recuerda entre sesiones (preferencia `importOptions`): la
 * última elección del diálogo «Importar con opciones». La escala y la página del PDF no se
 * recuerdan: valen para el archivo del momento, no para el siguiente.
 */
export interface RememberedImport {
  /** Resolución elegida (ppp); `null` = la predeterminada de las preferencias. */
  dpi: number | null;
  /** DXF sin unidades: mm que vale una unidad; `null` = milímetros. */
  dxf_unit_mm: number | null;
  group: boolean;
  auto_group: boolean;
  /** Capa destino (`#rrggbb`); `null` = una capa por color. */
  target_layer: string | null;
  gcode_by_power: boolean;
}

export const DEFAULT_REMEMBERED_IMPORT: RememberedImport = {
  dpi: null,
  dxf_unit_mm: null,
  group: false,
  auto_group: false,
  target_layer: null,
  gcode_by_power: true,
};

/** Resolución mínima y máxima (ppp) que acepta la preferencia y el diálogo. */
export const IMPORT_DPI_MIN = 10;
export const IMPORT_DPI_MAX = 2400;
/** Resolución de fábrica del motor para un SVG sin unidades o una imagen. */
export const IMPORT_DPI_DEFAULT = 96;

const bool = (v: unknown, fallback: boolean): boolean => (typeof v === 'boolean' ? v : fallback);

/** Opciones recordadas válidas a partir de cualquier dato; `null` si no es un objeto. Lo inválido toma el valor de fábrica. */
export function sanitizeRememberedImport(raw: unknown): RememberedImport | null {
  if (!raw || typeof raw !== 'object') return null;
  const r = raw as Record<string, unknown>;
  const dpi = typeof r.dpi === 'number' && Number.isFinite(r.dpi) ? Math.round(Math.min(IMPORT_DPI_MAX, Math.max(IMPORT_DPI_MIN, r.dpi))) : null;
  const unit = typeof r.dxf_unit_mm === 'number' && Number.isFinite(r.dxf_unit_mm) && r.dxf_unit_mm > 0 && r.dxf_unit_mm <= 1000 ? r.dxf_unit_mm : null;
  const layer = typeof r.target_layer === 'string' && /^#[0-9a-f]{6}$/i.test(r.target_layer) ? r.target_layer : null;
  const group = bool(r.group, false);
  // Agrupar y agrupar por contención se excluyen entre sí en el diálogo.
  const autoGroup = group ? false : bool(r.auto_group, false);
  return { dpi, dxf_unit_mm: unit, group, auto_group: autoGroup, target_layer: layer, gcode_by_power: bool(r.gcode_by_power, true) };
}

/** Lo que se recuerda de las opciones que eligió el usuario en el diálogo. */
export function rememberImport(o: RememberedImport): RememberedImport {
  return sanitizeRememberedImport(o) ?? DEFAULT_REMEMBERED_IMPORT;
}

/**
 * Resolución que se manda al motor: la elegida o, si el diálogo quedó en «predeterminada», la de las
 * preferencias (`null` mientras sea la de fábrica, que el motor ya usa por sí solo).
 */
export function effectiveImportDpi(chosen: number | null, prefDpi: number): number | null {
  if (chosen !== null) return chosen;
  return prefDpi === IMPORT_DPI_DEFAULT ? null : prefDpi;
}
