import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import type { LaserApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { bindMachineEvents, DEFAULT_PREFS, useStore } from '@/store';
import {
  applyAutomation,
  automationConfig,
  handleAutomationLoad,
  handleAutomationStart,
  startAutomation,
  useAutomationStatus,
} from './automation';

const memory = new Map<string, string>();

async function connectedMachine(): Promise<MockApi> {
  memory.clear();
  vi.stubGlobal('localStorage', { getItem: (k: string) => memory.get(k) ?? null, setItem: (k: string, v: string) => void memory.set(k, v) });
  const api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 200, jobSpeedup: 1e6 });
  setApi(api);
  const s = () => useStore.getState();
  await s().loadDevices();
  await bindMachineEvents(api, useStore);
  await s().refreshPorts();
  s().setSelectedPort('virtual:falcon2');
  await s().connect();
  await vi.waitFor(() => expect(s().status?.state).toBe('idle'));
  s().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });
  return api;
}

const lastNotice = () => useStore.getState().notices.at(-1);

afterEach(async () => {
  await useStore.getState().disconnect();
  useStore.getState().setPref('jobChecklistRequired', false);
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('configuración desde las preferencias', () => {
  it('de fábrica está apagada, con el puerto por omisión y sin permitir iniciar', () => {
    expect(automationConfig(DEFAULT_PREFS)).toEqual({ enabled: false, port: 47801, allow_start: false });
    expect(DEFAULT_PREFS.automationEnabled).toBe(false);
    expect(DEFAULT_PREFS.automationAllowStart).toBe(false);
  });

  it('el estado real del motor se guarda, también cuando falla', async () => {
    setApi(createMockApi({ latencyMs: 0 }));
    expect(await applyAutomation({ enabled: true, port: 47801, allow_start: false })).toEqual({ running: true, port: 47801, error: null });
    expect(useAutomationStatus.getState().status?.running).toBe(true);
    const busy = await applyAutomation({ enabled: true, port: 80, allow_start: false });
    expect(busy.running).toBe(false);
    expect(busy.error).toMatch(/1024/);
    const api = { automationConfigure: () => Promise.reject('boom') } as unknown as LaserApi;
    setApi(api);
    expect(await applyAutomation({ enabled: true, port: 5000, allow_start: false })).toEqual({ running: false, port: 5000, error: 'boom' });
  });
});

describe('START desde automatización', () => {
  beforeEach(async () => {
    await connectedMachine();
  });

  it('sin permiso explícito abre el diálogo de inicio y no inicia nada', () => {
    const start = vi.spyOn(useStore.getState(), 'startJob');
    handleAutomationStart(false);
    expect(useStore.getState().dialog).toEqual({ kind: 'start' });
    expect(start).not.toHaveBeenCalled();
    expect(lastNotice()?.text).toMatch(/programa externo pidió iniciar/);
  });

  it('con permiso explícito inicia directamente y lo avisa', () => {
    const start = vi.spyOn(useStore.getState(), 'startJob').mockResolvedValue();
    handleAutomationStart(true);
    expect(start).toHaveBeenCalledOnce();
    expect(useStore.getState().dialog).toBeNull();
    expect(lastNotice()?.text).toMatch(/iniciado por un programa externo/);
  });

  it('si la lista de comprobación es obligatoria, el permiso no la salta: se pregunta en el diálogo', () => {
    useStore.getState().setPref('jobChecklistRequired', true);
    const start = vi.spyOn(useStore.getState(), 'startJob').mockResolvedValue();
    handleAutomationStart(true);
    expect(start).not.toHaveBeenCalled();
    expect(useStore.getState().dialog).toEqual({ kind: 'start' });
  });

  it('con la máquina ocupada no inicia ni abre el diálogo, aunque esté permitido', async () => {
    await useStore.getState().disconnect();
    await vi.waitFor(() => expect(useStore.getState().connection.connected).toBe(false));
    const start = vi.spyOn(useStore.getState(), 'startJob').mockResolvedValue();
    handleAutomationStart(true);
    expect(start).not.toHaveBeenCalled();
    expect(useStore.getState().dialog).toBeNull();
    expect(lastNotice()).toMatchObject({ level: 'warning' });
    expect(lastNotice()?.text).toMatch(/no se puede/);
  });
});

describe('LOAD desde automatización', () => {
  it('importa un diseño y abre un proyecto .tlaku', async () => {
    setApi(createMockApi({ latencyMs: 0 }));
    const before = useStore.getState().doc.objects.length;
    await handleAutomationLoad('/tmp/placa.svg');
    expect(useStore.getState().doc.objects.length).toBeGreaterThan(before);
  });
});

describe('arranque', () => {
  it('suscribe los eventos y sigue los cambios de las preferencias', async () => {
    memory.clear();
    vi.stubGlobal('localStorage', { getItem: (k: string) => memory.get(k) ?? null, setItem: (k: string, v: string) => void memory.set(k, v) });
    const calls: unknown[] = [];
    const handlers = new Map<string, (p: never) => void>();
    const api = {
      automationConfigure: ({ config }: { config: unknown }) => {
        calls.push(config);
        return Promise.resolve({ running: true, port: 47801, error: null });
      },
      on: (event: string, h: (p: never) => void) => {
        handlers.set(event, h);
        return Promise.resolve(() => handlers.delete(event));
      },
    } as unknown as LaserApi;
    setApi(api);
    const stop = startAutomation();
    await vi.waitFor(() => expect(handlers.size).toBe(2));
    expect(calls).toEqual([{ enabled: false, port: 47801, allow_start: false }]);

    useStore.getState().setPref('automationEnabled', true);
    useStore.getState().setPref('automationAllowStart', true);
    await vi.waitFor(() => expect(calls).toHaveLength(3));
    expect(calls.at(-1)).toEqual({ enabled: true, port: 47801, allow_start: true });
    // Una preferencia ajena no reconfigura el servidor.
    useStore.getState().setPref('snapPx', 9);
    expect(calls).toHaveLength(3);

    stop();
    await vi.waitFor(() => expect(handlers.size).toBe(0));
    useStore.getState().setPref('automationEnabled', false);
    useStore.getState().setPref('automationAllowStart', false);
    expect(calls).toHaveLength(3);
  });
});
