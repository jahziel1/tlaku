import type { LaserApi } from '../api';
import type { RemoteAddr, RemoteClient, RemoteConfig, RemoteLiveRoute, RemoteSnapshot, RemoteStatus } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

const ADDRS: RemoteAddr[] = [
  { ip: '192.168.1.20', name: 'en0', recommended: true },
  { ip: '10.0.0.7', name: 'en1', recommended: false },
];

/** Lo que el control remoto simulado recibió de la ventana (para las pruebas). */
export interface MockRemote {
  config: RemoteConfig | null;
  snapshots: RemoteSnapshot[];
  routes: ({ bed: [number, number]; d: string } | null)[];
  liveRoutes: (RemoteLiveRoute | null)[];
  frames: number;
  startResults: { id: number; ok: boolean; code?: string; message?: string }[];
  clients: RemoteClient[];
  token: string;
}

function randomToken(): string {
  return Array.from({ length: 32 }, () => Math.floor(Math.random() * 16).toString(16)).join('');
}

/** Control remoto simulado: no abre ningún socket, solo guarda lo que la ventana le manda y valida la configuración como el motor. */
export function mockRemote(lag: Lag): Pick<LaserApi, 'remoteConfigure' | 'remoteStatus' | 'remoteRegenerate' | 'remoteKick' | 'remoteUpdate' | 'remoteSetRoute' | 'remoteSetLive' | 'remoteCameraFrame' | 'remoteStartResult'> & { remote: MockRemote } {
  const remote: MockRemote = { config: null, snapshots: [], routes: [], liveRoutes: [], frames: 0, startResults: [], clients: [], token: randomToken() };
  let error: string | null = null;
  const status = (): RemoteStatus => {
    const c = remote.config;
    const running = !!c && c.enabled && error === null;
    const ip = c?.ip && ADDRS.some((a) => a.ip === c.ip) ? c.ip : ADDRS[0]!.ip;
    const url = running && c ? `http://${ip}:${c.port}/` : null;
    return {
      running,
      port: c?.port ?? 0,
      ip: running ? ip : null,
      url,
      full_url: url ? `${url}#${remote.token}` : null,
      qr_svg: url ? '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#fff"/><rect x="1" y="1" width="3" height="3"/></svg>' : null,
      error,
      addrs: ADDRS,
      clients: running ? remote.clients : [],
    };
  };
  return {
    remote,
    remoteConfigure: ({ config }) =>
      lag(() => {
        remote.config = config;
        error = null;
        if (config.enabled && (!Number.isInteger(config.port) || config.port < 1024 || config.port > 65535)) {
          error = 'El puerto debe estar entre 1024 y 65535 (los menores necesitan privilegios).';
        }
        if (!config.enabled) remote.clients = [];
        return status();
      }),
    remoteStatus: () => lag(status),
    remoteRegenerate: () =>
      lag(() => {
        remote.token = randomToken();
        remote.clients = [];
        return status();
      }),
    remoteKick: ({ id }) =>
      lag(() => {
        remote.clients = remote.clients.filter((c) => c.id !== id);
        return status();
      }),
    remoteUpdate: ({ snapshot }) => lag(() => void remote.snapshots.push(snapshot)),
    remoteSetRoute: ({ route }) => lag(() => void remote.routes.push(route)),
    remoteSetLive: ({ route }) => lag(() => void remote.liveRoutes.push(route)),
    remoteCameraFrame: ({ jpeg_b64 }) =>
      lag(() => {
        if (jpeg_b64) remote.frames += 1;
        return remote.clients.length > 0 && jpeg_b64 !== '';
      }),
    remoteStartResult: ({ result }) => lag(() => void remote.startResults.push(result)),
  };
}
