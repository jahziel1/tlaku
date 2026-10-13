import type { DockTab } from '@/store/view';
import type { I18nKey } from '@/i18n';
import type { CommandDef } from '../types';

const TABS: [DockTab, string][] = [
  ['layers', 'Mod+1'],
  ['machine', 'Mod+2'],
  ['console', 'Mod+3'],
  ['camera', 'Mod+5'],
];

export const panelCommands: CommandDef[] = TABS.map(([tab, key]) => ({
  id: `panel.${tab}`,
  label: `cmd.panel.${tab}` as I18nKey,
  category: 'panels',
  shortcuts: [key],
  checked: (s) => s.dockTab === tab,
  run: (s) => s.setDockTab(tab),
}));
