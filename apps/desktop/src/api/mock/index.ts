import { mockCamera } from './camera';
import { mockCodes } from './codes';
import { mockDevices } from './devices';
import { mockFileio } from './fileio';
import { mockMaterials } from './materials';
import { mockNest } from './nest';
import { mockBundle } from './bundle';
import { mockPrefs } from './prefs';
import { mockSettingsBackup } from './settingsBackup';
import { mockAutomation } from './automation';
import { mockProbe } from './probe';
import { mockDesign } from './design';
import { mockGeometry } from './geometry';
import { mockShapes } from './shapes';
import { mockText } from './text';
import { mockTrace } from './trace';
import { mockTraceExtras } from './traceExtras';
import { mockVariables } from './variables';
import type { LaserApi } from '../api';
import type { DeviceProfile, Document } from '../types';
import { mockComputeJob } from './job';
import { DEFAULT_MOCK_OPTIONS, MOCK_LHY_PORT, MOCK_LHY_USB_PORT, MOCK_RUIDA_PORT, MOCK_VIRTUAL_NET_PORTS, MOCK_VIRTUAL_PORTS, MockMachine, type MockMachineOptions } from './machine';
import { sampleDevices, sampleDocument, sampleLayers, sampleObjects } from './sample';

export interface MockApi extends LaserApi {
  /** Para pruebas: acceso a la máquina simulada y limpieza de temporizadores. */
  readonly machine: MockMachine;
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

  return {
    machine,
    dispose: () => machine.dispose(),

    appVersion: () => lag(() => MOCK_VERSION),

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
      }),
    deleteDevice: ({ id }) =>
      lag(() => {
        devices.delete(id);
      }),

    listPorts: () => lag(() => ['/dev/cu.usbserial-1420', '/dev/cu.usbmodem1101', MOCK_LHY_USB_PORT, ...MOCK_VIRTUAL_PORTS, ...MOCK_VIRTUAL_NET_PORTS, MOCK_RUIDA_PORT, MOCK_LHY_PORT]),
    connect: ({ port, device }) => lag(() => ({ firmware: machine.connect(port, device?.kind === 'ruida', device?.kind === 'lhy_micro') })),
    disconnect: () => lag(() => machine.disconnect()),
    machineCommand: ({ line }) => lag(() => machine.command(line)),
    machineRealtime: ({ cmd }) => lag(() => machine.realtime(cmd)),
    jog: ({ dx, dy, speed }) => lag(() => machine.jog(dx, dy, [400, 415], speed)),

    startJob: ({ doc, device, options: o }) =>
      lag(() => {
        const preview = mockComputeJob(doc, device, o);
        // Ruida guarda el trabajo como documento salvo que el perfil pida ejecutarlo al subir.
        if (device.kind === 'ruida' && !device.ruida.run_after_upload) return machine.storeDocument();
        return machine.startJob(preview, device);
      }),
    pauseJob: () => lag(() => machine.setPaused(true)),
    resumeJob: () => lag(() => machine.setPaused(false)),
    cancelJob: () => lag(() => machine.cancel()),
    frameJob: ({ doc, device, options: o }) =>
      lag(() => machine.frame(mockComputeJob(doc, device, o).bounds)),
    frameHull: ({ doc, device, options: o }) =>
      lag(() => machine.frameHull(mockComputeJob(doc, device, o).moves)),
    jogTo: ({ x, y, speed }) => lag(() => machine.jogTo(x, y, [400, 415], speed)),
    jogAxis: ({ axis, delta, speed }) => lag(() => machine.jogAxis(axis, delta, speed, true)),
    jogAxisTo: ({ axis, pos, speed }) => lag(() => machine.jogAxis(axis, pos, speed, false)),
    fireTest: ({ power_pct, s_max, duration_ms }) => lag(() => machine.fire(power_pct, s_max, duration_ms)),
    setOverride: ({ kind, percent }) => lag(() => machine.setOverride(kind, percent)),
    simulateFault: ({ fault }) => lag(() => machine.simulateFault(fault)),
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
        };
      }),
    ruidaFiles: () =>
      lag(() => {
        machine.requireConnection();
        if (!machine.ruida) throw new Error('Esta función es solo de las controladoras Ruida.');
        return machine.storedDocs.map((name, i) => ({ index: i + 1, name }));
      }),

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

    ...mockText(lag),
    ...mockVariables(lag),
    ...mockCodes(lag),
    ...mockGeometry(lag),
    ...mockShapes(lag),
    ...mockDesign(lag),
    ...mockTrace(lag),
    ...mockTraceExtras(lag),
    ...mockNest(lag),
    ...mockCamera(lag),
    ...mockMaterials(lag),
    ...mockProbe(lag),
    ...mockAutomation(lag),
    ...mockDevices(lag),
    ...mockFileio(lag),
    ...mockPrefs(lag),
    ...mockSettingsBackup(lag),
    ...mockBundle(lag),

    on: (event, handler) => Promise.resolve(machine.on(event, handler)),
  };
}
