import { getApi, type EventMap } from '@/api';
import type { RemoteClient, RemoteConfig, RemoteLiveRoute, RemoteStatus } from '@/api/api';
import { buildLivePayload } from '@/features/live/remoteRoute';
import { addLiveConsumer, getLiveService, onLiveRoute, pokeLive } from '@/features/live/service';
import type { LiveRoute } from '@/features/live/route';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { selectActiveDevice, selectJobPreview, useStore, hasOutput, type Prefs } from '@/store';
import { selectFramed } from '@/store/machine';
import type { AppState } from '@/store/types';
import { create } from 'zustand';
import { grabCameraFrame } from './cameraFrame';
import { buildRoute, buildSnapshot } from './snapshot';

/**
 * Control remoto desde el celular, lado de la ventana. El servidor (`src-tauri/src/remote`) atiende
 * la red; aquí vive lo que depende del documento y de las opciones del trabajo:
 *
 * - mantiene el servidor al día con las preferencias;
 * - le informa el estado del trabajo (progreso, tiempo, capa, cabezal y, sobre todo, si ya se
 *   encuadró tras el último cambio), todo a partir de los eventos `machine://*` que el store ya sigue;
 * - le manda el recorrido y, solo si hay celulares conectados y la cámara está encendida, un
 *   cuadro reducido cada pocos segundos;
 * - para la vista en vivo del celular (solo lectura, ver `features/live`): con celulares conectados
 *   pide a la PC seguir el trabajo, le manda la ruta simplificada UNA vez por trabajo y, en cada
 *   informe, dónde está la máquina sobre ella;
 * - atiende el pedido de INICIAR del celular con la ruta de siempre (`startJob`), volviendo a
 *   comprobar el encuadre: la lógica de inicio no se duplica en el servidor.
 */

/** Cada cuánto se manda un cuadro de la cámara. */
export const CAMERA_EVERY_MS = 2500;
/** Mínimo entre dos informes de estado. */
export const SNAPSHOT_EVERY_MS = 250;
/** Si nada cambia, igual se informa cada tanto para que el servidor sepa que la ventana sigue viva. */
export const HEARTBEAT_MS = 4000;

/** Estado del servidor y de los clientes, para Preferencias. `null` mientras no se ha configurado. */
export const useRemote = create<{ status: RemoteStatus | null; clients: RemoteClient[] }>()(() => ({ status: null, clients: [] }));

/** Las preferencias que configuran el servidor. */
export function remoteConfig(p: Pick<Prefs, 'remoteEnabled' | 'remotePort' | 'remoteIp'>): RemoteConfig {
  return { enabled: p.remoteEnabled, port: p.remotePort, ip: p.remoteIp };
}

function setStatus(status: RemoteStatus): RemoteStatus {
  useRemote.setState({ status, clients: status.clients });
  return status;
}

/** Aplica la configuración en el motor y guarda el estado real (puerto ocupado, sin red, etc.). */
export async function applyRemote(config: RemoteConfig): Promise<RemoteStatus> {
  try {
    return setStatus(await getApi().remoteConfigure({ config }));
  } catch (e) {
    return setStatus({ running: false, port: config.port, ip: null, url: null, full_url: null, qr_svg: null, error: errorMessage(e), addrs: [], clients: [] });
  }
}

/** Clave nueva: los clientes conectados quedan expulsados. */
export async function regenerateRemote(): Promise<void> {
  setStatus(await getApi().remoteRegenerate());
}

/** Desconecta a un cliente. */
export async function kickRemoteClient(id: string): Promise<void> {
  setStatus(await getApi().remoteKick({ id }));
}

// ---------------------------------------------------------------------------
// Pedido de inicio del celular
// ---------------------------------------------------------------------------

export type RemoteStartCode = 'not_connected' | 'job_active' | 'not_idle' | 'nothing_to_send' | 'galvo' | 'not_framed';

/**
 * Por qué NO se puede iniciar desde el celular ahora (o `null`). Es la comprobación de la ventana;
 * el servidor ya hizo la suya con el estado de la máquina, y esta vuelve a mirar con lo último que
 * sabe la interfaz (un cambio de último momento invalida el encuadre).
 */
