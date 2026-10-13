import { create } from 'zustand';

interface WizardUi {
  open: boolean;
  openWizard: () => void;
  close: () => void;
}

/** Estado de interfaz del asistente "Agregar máquina": solo si está abierto. El resto vive en el propio asistente. */
export const useDevicesWizard = create<WizardUi>()((set) => ({
  open: false,
  openWizard: () => set({ open: true }),
  close: () => set({ open: false }),
}));
