// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setApi, type UserBundle } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleDevices, sampleDocument } from '@/api/mock/sample';
import { commands } from '@/commands';
import { buildUserBundle } from '@/features/bundle/model';
import { useBundleUi } from '@/features/bundle/uiStore';
import { emptyLibrary } from '@/features/materials/library';
import { useMaterials } from '@/features/materials/state';
import { emptyRemnants } from '@/features/nest/remnants';
import { useRemnants } from '@/features/nest/remnantStore';
import { defaultStored } from '@/features/costs/costProfiles';
import { setLanguage } from '@/i18n';
import { DEFAULT_PREFS, useStore } from '@/store';
import { act } from 'react';
import { click, cleanup, mount } from '@/test/dom';
import {
  backupOnClose,
  backupSections,
  CLOSE_BACKUP_TIMEOUT_MS,
  isBackupDue,
  loadBackupConfig,
  markBackupSeen,
  resetBackupWarning,
  runBackup,
  runBackupStartup,
  saveBackupSettings,
  shouldBackupOnClose,
} from './actions';
import { BackupDialog } from './BackupDialog';
import { NewerBackupPrompt } from './NewerBackupPrompt';
import { resetBackupStore, useBackup } from './store';

let api: MockApi;

function foreignBundle(over: Partial<Parameters<typeof buildUserBundle>[0]> = {}): UserBundle {
  const dev = { ...sampleDevices()[0]!, id: 'de-la-otra', name: 'Láser de la otra compu' };
  return buildUserBundle(
    { devices: [dev], activeDeviceId: dev.id, library: emptyLibrary(), prefs: { ...DEFAULT_PREFS }, shortcuts: {}, costs: defaultStored('es', 'es-MX'), remnants: emptyRemnants(), templates: [], ...over },
    { app: '0.9.0' },
  );
}

beforeEach(async () => {
  setLanguage('es');
  api = createMockApi({ latencyMs: 0 });
  setApi(api);
  localStorage.clear();
  resetBackupStore();
  resetBackupWarning();
  useBundleUi.setState({ mode: null, source: null });
  useMaterials.setState({ library: emptyLibrary(), loaded: false, loadError: null });
  useRemnants.setState({ library: emptyRemnants(), loaded: false, loadError: null });
  useStore.setState({ jobState: 'idle', notices: [] });
  await useStore.getState().loadDevices();
});

afterEach(cleanup);

/** Deja correr las promesas y temporizadores del mock (latencia 0) y los efectos de React. */
const settle = () => act(async () => { await new Promise((r) => setTimeout(r, 20)); });
/** Espera hasta que se cumpla algo, dejando que React pinte entre intentos (dentro de un solo `act` no se pinta). */
async function until(cond: () => boolean | Promise<boolean>): Promise<void> {
  for (let i = 0; i < 300 && !(await cond()); i++) await act(async () => { await new Promise((r) => setTimeout(r, 5)); });
}
const names = () => api.backupSim.names();
const lastNotice = () => useStore.getState().notices.at(-1);

describe('cuándo toca respaldar', () => {
  const DAY = 24 * 60 * 60 * 1000;
  it('cada N días desde el último; nunca sin carpeta o con 0 días', () => {
    const now = 100 * DAY;
    expect(isBackupDue({ folder: '/x', every_days: 7, last_backup_ms: now - 8 * DAY }, now)).toBe(true);
    expect(isBackupDue({ folder: '/x', every_days: 7, last_backup_ms: now - 6 * DAY }, now)).toBe(false);
    expect(isBackupDue({ folder: '/x', every_days: 7, last_backup_ms: now - 7 * DAY }, now)).toBe(true);
    expect(isBackupDue({ folder: '/x', every_days: 7, last_backup_ms: 0 }, now)).toBe(true);
    expect(isBackupDue({ folder: '/x', every_days: 0, last_backup_ms: 0 }, now)).toBe(false);
    expect(isBackupDue({ folder: null, every_days: 7, last_backup_ms: 0 }, now)).toBe(false);
  });

  it('el historial solo entra si la persona lo pidió', () => {
    expect(backupSections({ include_history: false }).history).toBe(false);
    expect(backupSections({ include_history: true }).history).toBe(true);
    expect(backupSections({ include_history: false }).costs).toBe(true);
  });
});

