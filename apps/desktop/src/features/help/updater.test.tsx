import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { en, es, setLanguage } from '@/i18n';
import tauriConf from '../../../src-tauri/tauri.conf.json';
import { UpdateView } from './HelpDialogs';
import { useHelpUi } from './helpStore';
import { isUpdaterConfigured, scheduleStartupCheck, updaterEnv, useUpdateUi, type DownloadEvent, type UpdateLike } from './updater';

// Los dos plugins se simulan: ninguna prueba toca la red ni Tauri.
const checkMock = vi.fn<() => Promise<UpdateLike | null>>();
const relaunchMock = vi.fn(async () => undefined);
vi.mock('@tauri-apps/plugin-updater', () => ({ check: () => checkMock() }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: () => relaunchMock() }));

function fakeUpdate(over: Partial<UpdateLike> = {}): UpdateLike & { install: ReturnType<typeof vi.fn> } {
  const install = vi.fn(async (onEvent?: (e: DownloadEvent) => void) => {
    onEvent?.({ event: 'Started', data: { contentLength: 200 } });
    onEvent?.({ event: 'Progress', data: { chunkLength: 50 } });
    onEvent?.({ event: 'Progress', data: { chunkLength: 150 } });
    onEvent?.({ event: 'Finished' });
  });
  return { version: '0.2.0', currentVersion: '0.1.0', date: '2026-11-01', body: 'Mejoras del lienzo.\nCorrecciones.', downloadAndInstall: install, install, ...over };
}

/** Pinta la vista con el estado actual del store (el render en servidor no ve cambios del store). */
const html = (jobBusy = false) => {
  const s = useUpdateUi.getState();
  const noop = () => undefined;
  return renderToStaticMarkup(
    <UpdateView phase={s.phase} info={s.info} progress={s.progress} error={s.error} jobBusy={jobBusy} onClose={noop} onInstall={noop} onRestart={noop} onRetry={noop} />,
  );
};

describe('isUpdaterConfigured', () => {
  it('pide llave pública y al menos una dirección', () => {
    expect(isUpdaterConfigured(null)).toBe(false);
    expect(isUpdaterConfigured({})).toBe(false);
    expect(isUpdaterConfigured({ pubkey: '', endpoints: [] })).toBe(false);
    expect(isUpdaterConfigured({ pubkey: 'abc' })).toBe(false);
    expect(isUpdaterConfigured({ pubkey: 'abc', endpoints: [''] })).toBe(false);
    expect(isUpdaterConfigured({ pubkey: '  ', endpoints: ['https://x.org/latest.json'] })).toBe(false);
    expect(isUpdaterConfigured({ pubkey: 'abc', endpoints: ['https://x.org/latest.json'] })).toBe(true);
  });

  it('el repositorio trae el actualizador y los artefactos firmados activados a la vez', () => {
    // scripts/configure-updater.sh activa las dos cosas juntas (o las apaga con --desactivar).
    expect(isUpdaterConfigured(tauriConf.plugins.updater)).toBe(tauriConf.bundle.createUpdaterArtifacts);
    expect(tauriConf.plugins.updater.endpoints).toEqual(['https://github.com/jahziel1/tlaku/releases/latest/download/latest.json']);
  });
});

