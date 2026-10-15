// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { genericGrblProbe } from '@/api/mock/probe';
import { cleanup, click, mount } from '@/test/dom';
import { setLanguage } from '@/i18n';
import { useStore } from '@/store';
import { AddMachineWizard } from './AddMachineWizard';
import { useDevicesWizard } from './wizardStore';

const button = (label: string): HTMLButtonElement => {
  const b = [...document.querySelectorAll('button')].find((x) => x.textContent?.trim() === label);
  if (!b) throw new Error(`No hay botón «${label}»`);
  return b as HTMLButtonElement;
};

async function settle() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 20));
  });
}

beforeEach(async () => {
  setLanguage('es');
  setApi(createMockApi({ latencyMs: 0 }));
  useDevicesWizard.setState({ open: false });
  await useStore.getState().refreshPorts();
});
afterEach(cleanup);

describe('asistente: preselección del modelo por la firma del firmware', () => {
  it('una Falcon2 Pro detectada llega al paso Modelo ya seleccionada y con su evidencia', async () => {
    await mount(<AddMachineWizard />);
    await settle();
    await click(button('Detectar'));
    await settle();
    expect(document.body.textContent).toContain('Detectamos: Creality Falcon2 Pro (22 / 40 W)');
    expect(document.body.textContent).toContain('CV50-Pro-MASTER-Release V1.0.1');

    await click(button('Siguiente'));
    const checked = [...document.querySelectorAll<HTMLInputElement>('input[type=radio]')].filter((r) => r.checked);
    expect(checked).toHaveLength(1);
    expect(checked[0]!.closest('label')?.textContent).toContain('Falcon2 Pro');
    expect(document.body.textContent).toContain('Detectamos: Creality Falcon2 Pro (22 / 40 W)');

    // Revisar: el volcado antiguo da 50 000 mm/min y 5000 mm/s²; por defecto se propone lo de la máquina.
    await click(button('Siguiente'));
    const boxes = [...document.querySelectorAll<HTMLInputElement>('input[type=checkbox]')];
    const byLabel = (needle: string) => boxes.find((b) => (b.getAttribute('aria-label') ?? '').includes(needle));
    expect(byLabel('Velocidad máxima')?.checked).toBe(true);
    expect(byLabel('Aceleración')?.checked).toBe(true);
  });

  it('el usuario puede elegir otro modelo después de la preselección', async () => {
    await mount(<AddMachineWizard />);
    await settle();
    await click(button('Detectar'));
    await settle();
    await click(button('Siguiente'));
    const other = [...document.querySelectorAll<HTMLInputElement>('input[type=radio]')].find((r) => r.closest('label')?.textContent?.includes('Falcon2 22 W'));
    expect(other).toBeTruthy();
    await click(other!);
    const now = [...document.querySelectorAll<HTMLInputElement>('input[type=radio]')].filter((r) => r.checked);
    expect(now.map((r) => r.closest('label')?.textContent ?? '').join('|')).toContain('Falcon2 22 W');
  });

  it('un GRBL cualquiera no preselecciona nada ni muestra «Detectamos»', async () => {
    const api = createMockApi({ latencyMs: 0 });
    setApi({ ...api, probePort: async () => genericGrblProbe() });
    await mount(<AddMachineWizard />);
    await settle();
    await click(button('Detectar'));
    await settle();
    expect(document.body.textContent).not.toContain('Detectamos:');
    await click(button('Siguiente'));
    expect([...document.querySelectorAll<HTMLInputElement>('input[type=radio]')].some((r) => r.checked)).toBe(false);
  });
});

describe('asistente: la máquina agregada queda como la de siempre', () => {
  it('Guardar y activar la deja activa y recordada para la próxima vez que se abra la app', async () => {
    const memory = new Map<string, string>();
    vi.stubGlobal('localStorage', { getItem: (k: string) => memory.get(k) ?? null, setItem: (k: string, v: string) => void memory.set(k, v) });
    try {
      await useStore.getState().loadDevices();
      await mount(<AddMachineWizard />);
      await settle();
      await click(button('Detectar'));
      await settle();
      await click(button('Siguiente'));
      await click(button('Siguiente'));
      await click(button('Siguiente'));
      await click(button('Guardar y activar'));
      await settle();
      const active = useStore.getState().activeDeviceId;
      expect(active).toBeTruthy();
      expect(useStore.getState().devices.find((d) => d.id === active)?.name).toContain('Falcon2 Pro');
      expect(memory.get('tlaku.lastDevice')).toBe(active);
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
