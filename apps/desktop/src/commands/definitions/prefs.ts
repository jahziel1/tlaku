import { useSettingsUi } from '@/features/settings/uiStore';
import { DEFAULT_DOCK_LAYOUT } from '@/lib/dockLayout';
import { t } from '@/i18n';
import { resetPanelSections } from '@/ui/PanelSection';
import type { CommandDef } from '../types';

/**
 * Preferencias. `Mod+,` es el atajo habitual para abrirlas (Cmd+, en macOS, Ctrl+, en el resto).
 * Van en la categoría Edición: es el menú donde cae lo que ningún menú menciona.
 */
export const prefsCommands: CommandDef[] = [
  { id: 'prefs.open', label: 'prefs.cmd.open', category: 'edit', shortcuts: ['Mod+,'], run: () => useSettingsUi.getState().openSettings() },
  { id: 'prefs.shortcuts', label: 'prefs.cmd.shortcuts', category: 'edit', run: () => useSettingsUi.getState().openSettings('shortcuts') },
  {
    id: 'prefs.resetLayout',
    label: 'prefs.cmd.resetLayout',
    category: 'panels',
    run: (s) => {
      s.setPref('dockLayout', DEFAULT_DOCK_LAYOUT);
      s.notify('info', t('prefs.dock.resetDone'));
    },
  },
  // Vuelve las secciones plegables de los paneles a su estado de fábrica (solo las más usadas abiertas).
  {
    id: 'prefs.resetPanel',
    label: 'prefs.panel.resetSections',
    category: 'panels',
    run: (s) => {
      resetPanelSections();
      s.notify('info', t('prefs.panel.resetDone'));
    },
  },
];
