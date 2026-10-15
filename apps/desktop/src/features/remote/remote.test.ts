import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getApi, setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { startLive } from '@/features/live/service';
import { applyBundle, buildBundle } from '@/features/settings/bundle';
import { bindMachineEvents, DEFAULT_PREFS, selectActiveDevice, useStore } from '@/store';
import { frameMarkOf, selectFramed } from '@/store/machine';
import { applyRemote, handleRemoteAction, handleRemoteStart, remoteConfig, remoteStartBlock, setFrameGrabber, startRemote, useRemote } from './remote';
import { buildRoute, buildSnapshot, jobFraction, layerAt } from './snapshot';

const memory = new Map<string, string>();
const s = () => useStore.getState();

async function connectedMachine(port = 'virtual:falcon2'): Promise<MockApi> {
  memory.clear();
  vi.stubGlobal('localStorage', { getItem: (k: string) => memory.get(k) ?? null, setItem: (k: string, v: string) => void memory.set(k, v) });
  const api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 200, jobSpeedup: 1e6 });
  // El encuadre simulado tarda varios segundos; aquí solo importa que el motor lo acepte.
  vi.spyOn(api, 'frameJob').mockResolvedValue();
  vi.spyOn(api, 'frameHull').mockResolvedValue();
  setApi(api);
  await s().loadDevices();
  await bindMachineEvents(api, useStore);
  await s().refreshPorts();
  s().setSelectedPort(port);
  await s().connect();
  await vi.waitFor(() => expect(s().status?.state).toBe('idle'));
  s().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });
  return api;
}

/** Encuadra (el motor simulado lo acepta al instante y la máquina sigue en reposo). */
async function frame(kind: 'rect' | 'hull' = 'rect'): Promise<void> {
  await (kind === 'rect' ? s().frameJob() : s().frameHull());
}

afterEach(async () => {
  await s().disconnect();
  s().setPref('remoteEnabled', false);
  s().setCutSelectionOnly(false);
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  setFrameGrabber(null);
  useRemote.setState({ status: null, clients: [] });
});

