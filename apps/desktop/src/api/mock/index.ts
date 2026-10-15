import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { mockCamera } from './camera';
import { mockCodes } from './codes';
import { mockDevices } from './devices';
import { mockFileio } from './fileio';
import { mockMaterials } from './materials';
import { mockNest } from './nest';
import { mockRemnants } from './remnants';
import { mockBundle } from './bundle';
import { mockBackup, type MockBackupSim } from './backup';
import { mockLiveWindow } from './live';
import { mockPrefs } from './prefs';
import { mockVersions } from './versions';
import { mockJobHistory } from './jobHistory';
import { mockSettingsBackup } from './settingsBackup';
import { mockAutomation } from './automation';
import { mockRemote, type MockRemote } from './remote';
import { mockRuida } from './ruida';
import { mockResume } from './resume';
import { mockProbe } from './probe';
import { mockDesign } from './design';
import { mockParts } from './parts';
import { mockGeometry } from './geometry';
import { mockReview } from './review';
import { mockShapes } from './shapes';
import { mockText } from './text';
import { mockTrace } from './trace';
import { mockTraceExtras } from './traceExtras';
import { mockImages } from './images';
import { mockLayout } from './layout';
import { mockVariables } from './variables';
import type { LaserApi, UserBundle } from '../api';
import type { DeviceProfile, Document } from '../types';
import { mockComputeJob } from './job';
import { MockGalvo } from './galvo';
import { DEFAULT_MOCK_OPTIONS, MOCK_GALVO_PORT, MOCK_GALVO_USB_PORT, MOCK_LHY_PORT, MOCK_LHY_USB_PORT, MOCK_RUIDA_PORT, MOCK_VIRTUAL_NET_PORTS, MOCK_VIRTUAL_PORTS, MockMachine, type MockMachineOptions } from './machine';
import { sampleDevices, sampleDocument, sampleLayers, sampleObjects } from './sample';

export interface MockApi extends LaserApi {
  /** Para pruebas: acceso a la máquina simulada y limpieza de temporizadores. */
  readonly machine: MockMachine;
  /** Para pruebas: el galvo simulado (puntero rojo, aviso de energía, confirmaciones de seguridad). */
  readonly galvo: MockGalvo;
  /** Para pruebas: lo que el control remoto simulado recibió de la ventana. */
  readonly remote: MockRemote;
  /** Para pruebas y capturas: la carpeta de respaldo simulada (unidad ausente, respaldo de otra computadora). */
  readonly backupSim: MockBackupSim;
  dispose(): void;
}

const MOCK_VERSION = '0.1.0-mock';

/**
 * Implementación simulada en memoria. No sabe nada del motor: devuelve datos plausibles
 * (un diseño de ejemplo, un JobPreview con movimientos, una máquina que emite eventos)
 * para poder desarrollar la interfaz sin Rust.
 */
