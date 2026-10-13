import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import type { Connection } from '@/bindings/Connection';
import { createMockApi } from '@/api/mock';
import { createAppStore, bindMachineEvents } from '@/store';
import { selectIsVirtual } from '@/store/machine';

describe('conexión por red en el store', () => {
  let store: ReturnType<typeof createAppStore>;

  beforeEach(async () => {
    const api = createMockApi({ latencyMs: 0, statusIntervalMs: 20 });
    setApi(api);
    store = createAppStore();
    await store.getState().loadDevices();
    await bindMachineEvents(api, store);
  });

  const useConnection = (connection: Connection) => {
    const s = store.getState();
    const dev = s.devices.find((d) => d.id === s.activeDeviceId) ?? s.devices[0]!;
    store.setState({ devices: s.devices.map((d) => (d.id === dev.id ? { ...d, connection } : d)), activeDeviceId: dev.id });
  };

  it('un perfil TCP conecta a su dirección sin elegir puerto serie', async () => {
    useConnection({ type: 'tcp', host: '192.168.1.50', port: 23 });
    store.getState().setSelectedPort(null);
    await store.getState().connect();
    expect(store.getState().connection).toMatchObject({ connected: true, port: 'tcp://192.168.1.50:23' });
    expect(store.getState().notices.some((n) => n.level === 'warning')).toBe(false);
  });

  it('un perfil WebSocket conecta con su URL tal cual', async () => {
    useConnection({ type: 'web_socket', url: 'ws://fluidnc.local:81/' });
    await store.getState().connect();
    expect(store.getState().connection.port).toBe('ws://fluidnc.local:81/');
  });

  it('un perfil serie sigue usando el puerto elegido', async () => {
    useConnection({ type: 'serial' });
    await store.getState().refreshPorts();
    store.getState().setSelectedPort('/dev/cu.usbserial-1420');
    await store.getState().connect();
    expect(store.getState().connection.port).toBe('/dev/cu.usbserial-1420');
  });

  it('la máquina virtual por red se ofrece, conecta y queda marcada como virtual', async () => {
    useConnection({ type: 'serial' });
    await store.getState().refreshPorts();
    expect(store.getState().ports).toContain('virtualnet:falcon2');
    store.getState().setSelectedPort('virtualnet:falcon2');
    store.getState().setVirtualSpeed(5);
    await store.getState().connect();
    expect(store.getState().connection.port).toBe('virtualnet:falcon2@5');
    expect(selectIsVirtual(store.getState())).toBe(true);
    expect(store.getState().firmware).toBe('Grbl 1.1f');
  });
});