describe('flujo de actualización', () => {
  let available = false;
  beforeEach(() => {
    setLanguage('es');
    available = false;
    updaterEnv.available = () => available;
    checkMock.mockReset();
    relaunchMock.mockClear();
    useUpdateUi.getState().reset();
    useHelpUi.getState().close();
  });
  afterEach(() => setLanguage('es'));

  it('sin llave configurada avisa y no llama al plugin', async () => {
    await useUpdateUi.getState().check(false);
    expect(checkMock).not.toHaveBeenCalled();
    expect(useUpdateUi.getState().phase).toBe('unconfigured');
    expect(html()).toContain('Las actualizaciones automáticas no están configuradas en esta compilación');
    setLanguage('en');
    expect(html()).toContain(en['help.update.unconfigured']);
    expect(en['help.update.unconfigured']).not.toBe(es['help.update.unconfigured']);
  });

  it('el chequeo de arranque, sin llave, no dice nada ni abre nada', async () => {
    await useUpdateUi.getState().check(true);
    expect(useUpdateUi.getState().phase).toBe('idle');
    expect(useHelpUi.getState().dialog).toBeNull();
  });

  it('sin versión nueva dice que está al día', async () => {
    available = true;
    checkMock.mockResolvedValue(null);
    await useUpdateUi.getState().check(false);
    expect(useUpdateUi.getState().phase).toBe('current');
    expect(html()).toContain(es['help.update.current']);
  });

  it('con versión nueva muestra versión, notas y los dos botones', async () => {
    available = true;
    checkMock.mockResolvedValue(fakeUpdate());
    await useUpdateUi.getState().check(false);
    expect(useUpdateUi.getState().info).toEqual({ version: '0.2.0', current: '0.1.0', notes: 'Mejoras del lienzo.\nCorrecciones.', date: '2026-11-01' });
    const out = html();
    expect(out).toContain('Tlaku 0.2.0');
    expect(out).toContain('Mejoras del lienzo.');
    expect(out).toContain('Descargar e instalar');
    expect(out).toContain('Ahora no');
    setLanguage('en');
    const inEn = html();
    expect(inEn).toContain('Download and install');
    expect(inEn).toContain('Not now');
  });

  it('con un trabajo en curso no deja instalar', async () => {
    available = true;
    checkMock.mockResolvedValue(fakeUpdate());
    await useUpdateUi.getState().check(false);
    expect(html(true)).toContain(es['help.update.busy']);
    expect(html(true)).toMatch(/<button[^>]*disabled=""[^>]*>Descargar e instalar/);
    expect(html(false)).not.toContain(es['help.update.busy']);
  });

  it('una versión sin notas lo dice', async () => {
    available = true;
    checkMock.mockResolvedValue(fakeUpdate({ body: undefined }));
    await useUpdateUi.getState().check(false);
    expect(html()).toContain(es['help.update.noNotes']);
  });

  it('el chequeo de arranque abre el diálogo solo si hay versión nueva', async () => {
    available = true;
    checkMock.mockResolvedValue(null);
    await useUpdateUi.getState().check(true);
    expect(useHelpUi.getState().dialog).toBeNull();
    checkMock.mockResolvedValue(fakeUpdate());
    await useUpdateUi.getState().check(true);
    expect(useHelpUi.getState().dialog).toBe('update');
  });

  it('descarga con progreso, instala y reinicia solo cuando se pide', async () => {
    available = true;
    const update = fakeUpdate();
    checkMock.mockResolvedValue(update);
    await useUpdateUi.getState().check(false);
    const seen: number[] = [];
    const stop = useUpdateUi.subscribe((s) => seen.push(s.progress.done));
    await useUpdateUi.getState().install();
    stop();
    expect(update.install).toHaveBeenCalledTimes(1);
    expect(seen).toContain(50);
    expect(useUpdateUi.getState().progress).toEqual({ done: 200, total: 200 });
    expect(useUpdateUi.getState().phase).toBe('installed');
    expect(relaunchMock).not.toHaveBeenCalled();
    expect(html()).toContain(es['help.update.restart']);
    await useUpdateUi.getState().restart();
    expect(relaunchMock).toHaveBeenCalledTimes(1);
  });

  it('un fallo al buscar o al instalar se muestra y permite reintentar', async () => {
    available = true;
    checkMock.mockRejectedValueOnce(new Error('sin red'));
    await useUpdateUi.getState().check(false);
    expect(useUpdateUi.getState().phase).toBe('error');
    expect(html()).toContain('sin red');
    expect(html()).toContain(es['help.update.retry']);

    const update = fakeUpdate();
    update.install.mockRejectedValueOnce(new Error('firma inválida'));
    checkMock.mockResolvedValue(update);
    await useUpdateUi.getState().check(false);
    await useUpdateUi.getState().install();
    expect(useUpdateUi.getState()).toMatchObject({ phase: 'error', error: 'firma inválida' });
  });

  it('no instala si no hay una actualización encontrada', async () => {
    available = true;
    await useUpdateUi.getState().install();
    expect(useUpdateUi.getState().phase).toBe('idle');
  });

  it('el comando del menú abre el diálogo y busca', async () => {
    const { commands } = await import('@/commands');
    await commands.run('help.checkUpdates');
    expect(useHelpUi.getState().dialog).toBe('update');
    await vi.waitFor(() => expect(useUpdateUi.getState().phase).toBe('unconfigured'));
  });
});

describe('chequeo al iniciar', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    checkMock.mockReset();
    checkMock.mockResolvedValue(null);
    useUpdateUi.getState().reset();
  });
  afterEach(() => vi.useRealTimers());

  it('con la preferencia apagada (de fábrica) no busca', async () => {
    updaterEnv.available = () => true;
    scheduleStartupCheck(() => false, async () => true, 10);
    await vi.advanceTimersByTimeAsync(50);
    expect(checkMock).not.toHaveBeenCalled();
  });

  it('con la preferencia encendida busca en la ventana principal, y no en las de documento', async () => {
    updaterEnv.available = () => true;
    scheduleStartupCheck(() => true, async () => false, 10);
    await vi.advanceTimersByTimeAsync(50);
    expect(checkMock).not.toHaveBeenCalled();
    scheduleStartupCheck(() => true, async () => true, 10);
    await vi.advanceTimersByTimeAsync(50);
    expect(checkMock).toHaveBeenCalledTimes(1);
  });

  it('sin actualizador configurado ni siquiera programa nada', async () => {
    updaterEnv.available = () => false;
    scheduleStartupCheck(() => true, async () => true, 10);
    await vi.advanceTimersByTimeAsync(50);
    expect(checkMock).not.toHaveBeenCalled();
  });
});
