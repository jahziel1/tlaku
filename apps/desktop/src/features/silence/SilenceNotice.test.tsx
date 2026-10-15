// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from 'react';
import { createMockApi, type MockApi } from '@/api/mock';
import { setApi } from '@/api';
import { setLanguage } from '@/i18n';
import { bindMachineEvents, useStore } from '@/store';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { SilenceNotice } from './SilenceNotice';

const emit = (api: MockApi, name: string, payload: unknown) => (api.machine as unknown as { emit: (e: string, p: unknown) => void }).emit(name, payload);

async function setup() {
  const api = createMockApi({ latencyMs: 0 });
  setApi(api);
  const off = await bindMachineEvents(api);
  // El store global se limpia con una desconexión (deja la máquina sin estado ni aviso).
  useStore.getState().onConnection({ connected: false, port: null, error: null });
  return { api, off };
}

const connectEv = { connected: true, port: '/dev/x', error: null };
const text = () => document.body.textContent ?? '';
const button = (label: string) => [...document.querySelectorAll('button')].find((b) => b.textContent?.includes(label))!;

beforeEach(() => setLanguage('es'));
afterEach(async () => {
  await cleanup();
  setLanguage('es');
});

describe('aviso «la máquina no contesta»', () => {
  it('aparece con los segundos y se quita solo cuando la placa vuelve a contestar', async () => {
    const { api, off } = await setup();
    await mount(<SilenceNotice />);
    expect(text()).toBe('');
    await act(async () => {
      emit(api, 'machine://connection', connectEv);
      emit(api, 'machine://silence', { silence_ms: 2300 });
    });
    expect(text()).toContain('La máquina no contesta desde hace 2 s…');
    // Sigue contando cada segundo.
    await act(async () => emit(api, 'machine://silence', { silence_ms: 3400 }));
    expect(text()).toContain('desde hace 3 s…');
    // Vuelve la respuesta: el aviso se va solo.
    await act(async () => emit(api, 'machine://silence', { silence_ms: 0 }));
    expect(text()).toBe('');
    off();
  });

  it('durante un trabajo dice que continuará solo; en inglés también', async () => {
    const { api, off } = await setup();
    await mount(<SilenceNotice />);
    await act(async () => {
      emit(api, 'machine://connection', connectEv);
      emit(api, 'machine://job', { state: 'started', message: '', position_lost: false });
      emit(api, 'machine://silence', { silence_ms: 2100 });
    });
    expect(text()).toContain('continuará solo');
    setLanguage('en');
    await act(async () => emit(api, 'machine://silence', { silence_ms: 2200 }));
    expect(text()).toContain('The machine has not answered for 2 s…');
    expect(text()).toContain('carry on by itself');
    off();
  });

  it('pasado un minuto en pleno trabajo cambia al aviso serio y no desconecta por su cuenta', async () => {
    const { api, off } = await setup();
    const disconnect = vi.spyOn(api, 'disconnect');
    await mount(<SilenceNotice />);
    await act(async () => {
      emit(api, 'machine://connection', connectEv);
      emit(api, 'machine://job', { state: 'started', message: '', position_lost: false });
      emit(api, 'machine://silence', { silence_ms: 61_000 });
    });
    expect(text()).toContain('Lleva más de un minuto sin contestar');
    expect(text()).toContain('paro de emergencia');
    expect(document.querySelector('[role="alert"]')).not.toBeNull();
    expect(disconnect).not.toHaveBeenCalled();
    expect(useStore.getState().connection.connected).toBe(true);
    off();
  });

  it('Detener y Desconectar son del usuario: llaman a lo de siempre', async () => {
    const { api, off } = await setup();
    const cancel = vi.spyOn(api, 'cancelJob').mockResolvedValue(undefined);
    const disconnect = vi.spyOn(api, 'disconnect').mockResolvedValue(undefined);
    await mount(<SilenceNotice />);
    await act(async () => {
      emit(api, 'machine://connection', connectEv);
      emit(api, 'machine://silence', { silence_ms: 3000 });
    });
    expect(cancel).not.toHaveBeenCalled();
    await click(button('Detener'));
    expect(cancel).toHaveBeenCalledTimes(1);
    await click(button('Desconectar'));
    expect(disconnect).toHaveBeenCalledTimes(1);
    off();
  });

  it('una caída de la conexión borra el aviso', async () => {
    const { api, off } = await setup();
    await mount(<SilenceNotice />);
    await act(async () => {
      emit(api, 'machine://connection', connectEv);
      emit(api, 'machine://silence', { silence_ms: 5000 });
    });
    expect(text()).toContain('desde hace 5 s');
    await act(async () => emit(api, 'machine://connection', { connected: false, port: '/dev/x', error: 'x' }));
    expect(text()).toBe('');
    off();
  });

  it('pasa axe, también en la versión seria', async () => {
    const { api, off } = await setup();
    const m = await mount(<SilenceNotice />);
    await act(async () => {
      emit(api, 'machine://connection', connectEv);
      emit(api, 'machine://silence', { silence_ms: 2500 });
    });
    expect(describeViolations(await audit(m.host))).toEqual([]);
    await act(async () => {
      emit(api, 'machine://job', { state: 'started', message: '', position_lost: false });
      emit(api, 'machine://silence', { silence_ms: 90_000 });
    });
    expect(describeViolations(await audit(m.host))).toEqual([]);
    off();
  });
});
