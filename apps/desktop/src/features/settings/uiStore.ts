import { create } from 'zustand';

export type SettingsSection = 'general' | 'editing' | 'appearance' | 'files' | 'automation' | 'remote' | 'shortcuts';

export const SETTINGS_SECTIONS: readonly SettingsSection[] = ['general', 'editing', 'appearance', 'files', 'automation', 'remote', 'shortcuts'];

interface SettingsUi {
  open: boolean;
  section: SettingsSection;
  openSettings: (section?: SettingsSection) => void;
  closeSettings: () => void;
  setSection: (section: SettingsSection) => void;
}

/** Estado de interfaz de la ventana de Preferencias (abierta o no, sección visible). */
export const useSettingsUi = create<SettingsUi>()((set) => ({
  open: false,
  section: 'general',
  openSettings: (section) => set((s) => ({ open: true, section: section ?? s.section })),
  closeSettings: () => set({ open: false }),
  setSection: (section) => set({ section }),
}));
