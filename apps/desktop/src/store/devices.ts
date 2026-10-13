import { getApi } from '@/api';
import type { DeviceProfile } from '@/api/types';
import { errorMessage } from '@/lib/errors';
import type { SliceCreator } from './types';

export interface DevicesSlice {
  devices: DeviceProfile[];
  activeDeviceId: string | null;
  devicesLoading: boolean;

  loadDevices: () => Promise<void>;
  setActiveDevice: (id: string) => void;
  saveDevice: (device: DeviceProfile) => Promise<void>;
  removeDevice: (id: string) => Promise<void>;
}

export const createDevicesSlice: SliceCreator<DevicesSlice> = (set, get) => ({
  devices: [],
  activeDeviceId: null,
  devicesLoading: false,

  loadDevices: async () => {
    set({ devicesLoading: true });
    try {
      const devices = await getApi().listDevices();
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
    try {
      await getApi().saveDevice({ device });
      await get().loadDevices();
    } catch (e) {
      get().notify('error', errorMessage(e));
    }
  },

  removeDevice: async (id) => {
    try {
      await getApi().deleteDevice({ id });
      await get().loadDevices();
    } catch (e) {
      get().notify('error', errorMessage(e));
    }
  },
});
