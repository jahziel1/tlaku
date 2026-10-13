import { usePrefsBoot } from './boot';
import { SettingsDialog } from './SettingsDialog';
import { useSettingsUi } from './uiStore';

export { useSettingsUi, type SettingsSection } from './uiStore';
export { exportPrefs, importPrefs } from './files';

/**
 * Punto de montaje de las preferencias (lo monta el shell una sola vez): arranca la lectura y el
 * guardado de las preferencias y pinta la ventana cuando está abierta.
 */
export function SettingsHost() {
  usePrefsBoot();
  const open = useSettingsUi((s) => s.open);
  return open ? <SettingsDialog /> : null;
}
