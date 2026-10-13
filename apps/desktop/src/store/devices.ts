import { getApi } from '@/api';
import type { DeviceProfile } from '@/api/types';
import { t } from '@/i18n';
import { normalizeDevice, validateDevice } from '@/lib/device';
import { errorMessage } from '@/lib/errors';
import type { SliceCreator } from './types';

export interface DevicesSlice {
  devices: DeviceProfile[];
  activeDeviceId: string | null;
  devicesLoading: boolean;

  loadDevices: () => Promise<void>;
  setActiveDevice: (id: string) => void;
  /** Guarda (crea o reemplaza) un perfil. Devuelve si quedó guardado. */
  saveDevice: (device: DeviceProfile) => Promise<boolean>;
  /** Borra un perfil. No deja la lista vacía ni borra la máquina conectada. Devuelve si se borró. */
  removeDevice: (id: string) => Promise<boolean>;
}

export const createDevicesSlice: SliceCreator<DevicesSlice> = (set, get) => ({
  devices: [],
  activeDeviceId: null,
  devicesLoading: false,

  loadDevices: async () => {
    set({ devicesLoading: true });
    try {
      // Perfiles guardados antes de la fase 4 llegan sin Z, rotativo ni corrección de barrido.
      const devices = (await getApi().listDevices()).map(normalizeDevice);
      const wanted = get().doc.device_id ?? get().activeDeviceId;
      const activeDeviceId = devices.find((d) => d.id === wanted)?.id ?? devices[0]?.id ?? null;
      set({ devices, activeDeviceId, devicesLoading: false });
      get().fitBed();
    } catch (e) {
      set({ devicesLoading: false });
      get().notify('error', errorMessage(e));
    }
  },

  setActiveDevice: (id) => {
    if (!get().devices.some((d) => d.id === id)) return;
    // El dispositivo del documento no entra en el historial: cambiarlo no es una edición del diseño.
    set((s) => ({ activeDeviceId: id, doc: { ...s.doc, device_id: id }, dirty: true }));
    get().fitBed();
  },

  saveDevice: async (device) => {
    // El gestor ya valida, pero un perfil roto nunca debe llegar a la máquina ni al disco.
    if (Object.keys(validateDevice(device)).length > 0) {
      get().notifyKey('error', 'machine.dev.invalid');
      return false;
    }
    try {
      await getApi().saveDevice({ device });
      await get().loadDevices();
      return true;
    } catch (e) {
      get().notify('error', errorMessage(e));
      return false;
    }
  },

  removeDevice: async (id) => {
    const s = get();
    if (s.devices.length <= 1) {
      s.notifyKey('warning', 'machine.dev.lastOne');
      return false;
    }
    if (s.connection.connected && s.activeDeviceId === id) {
      s.notify('warning', t('machine.dev.inUse'));
      return false;
    }
    try {
      await getApi().deleteDevice({ id });
      await get().loadDevices();
      return true;
    } catch (e) {
      get().notify('error', errorMessage(e));
      return false;
    }
  },
});
