import { create } from 'zustand';
import { getApi } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';
import { localSource } from './localSource';
import { getLiveService } from './service';
import type { LiveSource } from './source';
import { encodeRoute, encodeState } from './wire';

/**
 * Ventana aparte de la vista en vivo, vista desde la ventana PRINCIPAL: abrirla, cerrarla y mandarle
 * la ruta y el estado mientras está abierta. La ventana aparte (`DetachedLive.tsx`) solo escucha.
 */
export const useLiveWindow = create<{ detached: boolean }>()(() => ({ detached: false }));

/** Abre la ventana aparte; la flotante se oculta mientras esté abierta. */
export async function openDetached(): Promise<void> {
  try {
    await getApi().liveWindowOpen();
    useLiveWindow.setState({ detached: true });
  } catch (e) {
    useStore.getState().notify('error', t('live.ext.failed', { error: errorMessage(e) }));
  }
}

/** Cierra la ventana aparte (y vuelve la flotante si la vista sigue encendida). */
export function closeDetached(): void {
  useLiveWindow.setState({ detached: false });
  getApi()
    .liveWindowClose()
    .catch(() => undefined);
}

/**
 * Mientras la ventana aparte está abierta, le manda la ruta (una vez por trabajo) y el estado (en cada
 * cambio, juntando los que llegan seguidos). Atiende `live://ready` (la ventana nació o se recargó y
 * pide todo de nuevo) y `live://closed` (se cerró: vuelve la flotante). Devuelve cómo pararlo.
 */
export function startLiveBridge(source: LiveSource = localSource()): () => void {
  const api = getApi();
  let alive = true;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let routeSent = -1;
  let tooBig = -1;
  const unlisten: (() => void)[] = [];

  const send = (kind: 'route' | 'state', data: string) => {
    api.liveWindowSend({ kind, data }).catch(() => undefined);
  };
  const flush = () => {
    timer = null;
    if (!alive || !useLiveWindow.getState().detached) return;
    const snap = source.snapshot();
    let shown = snap;
    if (snap.route === 'ready') {
      const route = source.route();
      if (route && routeSent !== snap.jobSeq) {
        const text = encodeRoute(snap.jobSeq, route);
        if (text) {
          send('route', text);
          routeSent = snap.jobSeq;
        } else tooBig = snap.jobSeq;
      }
      // Una ruta demasiado grande no se manda: la ventana aparte lo dice en lugar de esperar para siempre.
      if (tooBig === snap.jobSeq) shown = { ...snap, route: 'unavailable' };
    }
    send('state', encodeState(shown, getLiveService()?.reading() ?? null));
  };
  const schedule = () => {
    if (timer === null && useLiveWindow.getState().detached) timer = setTimeout(flush, 0);
  };

  const offSource = source.subscribe(schedule);
  const offWindow = useLiveWindow.subscribe((s, prev) => {
    if (s.detached && !prev.detached) {
      routeSent = -1;
      schedule();
    }
  });
  // Apagar la vista en vivo cierra también la ventana aparte.
  const offPref = useStore.subscribe((s, prev) => {
    if (prev.prefs.liveView && !s.prefs.liveView && useLiveWindow.getState().detached) closeDetached();
  });
  void Promise.all([
    api.on('live://ready', () => {
      routeSent = -1;
      schedule();
    }),
    api.on('live://closed', () => useLiveWindow.setState({ detached: false })),
  ]).then((offs) => {
    if (alive) unlisten.push(...offs);
    else offs.forEach((f) => f());
  });
  return () => {
    alive = false;
    if (timer !== null) clearTimeout(timer);
    offSource();
    offWindow();
    offPref();
    unlisten.forEach((f) => f());
  };
}