describe('respaldar', () => {
  it('sin carpeta, el manual avisa y los automáticos callan', async () => {
    expect(await runBackup('manual')).toBe(false);
    expect(lastNotice()?.level).toBe('error');
    useStore.setState({ notices: [] });
    expect(await runBackup('schedule')).toBe(false);
    expect(useStore.getState().notices).toHaveLength(0);
  });

  it('escribe un respaldo con todas las secciones (sin historial) y anota la marca', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos' });
    expect(await runBackup('manual')).toBe(true);
    expect(names()).toHaveLength(1);
    const entry = (await api.backupList()).entries[0]!;
    const saved = await api.importUserBundle({ path: entry.path });
    expect(saved.version).toBe(2);
    for (const k of ['devices', 'materials', 'costs', 'remnants', 'templates', 'cameras', 'art', 'prefs'] as const) expect(saved[k], k).toBeTruthy();
    expect(saved.history).toBeUndefined();
    expect((await loadBackupConfig()).last_backup_ms).toBeGreaterThan(0);
    expect(lastNotice()?.level).toBe('success');
  });

  it('con la casilla del historial lo incluye', async () => {
    await api.jobHistoryAppend({ entry: { id: 'j1', at: 5 } });
    await saveBackupSettings({ folder: '/mock/Respaldos', include_history: true });
    await runBackup('manual');
    const saved = await api.importUserBundle({ path: (await api.backupList()).entries[0]!.path });
    expect(saved.history?.entries.map((e) => e.id)).toEqual(['j1']);
  });

  it('rota: conserva solo los últimos N', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos', keep: 3 });
    for (let i = 0; i < 5; i++) await runBackup('manual');
    expect(names()).toHaveLength(3);
  });

  it('una carpeta no disponible avisa una sola vez y la aplicación sigue', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos', on_close: true });
    api.backupSim.folderAvailable = false;
    expect(await runBackup('schedule')).toBe(false);
    expect(await runBackup('close')).toBe(false);
    expect(await runBackup('schedule')).toBe(false);
    const warnings = useStore.getState().notices.filter((n) => n.level === 'warning');
    expect(warnings).toHaveLength(1);
    expect(warnings[0]!.text).toContain('/mock/Respaldos');
    // Vuelve la unidad: se respalda y el aviso se rearma.
    api.backupSim.folderAvailable = true;
    expect(await runBackup('schedule')).toBe(true);
    const firstId = warnings[0]!.id;
    api.backupSim.folderAvailable = false;
    await runBackup('schedule');
    const again = useStore.getState().notices.filter((n) => n.level === 'warning');
    expect(again).toHaveLength(1); // el aviso idéntico no se apila...
    expect(again[0]!.id).toBeGreaterThan(firstId); // ...pero se volvió a emitir
  });

  it('el manual con la carpeta ausente sí muestra el error', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos' });
    api.backupSim.folderAvailable = false;
    expect(await runBackup('manual')).toBe(false);
    expect(lastNotice()?.level).toBe('error');
  });

  it('el motor rechaza un paquete que lleve secretos y no escribe nada', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos' });
    const bad = { ...foreignBundle(), prefs: { format: 'tlaku-prefs', version: 1, prefs: { remoteToken: 'abc123' }, shortcuts: {} } } as unknown as UserBundle;
    await expect(api.backupRun({ bundle: bad })).rejects.toThrow('secreto');
    const url = { ...foreignBundle(), cameras: { presets: [{ network_url: 'rtsp://admin:1234@192.168.1.5/stream' }] } } as unknown as UserBundle;
    await expect(api.backupRun({ bundle: url })).rejects.toThrow('secreto');
    expect(names()).toHaveLength(0);
  });

  it('el respaldo que arma la aplicación no lleva usuario ni clave en las URL de cámara', async () => {
    const s = useStore.getState();
    s.setCameraSetup({ ...s.cameraSetup, presets: [{ ...(s.cameraSetup.presets?.[0] ?? {}), id: 'c1', name: 'IP', source: 'network', network_url: 'rtsp://admin:1234@192.168.1.5/stream', device_id: null } as never] });
    await saveBackupSettings({ folder: '/mock/Respaldos' });
    expect(await runBackup('manual')).toBe(true);
    const saved = await api.importUserBundle({ path: (await api.backupList()).entries[0]!.path });
    expect(JSON.stringify(saved)).not.toContain('admin:1234');
  });
});