export function createMockApi(options: Partial<MockMachineOptions> = {}): MockApi {
  const opts = { ...DEFAULT_MOCK_OPTIONS, ...options };
  const machine = new MockMachine(opts);
  const galvo = new MockGalvo(machine);
  const devices = new Map<string, DeviceProfile>(sampleDevices().map((d) => [d.id, d]));
  const projects = new Map<string, Document>();
  const lag = <T>(value: () => T): Promise<T> =>
    new Promise((resolve, reject) =>
      setTimeout(() => {
        try {
          resolve(value());
        } catch (e) {
          reject(e);
        }
      }, opts.latencyMs),
    );
  const clone = <T>(v: T): T => structuredClone(v);
  const { onLive, ...liveWindowApi } = mockLiveWindow(lag);
  const remoteApi = mockRemote(lag);
  const ruidaApi = mockRuida(lag, machine);
  const resumeApi = mockResume(lag, machine);
  const bundleFiles = new Map<string, UserBundle>();
  const backup = mockBackup(lag, bundleFiles);

  return {
    machine,
    galvo,
    backupSim: backup.sim,
    dispose: () => machine.dispose(),

    appVersion: () => lag(() => MOCK_VERSION),

    setEngineLocale: () => lag(() => undefined),

    importFile: ({ path }) =>
      lag(() => {
        const warnings: string[] = [];
        if (/\.dxf$/i.test(path)) warnings.push('El DXF no declara unidades: se asumieron milímetros.');
        if (/\.svg$/i.test(path)) warnings.push('Se ignoró una máscara de recorte (clip-path) que el importador aún no aplica.');
        return { objects: sampleObjects(), new_layers: sampleLayers(), warnings };
      }),

    saveProject: ({ path, doc }) =>
      lag(() => {
        projects.set(path, clone(doc));
      }),
    loadProject: ({ path }) => lag(() => clone(projects.get(path) ?? sampleDocument())),

    computeJob: ({ doc, device, options: o }) => lag(() => mockComputeJob(doc, device, o)),
    exportGcode: ({ doc, device, options: o }) =>
      lag(() => {
        const p = mockComputeJob(doc, device, o);
        return { lines: p.moves.length / 4 + 6, estimate_s: p.total_s, warnings: p.warnings };
      }),

    listDevices: () => lag(() => [...devices.values()].map(clone)),
    saveDevice: ({ device }) =>
      lag(() => {
        devices.set(device.id, clone(device));
        // Solo un perfil se activa al abrir: el último en marcarse quita la marca a los demás.
        if (device.is_default) for (const d of devices.values()) if (d.id !== device.id) d.is_default = false;
      }),
    reorderDevices: ({ ids }) =>
      lag(() => {
        const rest = new Map(devices);
        const sorted = new Map<string, DeviceProfile>();
        for (const id of ids) {
          const d = rest.get(id);
          if (d) {
            sorted.set(id, d);
            rest.delete(id);
          }
        }
        for (const [id, d] of rest) sorted.set(id, d);
        devices.clear();
        for (const [id, d] of sorted) devices.set(id, d);
      }),
    jobLinkWarning: () => lag(() => null),
    deleteDevice: ({ id }) =>
      lag(() => {
        devices.delete(id);
      }),

    listPorts: () => lag(() => ['/dev/cu.usbserial-1420', '/dev/cu.usbmodem1101', MOCK_LHY_USB_PORT, MOCK_GALVO_USB_PORT, ...MOCK_VIRTUAL_PORTS, ...MOCK_VIRTUAL_NET_PORTS, MOCK_RUIDA_PORT, MOCK_LHY_PORT, MOCK_GALVO_PORT]),
    connect: ({ port, device, galvoAck }) =>
      lag(() => ({ firmware: device?.kind === 'galvo' ? galvo.connect(port, device, galvoAck) : machine.connect(port, device?.kind === 'ruida', device?.kind === 'lhy_micro') })),
    disconnect: () => lag(() => machine.disconnect()),
    machineCommand: ({ line }) => lag(() => machine.command(line)),
    machineRealtime: ({ cmd }) => lag(() => machine.realtime(cmd)),
    jog: ({ dx, dy, speed }) => lag(() => machine.jog(dx, dy, [400, 415], speed)),

    startJob: ({ doc, device, options: o, document }) =>
      lag(() => {
        const preview = mockComputeJob(doc, device, o);
        // Ruida guarda el trabajo como documento salvo que el perfil pida ejecutarlo al subir.
        if (device.kind === 'ruida' && !device.ruida.run_after_upload) return machine.storeDocument();
        const tile = o.tile ? ` · ${t('tiling.mock.tile', { x: fmt(o.tile.rect[0][0], 0), y: fmt(o.tile.rect[0][1], 0) })}` : '';
        machine.startJob(preview, device, tile);
        if (device.kind !== 'lhy_micro' && device.kind !== 'galvo') resumeApi.begin(preview, device, document ?? null);
      }),
    pauseJob: () => lag(() => machine.setPaused(true)),
    resumeJob: () => lag(() => machine.setPaused(false)),
    cancelJob: () => lag(() => machine.cancel()),
    frameJob: ({ doc, device, options: o }) =>
      lag(() => machine.frame(mockComputeJob(doc, device, o).bounds)),
    frameHull: ({ doc, device, options: o }) =>
      lag(() => machine.frameHull(mockComputeJob(doc, device, o).moves)),
    jogTo: ({ x, y, speed }) => lag(() => machine.jogTo(x, y, [400, 415], speed)),
    probeTouch: ({ dx, dy, feed, retract, confirmed }) => lag(() => machine.probe(dx, dy, feed, retract, confirmed)),
    jogAxis: ({ axis, delta, speed }) => lag(() => machine.jogAxis(axis, delta, speed, true)),
    jogAxisTo: ({ axis, pos, speed }) => lag(() => machine.jogAxis(axis, pos, speed, false)),
    fireTest: ({ power_pct, s_max, duration_ms }) => lag(() => machine.fire(power_pct, s_max, duration_ms)),
    setOverride: ({ kind, percent }) => lag(() => machine.setOverride(kind, percent)),
    simulateFault: ({ fault }) => lag(() => machine.simulateFault(fault)),
    interruptedJob: resumeApi.interruptedJob,
    interruptedJobView: resumeApi.interruptedJobView,
    discardInterruptedJob: resumeApi.discardInterruptedJob,
    continueCheck: resumeApi.continueCheck,
    continueFrame: resumeApi.continueFrame,
    continueJob: resumeApi.continueJob,
    ruidaInfo: () =>
      lag(() => {
        machine.requireConnection();
        if (!machine.ruida) throw new Error('Esta función es solo de las controladoras Ruida.');
        return {
          model: 'RDC6442S',
          card_id: '0x65106510',
          magic: 0x88,
          bed: [600, 400] as [number, number],
          rapid_mm_s: 200,
          home_mm_s: 20,
          axis_max_mm_s: [400, 400] as [number | null, number | null],
          axis_accel_mm_s2: [3000, 3000] as [number | null, number | null],
          total_on_s: 7200,
          total_work_s: 3600,
          total_laser_s: 1800,
          total_jobs: 42,
          travel_m: [120, 95] as [number | null, number | null],
          documents: machine.storedDocs.length,
          board_version: 'TLAKU-EMU',
          previous_work: 600,
          control_mode: 0,
          focus: true,
          z_return_docking: false,
          air_assist_mode: 1,
        };
      }),
    ruidaFiles: () =>
      lag(() => {
        machine.requireConnection();
        if (!machine.ruida) throw new Error('Esta función es solo de las controladoras Ruida.');
        return machine.storedDocs.map((name, i) => ({ index: i + 1, name }));
      }),

    ...ruidaApi,

    lhyPreview: ({ doc, device, options: o }) =>
      lag(() => {
        const preview = mockComputeJob(doc, device, o);
        // Como el codificador real: cada capa con salida que pide potencia avisa que la placa la ignora.
        const outputs = doc.layers.filter((l) => l.output && doc.objects.some((ob) => ob.layer === l.color && !ob.hidden));
        const warnings = outputs.flatMap((l, block) => [
          { kind: 'power_ignored' as const, block, layer: l.name, value: l.power, fires: false },
          ...(l.mode === 'image' && l.image.mode === 'grayscale' ? [{ kind: 'gray_thresholded' as const, block, layer: l.name, value: null, fires: false }] : []),
          ...(l.air ? [{ kind: 'air_assist_ignored' as const, block, layer: l.name, value: null, fires: false }] : []),
        ]);
        return {
          board: device.lhymicro.board.toUpperCase(),
          warnings,
          packets: Math.max(1, Math.round(preview.moves.length / 40)),
          bytes: Math.max(1, Math.round(preview.moves.length / 40)) * 34,
          est_seconds: preview.total_s,
          burn_mm: preview.layers.reduce((a, l) => a + l.seconds, 0) * 10,
          travel_mm: 100,
          homes_first: !machine.lhyKnown,
        };
      }),
    lhyHome: () => lag(() => machine.lhyHome()),
    lhyState: () => lag(() => machine.lhyState()),

    galvoPreview: (args) => lag(() => galvo.preview(args)),
    galvoState: () => lag(() => galvo.state()),
    galvoRedRun: ({ doc, device, options: o }) =>
      lag(() => {
        const preview = mockComputeJob(doc, device, o);
        machine.startJob(preview, device, 'recorrido en rojo');
      }),
    galvoRedLight: ({ on }) =>
      lag(() => {
        galvo.state();
        galvo.redLight = on;
      }),
    galvoConfirmMove: () =>
      lag(() => {
        if (!galvo.waiting) throw new Error('Ningún paso del trabajo espera que muevas el material.');
        galvo.waiting = null;
      }),
    galvoFrame: ({ shape, loops, gap_mm, ...job }) => lag(() => galvo.frame(job, shape, loops ?? null, gap_mm ?? null)),
    galvoAdjustFrame: ({ dx, dy, scale }) => lag(() => galvo.adjustFrame(dx, dy, scale)),
    galvoVerify: ({ device, port, galvo_ack }) => lag(() => galvo.verify(device, port, galvo_ack)),

    ...mockText(lag),
    ...mockVariables(lag),
    ...mockCodes(lag),
    ...mockGeometry(lag),
    ...mockShapes(lag),
    ...mockReview(lag),
    ...mockDesign(lag),
    ...mockParts(lag),
    ...mockTrace(lag),
    ...mockTraceExtras(lag),
    ...mockImages(lag),
    ...mockLayout(lag),
    ...mockNest(lag),
    ...mockRemnants(lag),
    ...mockCamera(lag),
    ...mockMaterials(lag),
    ...mockProbe(lag),
    ...mockAutomation(lag),
    ...remoteApi,
    ...mockDevices(lag),
    ...mockFileio(lag),
    ...mockPrefs(lag),
    ...mockVersions(lag, projects),
    ...liveWindowApi,
    ...mockSettingsBackup(lag),
    ...mockJobHistory(lag),
    ...mockBundle(lag, bundleFiles),
    ...backup.api,

    on: (event, handler) => Promise.resolve(event.startsWith('live://') ? onLive(event as `live://${string}` & typeof event, handler as never) : machine.on(event, handler)),
  };
}
