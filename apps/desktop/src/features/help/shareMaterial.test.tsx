// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { MaterialPreset } from '@/api/types';
import { setLanguage } from '@/i18n';
import { makeLayer } from '@/lib/layers';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { REPO_URL } from './links';
import { openEnv } from './openExternal';
import { ShareMaterialDialog } from './ShareDialogs';
import { useHelpUi } from './helpStore';

const preset: MaterialPreset = {
  id: 'mat-1', material: 'Contrachapado de tilo', thickness: 3, title: 'Corte', notes: 'privado', device_id: null,
  settings: makeLayer('#000000', 'Corte', { speed: 500, power: 100, passes: 2, air: true }),
  role: 'cut', machine_class: 'diode_20',
};

let open: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  setApi(createMockApi({ latencyMs: 0 }));
  setLanguage('es');
  useHelpUi.setState({ dialog: 'shareMaterial', sharePreset: preset });
  open = vi.spyOn(openEnv, 'open').mockResolvedValue(undefined);
});
afterEach(async () => {
  open.mockRestore();
  await cleanup();
});

describe('Compartir este ajuste', () => {
  it('muestra lo que se prellena y el aporte como JSON, y no abre ni envía nada al mostrarse', async () => {
    await mount(<ShareMaterialDialog />);
    await vi.waitFor(() => expect(document.body.textContent).toContain('Versión de Tlaku'));
    const text = document.body.textContent ?? '';
    expect(text).toContain('Ver el aporte como JSON');
    expect(text).toContain('Tlaku no envía nada por su cuenta');
    const json = document.querySelector('details code')!.textContent!;
    const draft = JSON.parse(json) as Record<string, unknown>;
    expect(draft).toMatchObject({ material: 'Contrachapado de tilo', thickness: 3, role: 'cut', speed: 500, power: 100, passes: 2, air: true });
    expect(json).not.toContain('privado');
    expect(json).not.toContain('mat-1');
    expect(open).not.toHaveBeenCalled();
  });

  it('solo al pulsar «Abrir el formulario» abre la dirección del formulario del repositorio, prellenada', async () => {
    await mount(<ShareMaterialDialog />);
    await vi.waitFor(() => expect(document.body.textContent).toContain('Versión de Tlaku'));
    const openBtn = [...document.querySelectorAll('button')].find((b) => b.textContent?.includes('Abrir el formulario en GitHub'))!;
    await click(openBtn);
    await vi.waitFor(() => expect(open).toHaveBeenCalledOnce());
    const url = new URL(open.mock.calls[0]![0] as string);
    expect(`${url.origin}${url.pathname}`).toBe(`${REPO_URL}/issues/new`);
    expect(url.searchParams.get('template')).toBe('material_settings.yml');
    expect(url.searchParams.get('material')).toBe('Contrachapado de tilo');
    expect(url.searchParams.get('speed')).toBe('500');
    expect(url.searchParams.get('tlaku_version')).toBeTruthy();
    expect(url.searchParams.has('notes')).toBe(false);
  });

  it('axe no encuentra problemas', async () => {
    await mount(<ShareMaterialDialog />);
    expect(describeViolations(await audit())).toEqual([]);
  });
});
