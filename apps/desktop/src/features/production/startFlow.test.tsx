// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { StartJobDialog } from '@/features/machine/StartJobDialog';
import { setLanguage } from '@/i18n';
import { bindMachineEvents, useStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { cleanup, click, mount, type Mounted } from '@/test/dom';
import { resetProductionSession } from './session';

const s = () => useStore.getState();
const FULL_TITLE = '¿Láser listo y área despejada?';
const COMPACT_TITLE = 'Iniciar (modo producción)';

let api: MockApi;
let unbind: () => void;
let mounted: Mounted | null = null;

async function boot(minJobMs: number): Promise<void> {
  api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs, jobSpeedup: 1e6 });
  setApi(api);
  unbind = await bindMachineEvents(api);
  await s().loadDevices();
  await s().refreshPorts();
  s().setSelectedPort('virtual:falcon2');
  await s().connect();
  await vi.waitFor(() => expect(s().status !== null).toBe(true));
  s().replaceDocument({ ...emptyDocument(), objects: sampleObjects(), layers: sampleLayers() }, null);
  useStore.setState({ jobState: 'idle', jobOrigin: null, jobMessage: '', jobError: null });
  s().closeDialog();
  resetProductionSession();
  s().setPref('productionMode', true);
}

const titles = () => [...document.querySelectorAll('h2')].map((h) => h.textContent);
const button = (text: string) => [...document.querySelectorAll('button')].find((b) => b.textContent?.includes(text));

/** Pide el inicio como lo hace Iniciar y deja montado el diálogo; devuelve cuál de los dos salió. */
async function open(production = true): Promise<'full' | 'compact'> {
  s().requestStart(production ? { production: true } : undefined);
  await vi.waitFor(() => expect(s().previewStatus).toBe('ready'));
  mounted = await mount(<StartJobDialog />);
  // El paso compacto empieza en «Comprobando…»: se espera a que termine de decidir.
  await vi.waitFor(
    () => expect(titles().includes(FULL_TITLE) || (titles().includes(COMPACT_TITLE) && !document.body.textContent?.includes('Comprobando'))).toBe(true),
    { timeout: 3000 },
  );
  return titles().includes(COMPACT_TITLE) ? 'compact' : 'full';
}

async function close(): Promise<void> {
  await mounted?.unmount();
  mounted = null;
  s().closeDialog();
}

/** Un trabajo confirmado con la lista completa que termina bien: la referencia del modo producción. */
async function goodFullJob(): Promise<void> {
  expect(await open()).toBe('full');
  await click(button('Iniciar trabajo')!);
  await vi.waitFor(() => expect(s().jobState).toBe('finished'));
  await close();
}

beforeEach(async () => {
  setLanguage('es');
  await boot(150);
});
afterEach(async () => {
  await cleanup();
  mounted = null;
  unbind();
  s().resetPrefs();
  resetProductionSession();
  vi.restoreAllMocks();
});

describe('modo producción en el diálogo de inicio', () => {
  it('el primer inicio de la sesión siempre muestra la lista completa', async () => {
    expect(await open()).toBe('full');
    expect(document.querySelector('.mc-checklist')).not.toBeNull();
  });

  it('tras un trabajo confirmado con la lista completa y terminado bien, el siguiente es el paso compacto', async () => {
    await goodFullJob();
    expect(await open()).toBe('compact');
    expect(document.querySelector('.mc-checklist')).toBeNull(); // la lista de comprobación no está
    // Muestra el tiempo estimado y deja un enlace a la lista completa.
    expect(document.querySelector('[data-testid="prod-time"]')?.textContent).toMatch(/\d/);
    expect(button('Ver la lista completa')).toBeDefined();
    // «Iniciar» lleva el foco, así que Enter lo confirma.
    await vi.waitFor(() => expect((document.activeElement as HTMLElement | null)?.textContent).toContain('Iniciar'));
    const startJob = vi.spyOn(api, 'startJob');
    await click(button('Iniciar (Enter)')!);
    await vi.waitFor(() => expect(startJob).toHaveBeenCalledTimes(1));
  });

  it('«Ver la lista completa» abre el diálogo de siempre y ya no vuelve al compacto', async () => {
    await goodFullJob();
    expect(await open()).toBe('compact');
    await click(button('Ver la lista completa')!);
    expect(titles()).toContain(FULL_TITLE);
    expect(titles()).not.toContain(COMPACT_TITLE);
  });

  it('apagado de fábrica: sin la preferencia nunca se omite la lista', async () => {
    await goodFullJob();
    s().setPref('productionMode', false);
    expect(await open()).toBe('full');
  });

  it('pedido sin `production` (cola guiada, automatización, modo sencillo) siempre es el completo', async () => {
    await goodFullJob();
    expect(await open(false)).toBe('full');
  });

  it('un cambio en los ajustes de una capa con salida obliga a mostrar el completo', async () => {
    await goodFullJob();
    const layer = s().doc.layers.find((l) => l.output)!;
    s().updateLayer(layer.color, { speed: layer.speed + 10 });
    expect(await open()).toBe('full');
  });

  it('cambiar el origen del trabajo obliga a mostrar el completo', async () => {
    await goodFullJob();
    s().setJobOptions({ job_origin: (s().jobOptions.job_origin + 1) % 9 });
    expect(await open()).toBe('full');
  });

  it('pasar a «Solo la selección» cambia lo que se corta y obliga a mostrar el completo', async () => {
    await goodFullJob();
    s().select([s().doc.objects[0]!.id]);
    s().setCutSelectionOnly(true);
    expect(await open()).toBe('full');
  });

  it('reconectar la máquina obliga a mostrar el completo', async () => {
    await goodFullJob();
    await s().disconnect();
    await s().connect();
    await vi.waitFor(() => expect(s().status !== null && s().connection.connected).toBe(true));
    expect(await open()).toBe('full');
  });

  it('un trabajo cancelado no vale como referencia', async () => {
    await boot(600);
    expect(await open()).toBe('full');
    await click(button('Iniciar trabajo')!);
    await vi.waitFor(() => expect(s().jobState).toBe('running'));
    await s().cancelJob();
    await vi.waitFor(() => expect(s().jobState).toBe('cancelled'));
    await close();
    await vi.waitFor(() => expect(s().status?.state).toBe('idle'));
    expect(await open()).toBe('full');
  });

  it('si el motor rechaza el inicio, ese intento no queda como referencia', async () => {
    expect(await open()).toBe('full');
    vi.spyOn(api, 'startJob').mockRejectedValueOnce(new Error('rechazado'));
    await click(button('Iniciar trabajo')!);
    await vi.waitFor(() => expect(s().jobError).not.toBeNull());
    await close();
    // Un trabajo posterior ajeno que termine bien no debe «validar» el intento rechazado.
    useStore.setState({ jobState: 'running' });
    useStore.setState({ jobState: 'finished' });
    expect(await open()).toBe('full');
  });

  it('con un aviso de la vista previa se muestra el completo aunque todo lo demás sea igual', async () => {
    await goodFullJob();
    const real = api.computeJob.bind(api);
    vi.spyOn(api, 'computeJob').mockImplementation(async (a) => ({ ...(await real(a)), warnings: ['Algo raro'] }));
    s().invalidatePreview();
    await s().computePreview();
    expect(await open()).toBe('full');
  });
});
