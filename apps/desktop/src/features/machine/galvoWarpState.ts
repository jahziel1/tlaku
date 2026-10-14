import { create } from 'zustand';

/** Estado de interfaz de la vista previa de la deformación de cilindro y cono (no se guarda con el perfil). */
interface WarpUi {
  show: boolean;
  setShow: (v: boolean) => void;
}

export const useWarpUi = create<WarpUi>((set) => ({
  show: false,
  setShow: (show) => set({ show }),
}));
