import { getApiKind } from '@/api';
import { t } from '@/i18n';

export type PickKind = 'import' | 'openProject' | 'saveProject' | 'exportGcode';

/** Extensiones por tipo de diálogo. */
const FILTERS: Record<PickKind, { name: string; extensions: string[] }[]> = {
  import: [
    { name: 'Diseños', extensions: ['svg', 'dxf', 'png', 'jpg', 'jpeg', 'bmp'] },
  ],
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

/** Pregunta si se descartan los cambios sin guardar. */
export async function confirmDiscard(): Promise<boolean> {
  if (getApiKind() === 'tauri') {
    const dialog = await import('@tauri-apps/plugin-dialog');
    return dialog.ask(t('dialog.discard.message'), {
      title: t('dialog.discard.title'),
      kind: 'warning',
      okLabel: t('dialog.discard.confirm'),
      cancelLabel: t('dialog.discard.cancel'),
    });
  }
  return window.confirm(t('dialog.discard.message'));
}
