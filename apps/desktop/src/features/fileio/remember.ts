import { DEFAULT_REMEMBERED_IMPORT, type RememberedImport } from '@/lib/importOptions';
import { DEFAULT_IMPORT_OPTIONS, type ImportDialogOptions } from './types';

/**
 * Opciones con que arranca el diálogo de importar: las de fábrica y, si se recuerdan, lo elegido la
 * vez anterior. La capa destino solo se conserva si el documento actual todavía la tiene.
 */
export function initialImportOptions(remembered: RememberedImport | null, layerColors: readonly string[]): ImportDialogOptions {
  if (!remembered) return DEFAULT_IMPORT_OPTIONS;
  const layer = remembered.target_layer;
  const known = layer !== null && layerColors.some((c) => c.toLowerCase() === layer.toLowerCase());
  return {
    ...DEFAULT_IMPORT_OPTIONS,
    dpi: remembered.dpi,
    dxf_unit_mm: remembered.dxf_unit_mm,
    group: remembered.group,
    auto_group: remembered.auto_group,
    target_layer: known ? layerColors.find((c) => c.toLowerCase() === layer.toLowerCase())! : null,
    gcode_by_power: remembered.gcode_by_power,
  };
}

/** Lo que se recuerda de una elección del diálogo (sin la escala ni la página del PDF). */
export function rememberedFrom(o: ImportDialogOptions): RememberedImport {
  return {
    ...DEFAULT_REMEMBERED_IMPORT,
    dpi: o.dpi,
    dxf_unit_mm: o.dxf_unit_mm,
    group: o.group,
    auto_group: o.auto_group,
    target_layer: o.target_layer,
    gcode_by_power: o.gcode_by_power,
  };
}
