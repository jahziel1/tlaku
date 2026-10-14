/**
 * Configuraciones de cámara con nombre (M35): guardar la fuente (cámara local o URL de red) y los
 * ajustes del fondo para volver a ellos con un clic. Lógica pura sobre las preferencias; guardar,
 * aplicar y borrar contra el store están al final.
 *
 * La URL de una cámara de red puede llevar `usuario:clave@`: queda en `camera.json` (perfil local)
 * y se quita al exportar (`features/bundle/cameras.ts`).
 */
import type { CameraPreset, CameraPrefs } from '@/api/types';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { persistCameraSetup, updateCameraPrefs } from './actions';
import { startCamera } from './session';

export const MAX_PRESETS = 32;
export const MAX_PRESET_NAME = 60;

/** Foto de las preferencias actuales con un nombre. */
export function presetFromPrefs(name: string, prefs: CameraPrefs): CameraPreset {
  return {
    name: name.trim(),
    source: prefs.source ?? 'local',
    device_id: prefs.device_id,
    device_label: prefs.device_label,
    width: prefs.width,
    height: prefs.height,
    network_url: prefs.network_url ?? null,
    dpm: prefs.dpm ?? null,
    offset_x_mm: prefs.offset_x_mm ?? 0,
    offset_y_mm: prefs.offset_y_mm ?? 0,
    grayscale: prefs.grayscale === true,
  };
}

/** Las preferencias que fija una configuración al aplicarla (la opacidad y lo demás no cambian). */
export function prefsFromPreset(p: CameraPreset): Partial<CameraPrefs> {
  return {
    source: p.source,
    device_id: p.device_id,
    device_label: p.device_label,
    width: p.width,
    height: p.height,
    network_url: p.network_url,
    dpm: p.dpm,
    offset_x_mm: p.offset_x_mm,
    offset_y_mm: p.offset_y_mm,
    grayscale: p.grayscale,
  };
}

export type PresetNameProblem = 'empty' | 'long';

export function checkPresetName(name: string): PresetNameProblem | null {
  const n = name.trim();
  if (n === '') return 'empty';
  return [...n].length > MAX_PRESET_NAME ? 'long' : null;
}

const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

/** Lista con `preset` agregada, o reemplazando a la del mismo nombre (sin distinguir mayúsculas). */
export function upsertPreset(list: readonly CameraPreset[], preset: CameraPreset): CameraPreset[] {
  const at = list.findIndex((p) => same(p.name, preset.name));
  if (at < 0) return [...list, preset];
  return list.map((p, i) => (i === at ? preset : p));
}

export function removePreset(list: readonly CameraPreset[], name: string): CameraPreset[] {
  return list.filter((p) => !same(p.name, name));
}

export function findPreset(list: readonly CameraPreset[], name: string): CameraPreset | null {
  return list.find((p) => same(p.name, name)) ?? null;
}

/** ¿Las preferencias actuales coinciden con la configuración? (para marcar la activa) */
export function matchesPreset(p: CameraPreset, prefs: CameraPrefs): boolean {
  const cur = presetFromPrefs(p.name, prefs);
  return JSON.stringify({ ...cur, name: '' }) === JSON.stringify({ ...p, name: '' });
}

// ---------------------------------------------------------------------------
// Acciones
// ---------------------------------------------------------------------------

/** Guarda las preferencias actuales con el nombre dado. Devuelve el problema, o `null` si se guardó. */
export function savePreset(name: string): PresetNameProblem | 'full' | null {
  const bad = checkPresetName(name);
  if (bad) return bad;
  const s = useStore.getState();
  const list = s.cameraSetup.presets ?? [];
  const exists = findPreset(list, name) !== null;
  if (!exists && list.length >= MAX_PRESETS) return 'full';
  s.setCameraSetup({ ...s.cameraSetup, presets: upsertPreset(list, presetFromPrefs(name, s.cameraSetup.prefs)) });
  void persistCameraSetup();
  s.notify('success', t(exists ? 'camera.presets.updated' : 'camera.presets.saved', { name: name.trim() }));
  return null;
}

/** Aplica la configuración: fija sus preferencias y, si la cámara estaba encendida, la reabre con ellas. */
export function applyPreset(name: string): boolean {
  const s = useStore.getState();
  const preset = findPreset(s.cameraSetup.presets ?? [], name);
  if (!preset) return false;
  updateCameraPrefs(prefsFromPreset(preset));
  if (useStore.getState().cameraStatus !== 'off') void startCamera();
  s.notify('info', t('camera.presets.applied', { name: preset.name }));
  return true;
}

export function deletePreset(name: string): void {
  const s = useStore.getState();
  s.setCameraSetup({ ...s.cameraSetup, presets: removePreset(s.cameraSetup.presets ?? [], name) });
  void persistCameraSetup();
}
