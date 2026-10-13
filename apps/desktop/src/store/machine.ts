import { getApi } from '@/api';
import type { ConnectionEvent, JobEvent, LogEvent, ProgressEvent, RealtimeCommand, StatusReport } from '@/api/types';
import { errorMessage } from '@/lib/errors';
import { buildJobArgs } from './selectors';
import type { SliceCreator } from './types';

export type JobUiState = 'idle' | 'running' | 'paused' | 'finished' | 'cancelled' | 'error';

export interface MachineSlice {
  connection: ConnectionEvent;
  connecting: boolean;
  firmware: string | null;
  ports: string[];
  selectedPort: string | null;
  baud: number;
  status: StatusReport | null;
  progress: ProgressEvent | null;
  jobState: JobUiState;
  jobMessage: string;
  log: LogEvent[];

  refreshPorts: () => Promise<void>;
  setSelectedPort: (port: string | null) => void;
  setBaud: (baud: number) => void;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  sendLine: (line: string) => Promise<void>;
  realtime: (cmd: RealtimeCommand) => Promise<void>;
  jog: (dx: number, dy: number, speed?: number) => Promise<void>;
  startJob: () => Promise<void>;
  pauseJob: () => Promise<void>;
  resumeJob: () => Promise<void>;
  cancelJob: () => Promise<void>;
  frameJob: () => Promise<void>;
  clearLog: () => void;

  // Entrada de eventos `machine://*` (los conecta `bindMachineEvents`).
  onStatus: (s: StatusReport) => void;
  onProgress: (p: ProgressEvent) => void;
  onLog: (l: LogEvent) => void;
  onJob: (j: JobEvent) => void;
  onConnection: (c: ConnectionEvent) => void;
}

export const LOG_LIMIT = 500;
export const FRAME_SPEED = 6000;

const DISCONNECTED: ConnectionEvent = { connected: false, port: null, error: null };

export const createMachineSlice: SliceCreator<MachineSlice> = (set, get) => {
  /** Ejecuta una llamada al motor y convierte el rechazo en un aviso. */
  const guard = async (fn: () => Promise<unknown>): Promise<void> => {
    try {
      await fn();
    } catch (e) {
      get().notify('error', errorMessage(e));
    }
  };

  return {
    connection: DISCONNECTED,
    connecting: false,
    firmware: null,
    ports: [],
    selectedPort: null,
    baud: 115200,
    status: null,
    progress: null,
    jobState: 'idle',
    jobMessage: '',
    log: [],

    refreshPorts: () =>
      guard(async () => {
        const ports = await getApi().listPorts();
        set((s) => ({ ports, selectedPort: s.selectedPort && ports.includes(s.selectedPort) ? s.selectedPort : (ports[0] ?? null) }));
      }),
    setSelectedPort: (selectedPort) => set({ selectedPort }),
    setBaud: (baud) => set({ baud }),

    connect: async () => {
      const { selectedPort, baud } = get();
      if (!selectedPort) {
        get().notifyKey('warning', 'machine.noPort');
        return;
      }
      set({ connecting: true });
      try {
        const { firmware } = await getApi().connect({ port: selectedPort, baud });
        set({ firmware });
      } catch (e) {
        get().notify('error', errorMessage(e));
      } finally {
        set({ connecting: false });
      }
    },
    disconnect: () => guard(() => getApi().disconnect()),

    sendLine: (line) => guard(() => getApi().machineCommand({ line })),
    realtime: (cmd) => guard(() => getApi().machineRealtime({ cmd })),
    jog: (dx, dy, speed = 3000) => guard(() => getApi().jog({ dx, dy, speed })),

    startJob: () => guard(() => getApi().startJob(buildJobArgs(get()))),
    pauseJob: () => guard(() => getApi().pauseJob()),
    resumeJob: () => guard(() => getApi().resumeJob()),
    cancelJob: () => guard(() => getApi().cancelJob()),
    frameJob: () => guard(() => getApi().frameJob({ ...buildJobArgs(get()), speed: FRAME_SPEED })),
    clearLog: () => set({ log: [] }),

    onStatus: (status) => set({ status }),
    onProgress: (progress) => set({ progress }),
    onLog: (l) => set((s) => ({ log: [...s.log, l].slice(-LOG_LIMIT) })),
    onJob: ({ state, message }) => {
      const map = { started: 'running', paused: 'paused', finished: 'finished', cancelled: 'cancelled', error: 'error' } as const;
      set({ jobState: map[state], jobMessage: message, ...(state === 'started' && get().jobState === 'idle' ? { progress: null } : {}) });
      if (state === 'finished') get().notify('success', message);
      if (state === 'error') get().notify('error', message);
    },
    onConnection: (connection) => {
      set(
        connection.connected
          ? { connection }
          : { connection, firmware: null, status: null, progress: null, jobState: 'idle' },
      );
      if (connection.error) get().notify('error', connection.error);
    },
  };
};