describe('al cerrar', () => {
  it('solo si se pidió, hay carpeta y no hay un respaldo ajeno pendiente', async () => {
    expect(shouldBackupOnClose()).toBe(false);
    await saveBackupSettings({ folder: '/mock/Respaldos', on_close: true });
    expect(shouldBackupOnClose()).toBe(true);
    await backupOnClose();
    expect(names()).toHaveLength(1);
    useBackup.setState({ newer: { name: 'x', path: '/x', size_bytes: 1, created_ms: 1 } });
    expect(shouldBackupOnClose()).toBe(false);
    await backupOnClose();
    expect(names()).toHaveLength(1);
  });

  it('nunca lanza aunque el respaldo falle, y el tiempo máximo está acotado', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos', on_close: true });
    api.backupSim.folderAvailable = false;
    await expect(backupOnClose()).resolves.toBeUndefined();
    expect(CLOSE_BACKUP_TIMEOUT_MS).toBeLessThanOrEqual(10_000);
  });
});

describe('detección al abrir', () => {
  it('ofrece un respaldo más nuevo de otra computadora y no hace respaldos mientras tanto', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos', every_days: 1 });
    const foreign = api.backupSim.addForeign(foreignBundle());
    await runBackupStartup();
    expect(useBackup.getState().newer?.name).toBe(foreign.name);
    expect(names()).toHaveLength(1); // no se respaldó encima
    expect(await runBackup('schedule')).toBe(false);
    expect(await runBackup('close')).toBe(false);
    // Nunca se aplicó solo: el estado local sigue igual.
    expect(useStore.getState().devices.some((d) => d.id === 'de-la-otra')).toBe(false);
  });

  it('«Omitir» lo da por visto: ya no se ofrece y se reanudan los respaldos', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos', every_days: 1 });
    api.backupSim.addForeign(foreignBundle());
    await runBackupStartup();
    await markBackupSeen(useBackup.getState().newer!);
    expect(useBackup.getState().newer).toBeNull();
    await runBackupStartup();
    expect(useBackup.getState().newer).toBeNull();
    expect(names()).toHaveLength(2); // el programado corrió
  });

  it('un respaldo propio no se ofrece, ni uno más viejo que el último hecho aquí', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos' });
    await runBackup('manual');
    await runBackupStartup();
    expect(useBackup.getState().newer).toBeNull();
    api.backupSim.addForeign(foreignBundle(), Date.now() - 60 * 60 * 1000);
    await runBackupStartup();
    expect(useBackup.getState().newer).toBeNull();
  });

  it('sin carpeta o con la carpeta ausente no ofrece nada (y avisa una vez)', async () => {
    await runBackupStartup();
    expect(useBackup.getState().newer).toBeNull();
    await saveBackupSettings({ folder: '/mock/Respaldos' });
    api.backupSim.folderAvailable = false;
    await runBackupStartup();
    await runBackupStartup();
    expect(useStore.getState().notices.filter((n) => n.level === 'warning')).toHaveLength(1);
  });

  it('el programado corre al abrir cuando toca', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos', every_days: 3 });
    await runBackupStartup();
    expect(names()).toHaveLength(1);
    await runBackupStartup(); // acaba de respaldar: no toca
    expect(names()).toHaveLength(1);
    await runBackupStartup(Date.now() + 4 * 24 * 60 * 60 * 1000);
    expect(names()).toHaveLength(2);
  });
});

