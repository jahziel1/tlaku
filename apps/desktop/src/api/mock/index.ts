import type { LaserApi } from '../api';
import type { DeviceProfile, Document } from '../types';
import { mockComputeJob } from './job';
import { DEFAULT_MOCK_OPTIONS, MockMachine, type MockMachineOptions } from './machine';
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

    listPorts: () => lag(() => ['/dev/cu.usbserial-1420', '/dev/cu.usbmodem1101']),
    connect: ({ port }) => lag(() => ({ firmware: machine.connect(port) })),
    disconnect: () => lag(() => machine.disconnect()),
    machineCommand: ({ line }) => lag(() => machine.command(line)),
    machineRealtime: ({ cmd }) => lag(() => machine.realtime(cmd)),
    jog: ({ dx, dy, speed }) => lag(() => machine.jog(dx, dy, [400, 415], speed)),

    startJob: ({ doc, device, options: o }) =>
      lag(() => machine.startJob(mockComputeJob(doc, device, o), device)),
    pauseJob: () => lag(() => machine.setPaused(true)),
    resumeJob: () => lag(() => machine.setPaused(false)),
    cancelJob: () => lag(() => machine.cancel()),
    frameJob: ({ doc, device, options: o }) =>
      lag(() => machine.frame(mockComputeJob(doc, device, o).bounds)),

    on: (event, handler) => Promise.resolve(machine.on(event, handler)),
  };
}
