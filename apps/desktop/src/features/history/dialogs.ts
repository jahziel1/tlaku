import { getApiKind } from '@/api';
import { IMPORT_EXTENSIONS } from './paths';

export type PickKind = 'import' | 'openProject' | 'saveProject' | 'exportGcode';

/** Extensiones por tipo de diálogo. */
const FILTERS: Record<PickKind, { name: string; extensions: string[] }[]> = {
  import: [{ name: 'Diseños', extensions: [...IMPORT_EXTENSIONS] }],
  openProject: [{ name: 'Proyecto LaserSoft', extensions: ['lsproj'] }],
  saveProject: [{ name: 'Proyecto LaserSoft', extensions: ['lsproj'] }],
  exportGcode: [{ name: 'G-code', extensions: ['gcode', 'nc'] }],
};

/** Rutas ficticias para el modo mock (en el navegador no hay diálogos nativos). */
const MOCK_PATHS: Record<PickKind, string> = {
  import: 'ejemplos/placa.svg',
  openProject: 'ejemplos/placa.lsproj',
  saveProject: 'proyectos/sin-titulo.lsproj',
  exportGcode: 'trabajos/placa.gcode',
};

/** Muestra el diálogo de archivos nativo (Tauri) o devuelve una ruta de ejemplo (mock). */
export async function pickPath(kind: PickKind, defaultPath?: string): Promise<string | null> {
  if (getApiKind() !== 'tauri') return MOCK_PATHS[kind];
  const dialog = await import('@tauri-apps/plugin-dialog');
  const filters = FILTERS[kind];
  if (kind === 'import' || kind === 'openProject') {
    const picked = await dialog.open({ multiple: false, directory: false, filters });
    return typeof picked === 'string' ? picked : null;
  }
  return dialog.save({ filters, defaultPath });
}

/** Como `pickPath('import')`, pero permite elegir varios archivos a la vez. */
export async function pickImportPaths(): Promise<string[]> {
  if (getApiKind() !== 'tauri') return [MOCK_PATHS.import];
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.open({ multiple: true, directory: false, filters: FILTERS.import });
  if (Array.isArray(picked)) return picked;
  return typeof picked === 'string' ? [picked] : [];
}
