import { t } from '@/i18n';
import { defaultsLevel, layerTemplate, setColorDefaults, setDefaultsDevice, setLayerDefaults, withColorDefaults, withoutColorDefaults } from '@/lib/layers';
import type { AppState } from '@/store';
import { useStore } from '@/store';
import { selectActiveDevice } from '@/store/selectors';

/**
 * Valores por defecto de las capas nuevas (C35). Hay tres niveles, del más específico al más
 * general: color y máquina (`layerColorDefaults` con `device`), solo el color (`device: null`) y el
 * juego general (`layerDefaults`). Viven en las preferencias y `lib/layers.ts` los usa al crear una
 * capa; este módulo mantiene ambos al día (incluida la máquina activa).
 */
export function syncLayerDefaults(): void {
  const s = useStore.getState();
  setLayerDefaults(s.prefs.layerDefaults);
  setColorDefaults(s.prefs.layerColorDefaults);
  setDefaultsDevice(s.activeDeviceId);
}

// Al cargar el módulo y cada vez que cambian las preferencias (se leen del disco después de arrancar).
syncLayerDefaults();
useStore.subscribe((s, prev) => {
  if (s.prefs.layerDefaults !== prev.prefs.layerDefaults) setLayerDefaults(s.prefs.layerDefaults);
  if (s.prefs.layerColorDefaults !== prev.prefs.layerColorDefaults) setColorDefaults(s.prefs.layerColorDefaults);
  if (s.activeDeviceId !== prev.activeDeviceId) setDefaultsDevice(s.activeDeviceId);
});

/** Dónde vale un juego de valores por color: en la máquina activa o en cualquiera. */
export type ColorScope = 'machine' | 'color';

/** Id de máquina de un alcance; `null` si no hay máquina activa para el alcance `machine`. */
function scopeDevice(s: AppState, scope: ColorScope): string | null | undefined {
  if (scope === 'color') return null;
  return s.activeDeviceId ?? undefined;
}

/** Hay valores guardados para este color en este alcance. */
export function hasColorDefaults(s: Pick<AppState, 'prefs' | 'activeDeviceId'>, color: string, scope: ColorScope): boolean {
  const device = scope === 'color' ? null : s.activeDeviceId;
  if (scope === 'machine' && device === null) return false;
  const c = color.toLowerCase();
  return s.prefs.layerColorDefaults.some((e) => e.color === c && e.device === device);
}

/** Nivel que manda hoy para este color en la máquina activa (lo muestra el editor de capa). */
export function currentDefaultsLevel(s: Pick<AppState, 'prefs' | 'activeDeviceId'>, color: string) {
  return defaultsLevel(s.prefs.layerColorDefaults, s.prefs.layerDefaults, color, s.activeDeviceId);
}

/** Guarda los ajustes de esta capa como valores por defecto de su color: en esta máquina o en cualquiera. */
export function saveColorDefaults(s: AppState, color: string, scope: ColorScope): void {
  const layer = s.doc.layers.find((l) => l.color === color);
  const device = scopeDevice(s, scope);
  if (!layer || device === undefined) return;
  const list = withColorDefaults(s.prefs.layerColorDefaults, color, device, layerTemplate(layer));
  s.setPref('layerColorDefaults', list);
  setColorDefaults(list);
  s.notify('success', scope === 'machine' ? t('layers.defaults.savedMachine', { color, device: selectActiveDevice(s).name }) : t('layers.defaults.savedColor', { color }));
}

/** Quita los valores por defecto de este color en este alcance. */
export function removeColorDefaults(s: AppState, color: string, scope: ColorScope): void {
  const device = scopeDevice(s, scope);
  if (device === undefined) return;
  const list = withoutColorDefaults(s.prefs.layerColorDefaults, color, device);
  s.setPref('layerColorDefaults', list);
  setColorDefaults(list);
  s.notify('success', t('layers.defaults.removedColor', { color }));
}

/** Guarda los ajustes de esta capa como los valores con que empiezan las capas nuevas. */
export function saveLayerAsDefaults(s: AppState, color: string): void {
  const layer = s.doc.layers.find((l) => l.color === color);
  if (!layer) return;
  const template = layerTemplate(layer);
  s.setPref('layerDefaults', template);
  // Que `makeLayer` los use en este mismo instante, sin esperar al aviso del store.
  setLayerDefaults(template);
  s.notify('success', t('layers.defaults.saved'));
}

/** Vuelve a los valores de fábrica para las capas nuevas. */
export function resetLayerDefaults(s: AppState): void {
  s.setPref('layerDefaults', null);
  setLayerDefaults(null);
  s.notify('success', t('layers.defaults.restored'));
}
