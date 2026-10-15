import type { EventMap, EventName } from '../types';
import type { LaserApi } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;
type LiveEvent = Extract<EventName, `live://${string}`>;

interface Message {
  /** Quién lo recibe: la ventana que abrió la vista (`owner`) o la ventana aparte (`live`). */
  to: 'owner' | 'live';
  event: LiveEvent | 'close';
  payload: unknown;
}

const CHANNEL = 'tlaku-live-window';

/** ¿Esta pestaña hace de ventana aparte de la vista en vivo? (en el navegador: `?liveWindow=1`). */
export function isMockLivePage(): boolean {
  return typeof location !== 'undefined' && new URLSearchParams(location.search).has('liveWindow');
}

/**
 * Ventana aparte de la vista en vivo en el modo simulado: una pestaña emergente (`?liveWindow=1`) y un
 * `BroadcastChannel` hacen de los eventos de Tauri. Sirve para ver y probar el flujo en el navegador;
 * en la app real lo hacen `live_window_*` (Rust) y los eventos `live://*`.
 */
export function mockLiveWindow(lag: Lag): Pick<LaserApi, 'liveWindowOpen' | 'liveWindowClose' | 'liveWindowSend' | 'liveWindowHello'> & {
  onLive: <K extends LiveEvent>(event: K, handler: (payload: EventMap[K]) => void) => () => void;
} {
  const channel = typeof BroadcastChannel === 'undefined' ? null : new BroadcastChannel(CHANNEL);
  const me: Message['to'] = isMockLivePage() ? 'live' : 'owner';
  let popup: Window | null = null;
  const handlers = new Map<string, Set<(p: never) => void>>();
  const post = (m: Message) => channel?.postMessage(m);
  if (channel) {
    channel.onmessage = (e: MessageEvent<Message>) => {
      const m = e.data;
      if (m.to !== me) return;
      if (m.event === 'close') {
        if (typeof window !== 'undefined') window.close();
        return;
      }
      handlers.get(m.event)?.forEach((h) => h(m.payload as never));
    };
  }
  if (me === 'live' && typeof window !== 'undefined') window.addEventListener('pagehide', () => post({ to: 'owner', event: 'live://closed', payload: null }));
  return {
    liveWindowOpen: () =>
      lag(() => {
        if (popup && !popup.closed) {
          popup.focus();
          return;
        }
        if (typeof window === 'undefined' || typeof window.open !== 'function') return;
        popup = window.open(`${location.pathname}?liveWindow=1`, 'tlaku-live-window', 'popup=yes,width=760,height=560');
      }),
    liveWindowClose: () =>
      lag(() => {
        // La propia pestaña aparte se cierra sola; la principal le pide que se cierre.
        if (me === 'live') {
          if (typeof window !== 'undefined') window.close();
          return;
        }
        post({ to: 'live', event: 'close', payload: null });
        popup?.close();
        popup = null;
      }),
    liveWindowSend: ({ kind, data }) =>
      lag(() => {
        post({ to: 'live', event: `live://${kind}`, payload: data });
        return !!popup && !popup.closed;
      }),
    liveWindowHello: () =>
      lag(() => {
        post({ to: 'owner', event: 'live://ready', payload: null });
      }),
    onLive: (event, handler) => {
      let set = handlers.get(event);
      if (!set) handlers.set(event, (set = new Set()));
      set.add(handler as (p: never) => void);
      return () => set.delete(handler as (p: never) => void);
    },
  };
}
