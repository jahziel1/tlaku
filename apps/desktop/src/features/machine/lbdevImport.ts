import { getApi, getApiKind } from '@/api';
import type { DeviceProfile, LbdevImport, LbdevNotice } from '@/api/types';
import { baseName } from '@/features/history/paths';
import { t, tn, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';

/** Extensión del perfil de dispositivo que guarda LightBurn. */
export const LBDEV_EXTENSION = 'lbdev';

/** Ruta que se usa en modo mock (no hay diálogo nativo). */
export const MOCK_LBDEV_PATH = 'perfil-de-ejemplo.lbdev';

/**
 * Diálogo para elegir el `.lbdev`. En modo mock devuelve una ruta fija. Acepta también `.json`
 * porque los perfiles copiados de un foro suelen guardarse así.
 */
export async function pickLbdevToOpen(): Promise<string | null> {
  if (getApiKind() !== 'tauri') return MOCK_LBDEV_PATH;
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.open({
    multiple: false,
    directory: false,
    filters: [{ name: t('devices.lbdev.file.filter'), extensions: [LBDEV_EXTENSION, 'json'] }],
    title: t('devices.lbdev.pickTitle'),
  });
  return typeof picked === 'string' ? picked : null;
}

/** Perfiles propuestos por un archivo, listos para mostrar el resumen. */
export interface LbdevPreview {
  file: string;
  result: LbdevImport;
}

/** Elige y lee un `.lbdev` (no guarda nada). `null` si se canceló o falló (el error se avisa). */
export async function readLbdevPreview(): Promise<LbdevPreview | null> {
  const path = await pickLbdevToOpen();
  if (!path) return null;
  try {
    const result = await getApi().readLbdev({ path });
    return { file: baseName(path), result };
  } catch (e) {
    useStore.getState().notify('error', t('devices.lbdev.err', { error: errorMessage(e) }));
    return null;
  }
}

/** Palabras de «start, end» en el idioma de la interfaz. */
function scriptWords(detail: string): string {
  return detail
    .split(',')
    .map((w) => w.trim())
    .map((w) => (w === 'start' ? t('devices.lbdev.script.start') : w === 'end' ? t('devices.lbdev.script.end') : w))
    .join(' / ');
}

/** Texto de un aviso en el idioma de la interfaz. */
export function lbdevNoticeText(n: LbdevNotice): string {
  const device = n.device;
  const detail = n.detail;
  const key = (k: string): I18nKey => `devices.lbdev.notice.${k}` as I18nKey;
  switch (n.kind) {
    case 'not_imported':
      return t(key('notImported'), { device, detail });
    case 'invalid_value':
      return t(key('invalid'), { device, detail });
    case 'missing_value':
      return t(key('missing'), { device, detail });
    case 'unsupported_controller':
      return t(key('unsupported'), { device, detail });
    case 'skipped_entry':
      return t(key('skipped'), { detail });
    case 'simulator_values':
      return t(key('sim'), { device });
    case 'script_code':
      return t(key('script'), { device, detail: scriptWords(detail) });
    case 'z_axis_check':
      return t(key('z'), { device });
    case 'origin_inferred':
      return t(key('origin'), { device, detail });
    case 'connection_type':
      return t(key('connection'), { device, detail });
    case 'ruida_host':
      return t(key('ruidaHost'), { device });
  }
}

/**
 * Guarda los perfiles propuestos, uno tras otro (cada uno pasa por la validación del almacén).
 * Se detiene en el primero que falla. Devuelve los guardados.
 */
export async function saveLbdevDevices(devices: readonly DeviceProfile[]): Promise<DeviceProfile[]> {
  const saved: DeviceProfile[] = [];
  for (const d of devices) {
    if (!(await useStore.getState().saveDevice(d))) break;
    saved.push(d);
  }
  const n = saved.length;
  const s = useStore.getState();
  if (n === devices.length) s.notify('success', tn('devices.lbdev.done', n));
  else s.notify('warning', t('devices.lbdev.partial', { done: n, total: devices.length }));
  return saved;
}
