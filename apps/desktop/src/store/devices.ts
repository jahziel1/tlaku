import { getApi } from '@/api';
import type { DeviceProfile } from '@/api/types';
import { t } from '@/i18n';
import { normalizeDevice, validateDevice } from '@/lib/device';
import { errorMessage } from '@/lib/errors';
import { readPref, writePref } from '@/lib/platform';
import type { SliceCreator } from './types';

const LAST_DEVICE_PREF = 'lastDevice';

/**
 * Recuerda la máquina «de siempre» de este equipo: la última con la que se conectó o la que se
 * acaba de agregar con el asistente. Al abrir la app manda sobre la estrella «por defecto».
 */
export function rememberDevice(id: string): void {
  writePref(LAST_DEVICE_PREF, id);
}

/** Id de la última máquina recordada (puede ya no existir), o `null`. */
export function rememberedDevice(): string | null {
  const id = readPref(LAST_DEVICE_PREF);
  return id ? id : null;
}

export interface DevicesSlice {
  devices: DeviceProfile[];
  activeDeviceId: string | null;
  devicesLoading: boolean;

  loadDevices: () => Promise<void>;
  setActiveDevice: (id: string) => void;
  /** Guarda (crea o reemplaza) un perfil. Devuelve si quedó guardado. */
  saveDevice: (device: DeviceProfile) => Promise<boolean>;
  /** Ordena la lista como `ids` (el orden se guarda en `devices.json`). Devuelve si se guardó. */
  reorderDevices: (ids: string[]) => Promise<boolean>;
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
      // Orden: la del documento, la que ya está activa y, al abrir la app, la última con la que se
      // conectó (o agregó). Si nunca se conectó (o ya no existe) manda la marcada «por defecto».
      const known = (id: string | null | undefined) => (id ? devices.find((d) => d.id === id)?.id : undefined);
      const activeDeviceId =
        known(get().doc.device_id) ?? known(get().activeDeviceId) ?? known(rememberedDevice()) ?? devices.find((d) => d.is_default)?.id ?? devices[0]?.id ?? null;
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

  reorderDevices: async (ids) => {
    try {
      await getApi().reorderDevices({ ids });
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