describe('diálogo de respaldo', () => {
  it('elige carpeta, activa las opciones, respalda y lista el respaldo con «Restaurar»', async () => {
    const m = await mount(<BackupDialog onClose={() => {}} />);
    const text = () => m.host.textContent ?? '';
    expect(text()).toContain('Todavía no elegiste una carpeta.');
    const chosen = [...m.host.querySelectorAll('button')].find((b) => b.textContent === 'Elegir carpeta…')!;
    await click(chosen);
    await until(() => m.host.querySelector('[data-testid="backup-folder"]')?.textContent === '/mock/Respaldos');
    expect(m.host.querySelector('[data-testid="backup-folder"]')?.textContent).toBe('/mock/Respaldos');
    expect((await loadBackupConfig()).folder).toBe('/mock/Respaldos');

    const onClose = m.host.querySelector<HTMLInputElement>('input[type="checkbox"]')!;
    await click(onClose);
    await until(async () => (await loadBackupConfig()).on_close);
    expect((await loadBackupConfig()).on_close).toBe(true);

    const now = [...m.host.querySelectorAll('button')].find((b) => b.textContent === 'Respaldar ahora')!;
    await click(now);
    await until(() => text().includes('Restaurar…'));
    expect(names()).toHaveLength(1);
    expect(text()).toContain('Restaurar…');
    expect(m.host.querySelector('[data-testid="backup-last"]')?.textContent).toContain('Último respaldo desde esta computadora');

    await click([...m.host.querySelectorAll('button')].find((b) => b.textContent === 'Restaurar…')!);
    expect(useBundleUi.getState().mode).toBe('import');
    expect(useBundleUi.getState().source?.path).toContain('tlaku-respaldo');
  });

  it('muestra el aviso cuando la carpeta no está disponible', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos' });
    api.backupSim.folderAvailable = false;
    const m = await mount(<BackupDialog onClose={() => {}} />);
    await until(() => (m.host.textContent ?? '').includes('La carpeta no está disponible ahora'));
    expect(m.host.textContent).toContain('La carpeta no está disponible ahora');
  });

  it('muestra el respaldo más nuevo pendiente con «Revisar…»', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos' });
    const foreign = api.backupSim.addForeign(foreignBundle());
    await runBackupStartup();
    const m = await mount(<BackupDialog onClose={() => {}} />);
    await until(() => (m.host.textContent ?? '').includes('Review…') || (m.host.textContent ?? '').includes('Revisar…'));
    expect(m.host.textContent).toContain('más nuevo que lo de esta computadora');
    await click([...m.host.querySelectorAll('button')].find((b) => b.textContent === 'Revisar…')!);
    expect(useBundleUi.getState().source).toEqual({ path: foreign.path, createdMs: foreign.created_ms });
  });
});

describe('pregunta al abrir', () => {
  it('ofrece revisar, dejar para después u omitir; nunca aplica', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos' });
    const foreign = api.backupSim.addForeign(foreignBundle());
    await runBackupStartup();
    useBackup.setState({ promptOpen: true });
    const m = await mount(<NewerBackupPrompt />);
    expect(m.host.textContent).toContain('Hay un respaldo más nuevo');
    expect(m.host.textContent).toContain('No se aplica nada sin tu confirmación');

    await click([...m.host.querySelectorAll('button')].find((b) => b.textContent === 'Ahora no')!);
    expect(useBackup.getState().promptOpen).toBe(false);
    expect(useBackup.getState().newer).not.toBeNull();

    useBackup.setState({ promptOpen: true });
    await m.rerender(<NewerBackupPrompt />);
    await click([...m.host.querySelectorAll('button')].find((b) => b.textContent === 'Revisar…')!);
    expect(useBundleUi.getState()).toMatchObject({ mode: 'import', source: { path: foreign.path } });
    expect(useStore.getState().devices.some((d) => d.id === 'de-la-otra')).toBe(false);
  });

  it('«Omitir este respaldo» lo anota como visto', async () => {
    await saveBackupSettings({ folder: '/mock/Respaldos' });
    api.backupSim.addForeign(foreignBundle());
    await runBackupStartup();
    useBackup.setState({ promptOpen: true });
    const m = await mount(<NewerBackupPrompt />);
    await click([...m.host.querySelectorAll('button')].find((b) => b.textContent === 'Omitir este respaldo')!);
    await until(() => useBackup.getState().newer === null);
    await settle();
    expect(useBackup.getState().newer).toBeNull();
    expect(await api.backupCheckNewer()).toBeNull();
  });
});

describe('comandos', () => {
  it('abren el diálogo de respaldo y respaldan ahora', async () => {
    await commands.run('backup.open');
    expect(useBundleUi.getState().mode).toBe('backup');
    await saveBackupSettings({ folder: '/mock/Respaldos' });
    await commands.run('backup.now');
    await until(() => names().length === 1);
    expect(names()).toHaveLength(1);
    expect(sampleDocument()).toBeTruthy();
  });
});
