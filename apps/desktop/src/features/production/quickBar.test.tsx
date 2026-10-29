// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { commands } from '@/commands';
import { machineCommands } from '@/commands/definitions/machine';
import { emptyLibrary, presetFromLayer } from '@/features/materials/library';
import { useMaterials } from '@/features/materials/state';
import { usePrompts } from '@/features/history/prompt';
import { SettingsDialog } from '@/features/settings/SettingsDialog';
import { useSettingsUi } from '@/features/settings/uiStore';
import { setLanguage } from '@/i18n';
import { makeLayer } from '@/lib/layers';
import { bindMachineEvents, useStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { cleanup, click, mount } from '@/test/dom';
import { QuickBar } from './QuickBar';

const s = () => useStore.getState();
const bar = () => document.querySelector('[data-testid="quick-bar"]');
const btn = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('.qbar button')].find((b) => (b.getAttribute('aria-label') ?? b.textContent ?? '').includes(label));

let unbind: () => void;

beforeEach(async () => {
  s().resetPrefs();
  setLanguage('es');
  const api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 100, jobSpeedup: 1e6 });
  setApi(api);
  unbind = await bindMachineEvents(api);
  await s().loadDevices();
  await s().refreshPorts();
  s().setSelectedPort('virtual:falcon2');
  await s().connect();
  await vi.waitFor(() => expect(s().status !== null).toBe(true));
  s().replaceDocument({ ...emptyDocument(), objects: sampleObjects(), layers: sampleLayers() }, null);
  useStore.setState({ jobState: 'idle', jobOrigin: null, jobMessage: '', jobError: null, cutSelectionOnly: false });
  s().select([]);
  s().setTool('select');
  s().closeDialog();
  useMaterials.setState({ library: emptyLibrary(), factory: emptyLibrary(), loaded: true, loadError: null });
});
afterEach(async () => {
  await cleanup();
  unbind();
  s().resetPrefs();
  vi.restoreAllMocks();
});

describe('barra rápida de la selección', () => {
  it('aparece solo con algo seleccionado y con la herramienta Seleccionar', async () => {
    await mount(<QuickBar />);
    expect(bar()).toBeNull();
    s().select(['obj-estrella']);
    await vi.waitFor(() => expect(bar()).not.toBeNull());
    s().setTool('line');
    await vi.waitFor(() => expect(bar()).toBeNull());
  });

  it('se oculta en Preferencias (quickBar apagada)', async () => {
    s().select(['obj-estrella']);
    s().setPref('quickBar', false);
    await mount(<QuickBar />);
    expect(bar()).toBeNull();
  });

  it('gira y voltea con los comandos que ya existen', async () => {
    s().select(['obj-estrella']);
    await mount(<QuickBar />);
    const before = s().doc.objects.find((o) => o.id === 'obj-estrella')!.transform;
    await click(btn('Girar 90° a la izquierda')!);
    await vi.waitFor(() => expect(s().doc.objects.find((o) => o.id === 'obj-estrella')!.transform).not.toEqual(before));
    const rotated = s().doc.objects.find((o) => o.id === 'obj-estrella')!.transform;
    await click(btn('Voltear horizontal')!);
    await vi.waitFor(() => expect(s().doc.objects.find((o) => o.id === 'obj-estrella')!.transform).not.toEqual(rotated));
    expect(btn('Girar 90° a la derecha')).toBeDefined();
    expect(btn('Voltear vertical')).toBeDefined();
  });

  it('un favorito mueve la selección a una capa con sus ajustes', async () => {
    const preset = presetFromLayer(makeLayer('#000000', 'x', { speed: 777, power: 66 }), { material: 'mdf', thickness: 3, title: 'Mi corte', notes: '', device_id: null }, 'mine-1');
    useMaterials.setState({ library: { ...emptyLibrary(), presets: [preset] } });
    s().setPref('favoriteMaterials', ['mine-1']);
    s().select(['obj-estrella']);
    await mount(<QuickBar />);
    const chip = btn('Mi corte') ?? [...document.querySelectorAll<HTMLButtonElement>('.qbar__fav')][0]!;
    await click(chip);
    const obj = s().doc.objects.find((o) => o.id === 'obj-estrella')!;
    expect(s().doc.layers.find((l) => l.color === obj.layer)).toMatchObject({ speed: 777, power: 66 });
  });

  it('sin favoritos lo dice y lleva a la pestaña Materiales', async () => {
    s().select(['obj-estrella']);
    await mount(<QuickBar />);
    expect(document.querySelectorAll('.qbar__fav')).toHaveLength(0);
    await click(btn('Abrir la pestaña Materiales')!);
    expect(s().dockTab).toBe('materials');
  });

  it('«Iniciar la selección» abre el diálogo de inicio normal con «Solo la selección» ya marcada y no envía nada', async () => {
    s().select(['obj-estrella']);
    await mount(<QuickBar />);
    expect(s().cutSelectionOnly).toBe(false);
    await click(btn('Iniciar la selección')!);
    expect(s().cutSelectionOnly).toBe(true);
    expect(s().dialog).toEqual({ kind: 'start', production: true });
    expect(s().jobState).toBe('idle');
  });

  it('el comando «Iniciar la selección» no está disponible sin selección', async () => {
    expect(commands.availability('machine.startSelection', s())).toBe('machine.selection.reason.none');
    s().select(['obj-estrella']);
    expect(commands.availability('machine.startSelection', s())).toBe(true);
    expect(machineCommands.find((c) => c.id === 'machine.startSelection')).toBeDefined();
  });

  it('Iniciar normal marca el diálogo como abierto por una persona; el pedido sin opciones (cola, automatización) no', () => {
    s().requestStart({ production: true });
    expect(s().dialog).toEqual({ kind: 'start', production: true });
    s().closeDialog();
    s().requestStart();
    expect(s().dialog).toEqual({ kind: 'start' });
  });
});

describe('preferencias de producción', () => {
  beforeEach(() => useSettingsUi.getState().openSettings('general'));
  afterEach(() => useSettingsUi.getState().closeSettings());

  const check = () => [...document.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')].find((i) => i.closest('label')?.textContent?.includes('Modo producción'))!;

  it('el modo producción está apagado de fábrica y la barra rápida encendida', () => {
    expect(s().prefs.productionMode).toBe(false);
    expect(s().prefs.quickBar).toBe(true);
  });

  it('encenderlo pide confirmar con un texto que explica el riesgo; cancelar lo deja apagado', async () => {
    await mount(<SettingsDialog />);
    await click(check());
    await vi.waitFor(() => expect(usePrompts.getState().current).not.toBeNull());
    expect(usePrompts.getState().current!.message).toMatch(/responsable|riesgo|vigilancia/i);
    expect(s().prefs.productionMode).toBe(false); // todavía no
    usePrompts.getState().answer('no');
    await vi.waitFor(() => expect(usePrompts.getState().current).toBeNull());
    expect(s().prefs.productionMode).toBe(false);
  });

  it('confirmado, se enciende; apagarlo es inmediato', async () => {
    await mount(<SettingsDialog />);
    await click(check());
    await vi.waitFor(() => expect(usePrompts.getState().current).not.toBeNull());
    usePrompts.getState().answer('yes');
    await vi.waitFor(() => expect(s().prefs.productionMode).toBe(true));
    await click(check());
    await vi.waitFor(() => expect(s().prefs.productionMode).toBe(false));
    expect(usePrompts.getState().current).toBeNull();
  });
});