export function remoteStartBlock(s: AppState): RemoteStartCode | null {
  if (!s.connection.connected) return 'not_connected';
  if (s.jobState === 'running' || s.jobState === 'paused') return 'job_active';
  const state = s.status?.state;
  if (state && state !== 'idle') return 'not_idle';
  if (!hasOutput(s)) return 'nothing_to_send';
  // Haz invisible: el galvo se inicia mirando la máquina desde la PC, con su diálogo.
  if (selectActiveDevice(s).kind === 'galvo') return 'galvo';
  if (!selectFramed(s)) return 'not_framed';
  return null;
}

const reasonText = (code: string): string => {
  const key = `remote.reason.${code}` as I18nKey;
  return t(key);
};

/**
 * `remote://start`: un celular pidió iniciar (con su checklist completo). Se inicia por la ruta de
 * siempre (preflight incluido) y se le cuenta al servidor qué pasó.
 */
export async function handleRemoteStart(req: EventMap['remote://start']): Promise<void> {
  const api = getApi();
  const s = useStore.getState();
  const answer = (ok: boolean, code = '', message = '') => api.remoteStartResult({ result: { id: req.id, ok, code, message } }).catch(() => undefined);
  const blocked = remoteStartBlock(s);
  if (blocked) {
    const reason = reasonText(blocked);
    s.notify('warning', t('remote.notice.startRefused', { client: req.client, reason }));
    await answer(false, blocked, reason);
    return;
  }
  s.notify('info', t('remote.notice.starting', { client: req.client }));
  await s.startJob();
  const error = useStore.getState().jobError;
  if (error) {
    await answer(false, 'failed', error);
    return;
  }
  await answer(true);
}

/** `remote://action`: se pausó, reanudó o detuvo desde un celular. */
export function handleRemoteAction(a: EventMap['remote://action']): void {
  const s = useStore.getState();
  s.notify(a.action === 'stop' ? 'warning' : 'info', t(`remote.notice.${a.action}`, { client: a.client }));
  if (a.action === 'stop') {
    // Lo mismo que hace el botón de detener de la PC con lo que es solo de la interfaz.
    s.stopFrameLoop();
    useStore.setState({ galvoRedOn: false, jogging: false });
  }
}

// ---------------------------------------------------------------------------
// Arranque
// ---------------------------------------------------------------------------

/** Cuadros de cámara: sustituible en pruebas. */
let frameGrabber: () => string | null = grabCameraFrame;
export function setFrameGrabber(fn: (() => string | null) | null): void {
  frameGrabber = fn ?? grabCameraFrame;
}

