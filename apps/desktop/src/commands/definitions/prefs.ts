import { useSettingsUi } from '@/features/settings/uiStore';
import type { CommandDef } from '../types';

/**
 * Preferencias. `Mod+,` es el atajo habitual para abrirlas (Cmd+, en macOS, Ctrl+, en el resto).
 * Van en la categoría Edición: es el menú donde cae lo que ningún menú menciona.
 */
export const prefsCommands: CommandDef[] = [
  { id: 'prefs.open', label: 'prefs.cmd.open', category: 'edit', shortcuts: ['Mod+,'], run: () => useSettingsUi.getState().openSettings() },
  { id: 'prefs.shortcuts', label: 'prefs.cmd.shortcuts', category: 'edit', run: () => useSettingsUi.getState().openSettings('shortcuts') },
];