describe('configuración desde las preferencias', () => {
  it('de fábrica está apagado, con el puerto por omisión y la IP automática', () => {
    expect(remoteConfig(DEFAULT_PREFS)).toEqual({ enabled: false, port: 47802, ip: null });
    expect(DEFAULT_PREFS.remoteEnabled).toBe(false);
  });

  it('la IP y el puerto de las preferencias se sanean', () => {
    s().setPref('remotePort', 80);
    expect(s().prefs.remotePort).toBe(1024);
    s().setPref('remoteIp', 'no-es-una-ip');
    expect(s().prefs.remoteIp).toBe(DEFAULT_PREFS.remoteIp);
    s().setPref('remoteIp', '192.168.1.20');
    expect(s().prefs.remoteIp).toBe('192.168.1.20');
    s().setPref('remoteIp', null);
    s().setPref('remotePort', DEFAULT_PREFS.remotePort);
  });

  it('un paquete de preferencias ajeno nunca enciende el control remoto', () => {
    const bundle = buildBundle({ ...DEFAULT_PREFS, remoteEnabled: true, remotePort: 50000 }, {});
    expect(bundle.prefs.remoteEnabled).toBe(true);
    const change = applyBundle(bundle, { prefs: DEFAULT_PREFS, shortcuts: {} }, new Set());
    expect(change.prefs.remoteEnabled).toBe(false);
    expect(change.prefs.remotePort).toBe(50000);
    expect(change.ignoredPrefs).toBeGreaterThanOrEqual(1);
  });

  it('el estado real del motor se guarda, también cuando falla', async () => {
    setApi(createMockApi({ latencyMs: 0 }));
    const on = await applyRemote({ enabled: true, port: 47802, ip: null });
    expect(on.running).toBe(true);
    expect(on.full_url).toMatch(/^http:\/\/192\.168\.1\.20:47802\/#[0-9a-f]{32}$/);
    expect(useRemote.getState().status?.running).toBe(true);
    expect((await applyRemote({ enabled: true, port: 80, ip: null })).error).toMatch(/1024/);
    setApi({ remoteConfigure: () => Promise.reject('boom') } as never);
    expect(await applyRemote({ enabled: true, port: 5000, ip: null })).toMatchObject({ running: false, error: 'boom' });
  });
});

describe('marcador de encuadre tras el último cambio', () => {
  beforeEach(async () => {
    await connectedMachine();
  });

  it('no está encuadrado hasta que se encuadra, y encuadrar lo marca', async () => {
    expect(selectFramed(s())).toBe(false);
    await frame();
    expect(selectFramed(s())).toBe(true);
  });

  it('el encuadre en banda elástica también cuenta', async () => {
    await frame('hull');
    expect(selectFramed(s())).toBe(true);
  });

  it('cualquier cambio del documento lo invalida, y volver a encuadrar lo restituye', async () => {
    await frame();
    s().applyImport({ objects: sampleObjects().slice(0, 1), new_layers: [], warnings: [] });
    expect(selectFramed(s())).toBe(false);
    await frame();
    expect(selectFramed(s())).toBe(true);
  });

  it('cambiar las opciones del trabajo lo invalida', async () => {
    await frame();
    s().setJobOptions({ start_from: 'current_position' });
    expect(selectFramed(s())).toBe(false);
    await frame();
    expect(selectFramed(s())).toBe(true);
    s().setJobOptions({ layer_order: 'file' });
    expect(selectFramed(s())).toBe(false);
  });

  it('cambiar o editar la máquina lo invalida', async () => {
    await frame();
    await s().saveDevice({ ...selectActiveDevice(s()), name: 'Otro nombre' });
    expect(selectFramed(s())).toBe(false);
  });

  it('reconectar la máquina lo invalida; sin conexión nunca hay encuadre', async () => {
    await frame();
    expect(selectFramed(s())).toBe(true);
    await s().disconnect();
    await vi.waitFor(() => expect(s().connection.connected).toBe(false));
    expect(selectFramed(s())).toBe(false);
    s().setSelectedPort('virtual:falcon2');
    await s().connect();
    await vi.waitFor(() => expect(s().status?.state).toBe('idle'));
    expect(selectFramed(s())).toBe(false);
  });

  it('"solo la selección" es parte del trabajo: cambiarla invalida; la selección ajena no importa si no se usa', async () => {
    await frame();
    s().select([s().doc.objects[0]!.id]);
    expect(selectFramed(s())).toBe(true);
    s().setCutSelectionOnly(true);
    expect(selectFramed(s())).toBe(false);
    await frame();
    expect(selectFramed(s())).toBe(true);
    s().select([s().doc.objects[1]!.id]);
    expect(selectFramed(s())).toBe(false);
  });

  it('un encuadre rechazado por el motor no marca nada', async () => {
    const api = getApi();
    vi.spyOn(api, 'frameJob').mockRejectedValue(new Error('La máquina está en alarma.'));
    await frame();
    expect(s().jobError).toMatch(/alarma/);
    expect(selectFramed(s())).toBe(false);
  });

  it('un cambio mientras el motor encuadra también lo invalida (la marca es del estado de antes)', async () => {
    const api = getApi();
    vi.spyOn(api, 'frameJob').mockImplementation(async () => {
      s().setJobOptions({ start_from: 'current_position' });
    });
    await frame();
    expect(selectFramed(s())).toBe(false);
  });

  it('frameMarkOf usa las mismas referencias que el store', () => {
    const m = frameMarkOf(s());
    expect(m.doc).toBe(s().doc);
    expect(m.jobOptions).toBe(s().jobOptions);
    expect(m.selection).toBeNull();
  });
});

describe('resumen para el celular', () => {
  beforeEach(async () => {
    await connectedMachine();
  });

  it('en reposo informa la máquina, la cama y si hay algo que enviar y si está encuadrado', async () => {
    const a = buildSnapshot(s(), Date.now());
    expect(a).toMatchObject({ job_state: 'idle', has_output: true, framed: false, progress: 0, remaining_s: null, layer: null });
    expect(a.bed).toEqual(selectActiveDevice(s()).bed);
    expect(a.device_name).toBe(selectActiveDevice(s()).name);
    expect(a.head).not.toBeNull();
    await frame();
    expect(buildSnapshot(s(), Date.now()).framed).toBe(true);
  });

  it('el nombre del proyecto sale de su archivo', () => {
    useStore.setState({ filePath: '/tmp/mis cosas/placa.tlaku' });
    expect(buildSnapshot(s(), Date.now()).doc_name).toBe('placa.tlaku');
    useStore.setState({ filePath: null });
    expect(buildSnapshot(s(), Date.now()).doc_name).toBe('');
  });

  it('el avance sale de las líneas confirmadas y el tiempo restante del estimado', () => {
    useStore.setState({
      jobState: 'running',
      jobStartedAt: 1000,
      jobPausedMs: 0,
      jobPausedAt: null,
      jobEndedAt: null,
      progress: { acked: 25, total: 100 },
      preview: { layers: [], moves: [], move_times: [], total_s: 200, bounds: [[0, 0], [1, 1]], order_note: null, warnings: [] },
    });
    expect(jobFraction(s(), 31_000)).toBe(0.25);
    const snap = buildSnapshot(s(), 31_000);
    expect(snap).toMatchObject({ job_state: 'running', progress: 0.25, elapsed_s: 30, total_s: 200, remaining_s: 150 });
    useStore.setState({ jobState: 'finished', jobEndedAt: 61_000 });
    expect(buildSnapshot(s(), 99_000)).toMatchObject({ progress: 1, remaining_s: null });
  });

  it('con Ruida, K40 o galvo el avance es por tiempo (su progreso es la subida del archivo)', () => {
    const device = { ...selectActiveDevice(s()), kind: 'ruida' as const };
    useStore.setState({
      devices: [device],
      activeDeviceId: device.id,
      jobState: 'running',
      jobStartedAt: 0,
      jobPausedMs: 0,
      jobPausedAt: null,
      jobEndedAt: null,
      progress: { acked: 100, total: 100 },
      preview: { layers: [], moves: [], move_times: [], total_s: 100, bounds: [[0, 0], [1, 1]], order_note: null, warnings: [] },
    });
    expect(jobFraction(s(), 40_000)).toBeCloseTo(0.4);
  });

  it('la capa en curso sale del tiempo de cada capa', () => {
    const preview = {
      layers: [
        { color: '#ff0000', mode: 'line', seconds: 10, summary: '', travel_saved_pct: null, explanations: [] },
        { color: '#0000ff', mode: 'fill', seconds: 30, summary: '', travel_saved_pct: null, explanations: [] },
      ],
      moves: [],
      move_times: [],
      total_s: 40,
      bounds: [[0, 0], [1, 1]] as [[number, number], [number, number]],
      order_note: null,
      warnings: [],
    };
    expect(layerAt(preview, 0)).toEqual({ index: 1, count: 2, color: '#ff0000', mode: 'line' });
    expect(layerAt(preview, 0.3)).toMatchObject({ index: 2 });
    expect(layerAt(preview, 1)).toMatchObject({ index: 2 });
    expect(layerAt(null, 0.5)).toBeNull();
  });
});

describe('recorrido', () => {
  const preview = (moves: number[]) => ({ layers: [], moves, move_times: [], total_s: 1, bounds: [[0, 0], [1, 1]] as [[number, number], [number, number]], order_note: null, warnings: [] });

  it('dibuja solo los movimientos con láser, sin los saltos ni el overscan', () => {
    const r = buildRoute(preview([0, 0, 0, 0, 1, 10, 0, 50, 1, 10, 10, 50, 0, 50, 50, 0, 1, 60, 50, 0, 1, 70, 50, 80]), [100, 100]);
    expect(r?.d).toBe('M0 0L10 0L10 10M60 50L70 50');
    expect(r?.bed).toEqual([100, 100]);
  });

  it('sin trazos o sin vista previa no hay recorrido', () => {
    expect(buildRoute(null, [100, 100])).toBeNull();
    expect(buildRoute(preview([]), [100, 100])).toBeNull();
    expect(buildRoute(preview([0, 1, 1, 0]), [100, 100])).toBeNull();
  });

  it('un relleno enorme se resume a unos pocos miles de puntos y solo usa caracteres permitidos', () => {
    const moves: number[] = [];
    for (let row = 0; row < 20_000; row++) {
      const y = row * 0.01;
      moves.push(0, 0, y, 0, 1, 100, y, 80);
    }
    const r = buildRoute(preview(moves), [200, 200])!;
    const points = (r.d.match(/[ML]/g) ?? []).length;
    expect(points).toBeLessThanOrEqual(6100);
    expect(r.d.length).toBeLessThan(200_000);
    expect(r.d).toMatch(/^[0-9 .\-ML]+$/);
  });
});

describe('INICIAR desde el celular', () => {
  beforeEach(async () => {
    await connectedMachine();
  });

  it('explica por qué no se puede, en este orden', async () => {
    expect(remoteStartBlock(s())).toBe('not_framed');
    useStore.setState({ doc: { ...s().doc, objects: [] } });
    expect(remoteStartBlock(s())).toBe('nothing_to_send');
    await s().disconnect();
    await vi.waitFor(() => expect(s().connection.connected).toBe(false));
    expect(remoteStartBlock(s())).toBe('not_connected');
  });

  it('un galvo nunca se inicia desde el celular, aunque esté encuadrado', async () => {
    await frame();
    const device = { ...selectActiveDevice(s()), kind: 'galvo' as const };
    useStore.setState({ devices: [device], activeDeviceId: device.id });
    useStore.setState({ framedMark: frameMarkOf(s()) });
    expect(selectFramed(s())).toBe(true);
    expect(remoteStartBlock(s())).toBe('galvo');
  });

  it('sin encuadre previo se rechaza y no se inicia nada', async () => {
    const api = setupApi();
    const start = vi.spyOn(s(), 'startJob');
    await handleRemoteStart({ id: 7, client: '192.168.1.9 · iPhone' });
    expect(start).not.toHaveBeenCalled();
    expect(api.remote.startResults).toEqual([{ id: 7, ok: false, code: 'not_framed', message: expect.stringContaining('encuadr') }]);
    expect(s().jobState).toBe('idle');
    expect(s().notices.at(-1)?.text).toMatch(/192\.168\.1\.9/);
  });

  it('con el trabajo encuadrado inicia por la ruta de siempre y contesta que salió bien', async () => {
    const api = setupApi();
    await frame();
    await handleRemoteStart({ id: 8, client: '192.168.1.9 · iPhone' });
    expect(api.remote.startResults).toEqual([{ id: 8, ok: true, code: '', message: '' }]);
    await vi.waitFor(() => expect(s().jobState).not.toBe('idle'));
  });

  it('si el motor rechaza el inicio (preflight), el celular recibe el mensaje', async () => {
    const api = setupApi();
    await frame();
    vi.spyOn(api, 'startJob').mockRejectedValue(new Error('Preflight: la potencia máxima no coincide.'));
    await handleRemoteStart({ id: 9, client: 'x' });
    expect(api.remote.startResults).toEqual([{ id: 9, ok: false, code: 'failed', message: 'Preflight: la potencia máxima no coincide.' }]);
  });

  it('un cambio entre el pedido y la respuesta de la ventana también se rechaza', async () => {
    const api = setupApi();
    await frame();
    s().setJobOptions({ start_from: 'current_position' });
    await handleRemoteStart({ id: 10, client: 'x' });
    expect(api.remote.startResults[0]).toMatchObject({ ok: false, code: 'not_framed' });
  });

  it('detener desde el celular corta el bucle de encuadre y el puntero rojo', () => {
    useStore.setState({ frameLooping: true, galvoRedOn: true });
    handleRemoteAction({ action: 'stop', client: '192.168.1.9 · iPhone' });
    expect(s().frameLooping).toBe(false);
    expect(s().galvoRedOn).toBe(false);
    expect(s().notices.at(-1)).toMatchObject({ level: 'warning' });
    useStore.setState({ frameLooping: true });
    handleRemoteAction({ action: 'pause', client: 'x' });
    expect(s().frameLooping).toBe(true);
  });
});

/** La API simulada que dejó puesta `connectedMachine`. */
const setupApi = (): MockApi => getApi() as MockApi;

describe('arranque: servidor, informe, recorrido y cámara', () => {
  let api: MockApi;
  let stop: () => void;

  beforeEach(async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    api = await connectedMachine();
  });
  afterEach(() => {
    stop?.();
    vi.useRealTimers();
  });

  it('apagado de fábrica configura el servidor apagado y no informa nada', async () => {
    stop = startRemote();
    await vi.waitFor(() => expect(api.remote.config).toEqual({ enabled: false, port: 47802, ip: null }));
    await vi.advanceTimersByTimeAsync(5000);
    expect(api.remote.snapshots).toHaveLength(0);
    expect(api.remote.routes).toHaveLength(0);
    expect(api.remote.frames).toBe(0);
  });

  it('al activarlo informa el estado con el marcador de encuadre y manda el recorrido', async () => {
    stop = startRemote();
    await vi.waitFor(() => expect(api.remote.config).not.toBeNull());
    s().setPref('remoteEnabled', true);
    await vi.waitFor(() => expect(useRemote.getState().status?.running).toBe(true));
    await vi.advanceTimersByTimeAsync(1500);
    await vi.waitFor(() => expect(api.remote.snapshots.length).toBeGreaterThan(0));
    expect(api.remote.snapshots.at(-1)).toMatchObject({ has_output: true, framed: false });
    await s().computePreview();
    await vi.advanceTimersByTimeAsync(1500);
    await vi.waitFor(() => expect(api.remote.routes.length).toBeGreaterThan(0));
    expect(api.remote.routes.at(-1)?.d).toMatch(/^M/);
    // Al encuadrar, el siguiente informe lo dice.
    await frame();
    await vi.advanceTimersByTimeAsync(1000);
    await vi.waitFor(() => expect(api.remote.snapshots.at(-1)?.framed).toBe(true));
    // Un cambio al diseño lo borra otra vez.
    s().setJobOptions({ start_from: 'current_position' });
    await vi.advanceTimersByTimeAsync(1000);
    await vi.waitFor(() => expect(api.remote.snapshots.at(-1)?.framed).toBe(false));
  });

  it('la cámara solo se manda con celulares conectados y la cámara encendida; al apagarla se borra', async () => {
    let n = 0;
    setFrameGrabber(() => `jpeg${++n}`);
    stop = startRemote();
    s().setPref('remoteEnabled', true);
    await vi.waitFor(() => expect(useRemote.getState().status?.running).toBe(true));
    // Cámara encendida pero nadie conectado: nada.
    s().setCameraState({ cameraStatus: 'live' });
    await vi.advanceTimersByTimeAsync(8000);
    expect(api.remote.frames).toBe(0);
    // Entra un celular.
    api.remote.clients = [{ id: 'a1b2c3d4', ip: '192.168.1.9', agent: 'iPhone · Safari', since_ms: 1, idle_s: 0, connected: true }];
    useRemote.setState({ clients: api.remote.clients });
    await vi.advanceTimersByTimeAsync(3000);
    await vi.waitFor(() => expect(api.remote.frames).toBeGreaterThan(0));
    const sent = api.remote.frames;
    // Cámara apagada: deja de mandar.
    s().setCameraState({ cameraStatus: 'off' });
    await vi.advanceTimersByTimeAsync(8000);
    expect(api.remote.frames).toBe(sent);
  });

  describe('vista en vivo del celular (solo lectura)', () => {
    let stopLive: () => void;
    afterEach(() => stopLive?.());

    const phone = () => {
      api.remote.clients = [{ id: 'a1b2c3d4', ip: '192.168.1.9', agent: 'iPhone · Safari', since_ms: 1, idle_s: 0, connected: true }];
      useRemote.setState({ clients: api.remote.clients });
    };

    it('sin celulares conectados no se sigue el trabajo ni se manda ruta, aunque el control remoto esté encendido', async () => {
      stopLive = startLive(useStore);
      stop = startRemote();
      s().setPref('remoteEnabled', true);
      await vi.waitFor(() => expect(useRemote.getState().status?.running).toBe(true));
      await s().computePreview();
      await s().startJob();
      await vi.waitFor(() => expect(s().jobState).toBe('running'));
      await vi.advanceTimersByTimeAsync(1500);
      expect(api.remote.liveRoutes.filter((r) => r !== null)).toHaveLength(0);
      expect(api.remote.snapshots.at(-1)?.live ?? null).toBeNull();
    });

    it('con un celular conectado manda la ruta UNA vez y en cada informe dónde está la máquina', async () => {
      stopLive = startLive(useStore);
      stop = startRemote();
      s().setPref('remoteEnabled', true);
      await vi.waitFor(() => expect(useRemote.getState().status?.running).toBe(true));
      phone();
      await s().computePreview();
      await s().startJob();
      await vi.waitFor(() => expect(api.remote.liveRoutes.some((r) => r !== null)).toBe(true));
      const routes = api.remote.liveRoutes.filter((r) => r !== null);
      expect(routes).toHaveLength(1);
      const r = routes[0]!;
      expect(r.runs.length).toBeGreaterThan(0);
      expect(JSON.stringify(r).length).toBeLessThanOrEqual(300_000);
      expect(r.layers.every((c) => /^#[0-9a-f]{6}$/i.test(c))).toBe(true);
      // Los informes de estado llevan la ubicación sobre esa ruta y nada más.
      await vi.waitFor(() => expect(api.remote.snapshots.some((x) => x.live)).toBe(true));
      const live = api.remote.snapshots.findLast((x) => x.live)!.live!;
      expect(Object.keys(live).sort()).toEqual(['retreat_seq', 'running', 'src', 't']);
      expect(live.t).toBeGreaterThanOrEqual(0);
      expect(live.t).toBeLessThanOrEqual(r.total + 0.01);
      // Seguir informando no vuelve a mandar la ruta.
      await vi.advanceTimersByTimeAsync(1500);
      expect(api.remote.liveRoutes.filter((x) => x !== null)).toHaveLength(1);
    });

    it('un celular que entra con el trabajo a la mitad recibe la ruta entonces', async () => {
      stopLive = startLive(useStore);
      stop = startRemote();
      s().setPref('remoteEnabled', true);
      await vi.waitFor(() => expect(useRemote.getState().status?.running).toBe(true));
      await s().computePreview();
      await s().startJob();
      await vi.waitFor(() => expect(s().jobState).toBe('running'));
      expect(api.remote.liveRoutes.filter((x) => x !== null)).toHaveLength(0);
      phone();
      await vi.waitFor(() => expect(api.remote.liveRoutes.filter((x) => x !== null)).toHaveLength(1));
    });

    it('al empezar otro trabajo se borra la ruta vieja y se manda la nueva', async () => {
      stopLive = startLive(useStore);
      stop = startRemote();
      s().setPref('remoteEnabled', true);
      await vi.waitFor(() => expect(useRemote.getState().status?.running).toBe(true));
      phone();
      await s().computePreview();
      await s().startJob();
      await vi.waitFor(() => expect(api.remote.liveRoutes.some((r) => r !== null)).toBe(true));
      await vi.waitFor(() => expect(s().jobState).toBe('finished'), { timeout: 8000 });
      await s().startJob();
      await vi.waitFor(() => expect(api.remote.liveRoutes.filter((r) => r !== null)).toHaveLength(2));
      expect(api.remote.liveRoutes).toContain(null);
    });
  });

  it('las pruebas dejan el servidor apagado', () => {
    expect(s().prefs.remoteEnabled).toBe(false);
  });
});
