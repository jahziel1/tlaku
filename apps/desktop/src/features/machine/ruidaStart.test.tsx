// @vitest-environment jsdom
import { act, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import type { DeviceProfile } from '@/api/types';
import { setLanguage } from '@/i18n';
import { FALLBACK_DEVICE } from '@/lib/device';
import { useStore } from '@/store';
import { cleanup, mount } from '@/test/dom';
import { RuidaStartNotice } from './RuidaNotice';
import { existingDoc, isValidDocName, useRuidaStartGate, type RuidaStartGate } from './ruidaStart';

const ruida = (run: boolean): DeviceProfile => ({
  ...FALLBACK_DEVICE,
  id: 'ruida-1',
  kind: 'ruida',
  connection: { type: 'udp', host: '192.168.1.100' },
  ruida: { magic: null, run_after_upload: run },
});

let api: MockApi;
let gate: RuidaStartGate;

function Harness({ device, connected = true }: { device: DeviceProfile; connected?: boolean }): ReactNode {
  gate = useRuidaStartGate(device, connected);
  return <RuidaStartNotice device={device} gate={gate} />;
}

beforeEach(async () => {
  setLanguage('es');
  useStore.getState().setPref('ruidaExperimentalWrites', true);
  api = createMockApi({ latencyMs: 0 });
  setApi(api);
  await api.connect({ port: 'virtual:ruida', baud: 115200, device: ruida(false) });
  api.machine.storedDocs.push('PIEZA', 'otra');
});

afterEach(async () => {
  await cleanup();
  api.dispose();
});

const settle = () => act(async () => new Promise<void>((r) => setTimeout(r, 20)));
const nameInput = () => document.querySelector<HTMLInputElement>('.ruida-name input[type="text"]')!;

async function type(el: HTMLInputElement, value: string) {
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(el, value);
    el.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

describe('nombre del documento al guardar en la controladora (M23)', () => {
  it('los nombres válidos son de 1 a 8 letras, cifras, guion y guion bajo', () => {
    for (const ok of ['A', 'TLAKU001', 'mi-plan1']) expect(isValidDocName(ok), ok).toBe(true);
    for (const bad of ['', 'NOMBRELARGO', 'con espacio', 'ñu', 'a.b']) expect(isValidDocName(bad), bad).toBe(false);
    expect(existingDoc([{ index: 2, name: 'Pieza' }], ' pieza ')?.index).toBe(2);
    expect(existingDoc([{ index: 1, name: null }], 'x')).toBeNull();
    expect(existingDoc([{ index: 1, name: 'A' }], '')).toBeNull();
  });

  it('el campo solo aparece si el trabajo se guarda (no si se ejecuta al subir)', async () => {
    await mount(<Harness device={ruida(false)} />);
    expect(nameInput()).not.toBeNull();
    await cleanup();
    await mount(<Harness device={ruida(true)} />);
    expect(nameInput()).toBeNull();
    expect(gate.ok).toBe(true);
  });

  it('un nombre repetido pide confirmar el reemplazo antes de poder iniciar', async () => {
    await mount(<Harness device={ruida(false)} />);
    await settle();
    expect(gate.ok).toBe(true);
    await type(nameInput(), 'pieza');
    expect(document.body.textContent).toContain('Ya hay un documento llamado «PIEZA» (número 1)');
    expect(gate.ok).toBe(false);
    const box = document.querySelector<HTMLInputElement>('.ruida-name input[type="checkbox"]')!;
    await act(async () => box.click());
    expect(gate.ok).toBe(true);
    // Cambiar el nombre quita la confirmación.
    await type(nameInput(), 'nuevo');
    expect(gate.ok).toBe(true);
    expect(document.querySelector('.ruida-name input[type="checkbox"]')).toBeNull();
  });

  it('un nombre inválido bloquea y se avisa', async () => {
    await mount(<Harness device={ruida(false)} />);
    await type(nameInput(), 'tiene espacio');
    expect(document.body.textContent).toContain('Nombre no válido');
    expect(gate.ok).toBe(false);
  });

  it('commit le manda al motor el nombre y el permiso solo cuando hay choque confirmado', async () => {
    const spy = vi.spyOn(api, 'ruidaPrepareStore');
    await mount(<Harness device={ruida(false)} />);
    await settle();
    // Nada se manda mientras no se confirma el inicio.
    expect(spy).not.toHaveBeenCalled();
    await type(nameInput(), 'pieza');
    await act(async () => document.querySelector<HTMLInputElement>('.ruida-name input[type="checkbox"]')!.click());
    await act(async () => void (await gate.commit()));
    expect(spy).toHaveBeenLastCalledWith({ name: 'pieza', overwrite: true, experimental_writes: true });
    await type(nameInput(), 'nuevo');
    await act(async () => void (await gate.commit()));
    expect(spy).toHaveBeenLastCalledWith({ name: 'nuevo', overwrite: false, experimental_writes: true });
    await type(nameInput(), '');
    await act(async () => void (await gate.commit()));
    expect(spy).toHaveBeenLastCalledWith({ name: null, overwrite: false, experimental_writes: true });
  });

  it('si el motor rechaza el nombre, commit devuelve false y deja el motivo', async () => {
    await mount(<Harness device={ruida(false)} />);
    await settle();
    await api.disconnect();
    let ok = true;
    await act(async () => {
      ok = await gate.commit();
    });
    expect(ok).toBe(false);
    expect(gate.checkError).not.toBeNull();
  });

  it('con las escrituras apagadas, reemplazar un documento existente no se puede marcar', async () => {
    useStore.getState().setPref('ruidaExperimentalWrites', false);
    await mount(<Harness device={ruida(false)} />);
    await settle();
    await type(nameInput(), 'pieza');
    const box = document.querySelector<HTMLInputElement>('.ruida-name input[type="checkbox"]')!;
    expect(box.disabled).toBe(true);
    expect(gate.ok).toBe(false);
  });
});
