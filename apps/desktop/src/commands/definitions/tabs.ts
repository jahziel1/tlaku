import { closeOtherTabs, closeTab, openNewTab, saveAllTabs } from '@/features/tabs/actions';
import { sendTabToNewWindow } from '@/features/tabs/handoff';
import { tabViews } from '@/store';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';
import type { I18nKey } from '@/i18n';

/** Los comandos de pestañas actúan sobre la pestaña ACTIVA. El menú de la pestaña (clic derecho) llama a las mismas funciones con la suya. */
const several = (s: AppState): true | I18nKey => (s.tabs.length > 1 ? true : 'tabs.reason.onlyOne');

export const tabCommands: CommandDef[] = [
  { id: 'tabs.new', label: 'tabs.cmd.new', category: 'file', shortcuts: ['Mod+T'], run: () => openNewTab() },
  { id: 'tabs.close', label: 'tabs.cmd.close', category: 'file', shortcuts: ['Mod+W'], run: async (s) => void (await closeTab(s.activeTabId)) },
  { id: 'tabs.closeOthers', label: 'tabs.cmd.closeOthers', category: 'file', enabled: several, run: async (s) => void (await closeOtherTabs(s.activeTabId)) },
  { id: 'tabs.next', label: 'tabs.cmd.next', category: 'view', shortcuts: ['Mod+Tab'], enabled: several, run: (s) => s.cycleTab(1) },
  { id: 'tabs.prev', label: 'tabs.cmd.prev', category: 'view', shortcuts: ['Mod+Shift+Tab'], enabled: several, run: (s) => s.cycleTab(-1) },
  {
    id: 'tabs.duplicate',
    label: 'tabs.cmd.duplicate',
    category: 'file',
    run: (s) => {
      if (s.duplicateTab(s.activeTabId)) {
        s.notifyKey('info', 'tabs.duplicated');
        void s.computePreview();
      }
    },
  },
  { id: 'tabs.moveToWindow', label: 'tabs.cmd.moveToWindow', category: 'file', enabled: several, run: async (s) => void (await sendTabToNewWindow(s.activeTabId)) },
  {
    id: 'tabs.saveAll',
    label: 'tabs.cmd.saveAll',
    category: 'file',
    enabled: (s) => (tabViews(s).some((v) => v.dirty) ? true : 'tabs.reason.nothingToSave'),
    run: async () => void (await saveAllTabs()),
  },
];