/** Suscribe los eventos del motor, mantiene el servidor al día con las preferencias y lo informa. Devuelve cómo deshacerlo. */
export function startRemote(): () => void {
  const api = getApi();
  let stopped = false;
  const offs: (() => void)[] = [];
  const listen = <K extends 'remote://clients' | 'remote://start' | 'remote://action'>(event: K, handler: (p: EventMap[K]) => void) => {
    void api.on(event, handler).then((off) => (stopped ? off() : offs.push(off)));
  };
  listen('remote://clients', (p) => useRemote.setState((r) => ({ clients: p.clients, status: r.status ? { ...r.status, clients: p.clients } : r.status })));
  listen('remote://start', (p) => void handleRemoteStart(p));
  listen('remote://action', handleRemoteAction);

  const running = () => useRemote.getState().status?.running === true;

  // ---- informe de estado ----
  let timer: ReturnType<typeof setTimeout> | undefined;
  let lastSent = 0;
  let lastJson = '';
  const push = () => {
    timer = undefined;
    if (stopped || !running()) return;
    const snapshot = buildSnapshot(useStore.getState(), Date.now());
    const json = JSON.stringify(snapshot);
    // Sin cambios no se manda nada, salvo el latido.
    if (json === lastJson && Date.now() - lastSent < HEARTBEAT_MS) return;
    lastJson = json;
    lastSent = Date.now();
    void api.remoteUpdate({ snapshot }).catch(() => undefined);
  };
  const schedule = () => {
    if (timer !== undefined || stopped || !running()) return;
    timer = setTimeout(push, Math.max(0, SNAPSHOT_EVERY_MS - (Date.now() - lastSent)));
  };

  // ---- recorrido ----
  let routeTimer: ReturnType<typeof setTimeout> | undefined;
  let routeFor: unknown = Symbol('ninguno');
  const pushRoute = () => {
    routeTimer = undefined;
    if (stopped || !running()) return;
    const s = useStore.getState();
    // Con un trabajo en curso, el celular ve el recorrido de la pestaña desde la que se lanzó.
    const preview = selectJobPreview(s);
    routeFor = preview?.moves ?? null;
    const device = selectActiveDevice(s);
    void api.remoteSetRoute({ route: buildRoute(preview, [device.bed[0], device.bed[1]]) }).catch(() => undefined);
  };

  // ---- vista en vivo del celular (solo lectura) ----
  // Con celulares conectados la PC sigue el trabajo (ubica la posición real en la ruta) aunque su
  // propia vista en vivo esté apagada; sin celulares no hace nada.
  const hasClients = () => running() && useRemote.getState().clients.some((c) => c.connected);
  const removeConsumer = addLiveConsumer(hasClients);
  // `undefined`: todavía no hay nada que mandar; `null`: borrar la ruta del servidor.
  let livePayload: RemoteLiveRoute | null | undefined;
  let liveFor: LiveRoute | null = null;
  const sendLive = () => {
    if (stopped || !running() || livePayload === undefined) return;
    void api.remoteSetLive({ route: livePayload }).catch(() => undefined);
  };
  const offLiveRoute = onLiveRoute((route) => {
    liveFor = route;
    if (!route) {
      livePayload = null;
      return sendLive();
    }
    // Se arma fuera del camino del evento: con trabajos enormes tarda unos milisegundos.
    setTimeout(() => {
      if (stopped || liveFor !== route) return;
      const bed = getLiveService()?.device?.bed ?? [0, 0];
      livePayload = buildLivePayload(route, [bed[0], bed[1]]);
      sendLive();
    }, 0);
  });

  // ---- cámara ----
  let cameraBusy = false;
  let cameraSent = false;
  const pushCamera = async () => {
    if (stopped || !running() || cameraBusy) return;
    const s = useStore.getState();
    const anyone = useRemote.getState().clients.some((c) => c.connected);
    const jpeg = anyone && s.cameraStatus === 'live' ? frameGrabber() : null;
    if (!jpeg && !cameraSent) return; // nada que mandar ni que borrar
    cameraBusy = true;
    try {
      await api.remoteCameraFrame({ jpeg_b64: jpeg ?? '' });
      cameraSent = !!jpeg;
    } catch {
      /* mejor esfuerzo: el siguiente cuadro lo reintenta */
    } finally {
      cameraBusy = false;
    }
  };
  const cameraTimer = setInterval(() => void pushCamera(), CAMERA_EVERY_MS);
  const heartbeat = setInterval(schedule, HEARTBEAT_MS);

  // ---- preferencias y estado ----
  const apply = () => {
    void applyRemote(remoteConfig(useStore.getState().prefs)).then(() => {
      lastJson = '';
      routeFor = Symbol('nuevo');
      sendLive();
      schedule();
      if (running() && routeTimer === undefined) routeTimer = setTimeout(pushRoute, 100);
    });
  };
  apply();
  const unsubscribe = useStore.subscribe((s, prev) => {
    const a = remoteConfig(s.prefs);
    const b = remoteConfig(prev.prefs);
    if (a.enabled !== b.enabled || a.port !== b.port || a.ip !== b.ip) return apply();
    if (!running()) return;
    schedule();
    if ((selectJobPreview(s)?.moves ?? null) !== routeFor && routeTimer === undefined) routeTimer = setTimeout(pushRoute, 600);
  });
  // Al cambiar el estado del servidor (arrancó, o entró el primer celular) se informa de inmediato.
  const unsubscribeRemote = useRemote.subscribe((r, prev) => {
    if (r.status?.running && !prev.status?.running) {
      schedule();
      if (routeTimer === undefined) routeTimer = setTimeout(pushRoute, 100);
      sendLive();
    }
    // Entró o salió un celular: la PC empieza o deja de seguir el trabajo.
    if (r.clients !== prev.clients || r.status?.running !== prev.status?.running) pokeLive();
  });

  return () => {
    stopped = true;
    removeConsumer();
    offLiveRoute();
    unsubscribe();
    unsubscribeRemote();
    clearTimeout(timer);
    clearTimeout(routeTimer);
    clearInterval(cameraTimer);
    clearInterval(heartbeat);
    offs.forEach((off) => off());
  };
}
