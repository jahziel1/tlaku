// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { EventMap, StatusReport } from '@/api/types';
import { setLanguage } from '@/i18n';
import { FALLBACK_DEVICE } from '@/lib/device';
import { useStore } from '@/store';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { clearJobStart } from './capture';
import { DetachedLive } from './DetachedLive';
import { closeDetached, openDetached, startLiveBridge, useLiveWindow } from './detach';
import { fakeContext } from './fakeCanvas';
import { buildLiveRoute, pointAt, posAtTime } from './route';
import { startLive, useLive } from './service';
import { EMPTY_SNAPSHOT } from './source';
import { squareCut } from './testRoutes';
import { decodeRoute, decodeState, encodeRoute, encodeState } from './wire';

const s = () => useStore.getState();
const preview = squareCut();
const route = buildLiveRoute(preview)!;
const at = (t: number): [number, number] => {
  const p = posAtTime(route, t);
  return pointAt(route, p.move, p.frac);
};
const status = (t: number): StatusReport => ({ state: 'run', machine_pos: at(t), work_pos: at(t), feed: 600, spindle: 0, buffer: { planner: 15, rx: 127 } });
const flush = () => act(async () => new Promise((r) => setTimeout(r, 40)));

type Handler = (p: never) => void;
interface Fake {
  sent: { kind: string; data: string }[];
  handlers: Map<string, Set<Handler>>;
  calls: string[];
  emit: <K extends keyof EventMap>(event: K, payload: EventMap[K]) => void;
}

/** API de mentira que anota lo que la ventana principal le manda a la ventana aparte y deja disparar sus eventos. */
function fakeApi(): Fake {
  const f: Fake = { sent: [], handlers: new Map(), calls: [], emit: (event, payload) => f.handlers.get(event)?.forEach((h) => h(payload as never)) };
  const api = createMockApi({ latencyMs: 0 });
  Object.assign(api, {
    liveWindowOpen: async () => void f.calls.push('open'),
    liveWindowClose: async () => void f.calls.push('close'),
    liveWindowHello: async () => void f.calls.push('hello'),
    liveWindowSend: async (a: { kind: string; data: string }) => {
      f.sent.push(a);
      return true;
    },
    on: async (event: string, handler: Handler) => {
      let set = f.handlers.get(event);
      if (!set) f.handlers.set(event, (set = new Set()));
      set.add(handler);
      return () => void set.delete(handler);
    },
  });
  setApi(api);
  return f;
}

let stop: () => void;
let stopBridge: () => void = () => undefined;
let fake: Fake;

beforeEach(() => {
  setLanguage('es');
  clearJobStart();
  const { ctx } = fakeContext();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(ctx as never);
  fake = fakeApi();
  useStore.setState({ devices: [{ ...FALLBACK_DEVICE, id: 'dev' }], activeDeviceId: 'dev', preview, previewStatus: 'ready', previewStale: false, jobState: 'idle', status: null, progress: null, prefs: { ...s().prefs, liveView: true, liveWindow: null } });
  useLive.setState({ jobSeq: 0, route: 'none', source: null, fraction: 0, emitting: false, ended: null });
  useLiveWindow.setState({ detached: false });
  stop = startLive(useStore);
});
afterEach(async () => {
  stopBridge();
  stopBridge = () => undefined;
  stop();
  await cleanup();
  vi.restoreAllMocks();
});

const begin = () => act(async () => s().onJob({ state: 'started', message: '', position_lost: false }));

describe('la ventana principal le manda todo a la ventana aparte', () => {
  it('al abrirla manda la ruta una vez y el estado en cada cambio; cerrarla devuelve la flotante', async () => {
    stopBridge = startLiveBridge();
    await begin();
    await act(async () => s().onStatus(status(2)));
    await flush();
    // Abierta la ventana (todavía no escucha nada: no se manda).
    expect(fake.sent).toHaveLength(0);
    await act(async () => openDetached());
    await flush();
    expect(fake.calls).toContain('open');
    expect(useLiveWindow.getState().detached).toBe(true);
    const routes = fake.sent.filter((m) => m.kind === 'route');
    const states = fake.sent.filter((m) => m.kind === 'state');
    expect(routes).toHaveLength(1);
    expect(decodeRoute(routes[0]!.data)!.route.count).toBe(route.count);
    const first = decodeState(states.at(-1)!.data)!;
    expect(first.snap.jobSeq).toBe(1);
    expect(first.snap.route).toBe('ready');
    expect(first.reading).toMatchObject({ jobSeq: 1, mode: 'running' });
    // Más reportes: más estados, la ruta no se repite.
    for (const t of [3, 4, 5]) {
      await act(async () => s().onStatus(status(t)));
      await flush();
    }
    expect(fake.sent.filter((m) => m.kind === 'route')).toHaveLength(1);
    const last = decodeState(fake.sent.filter((m) => m.kind === 'state').at(-1)!.data)!;
    expect(last.reading!.t).toBeGreaterThan(first.reading!.t);
    expect(last.snap.fraction).toBeGreaterThan(first.snap.fraction);
    // La ventana aparte pide todo de nuevo (se recargó): llega otra vez la ruta.
    await act(async () => fake.emit('live://ready', null));
    await flush();
    expect(fake.sent.filter((m) => m.kind === 'route')).toHaveLength(2);
    // Se cerró: vuelve la flotante y ya no se manda nada.
    await act(async () => fake.emit('live://closed', null));
    expect(useLiveWindow.getState().detached).toBe(false);
    const n = fake.sent.length;
    await act(async () => s().onStatus(status(6)));
    await flush();
    expect(fake.sent).toHaveLength(n);
  });

  it('un trabajo nuevo manda su propia ruta; el estado lleva el tema y el idioma de la principal', async () => {
    stopBridge = startLiveBridge();
    await begin();
    await act(async () => s().onStatus(status(2)));
    await act(async () => openDetached());
    await flush();
    await act(async () => s().onJob({ state: 'finished', message: '', position_lost: false }));
    await begin();
    await act(async () => s().onStatus(status(1)));
    await flush();
    const routes = fake.sent.filter((m) => m.kind === 'route').map((m) => decodeRoute(m.data)!.seq);
    expect(routes).toEqual([1, 2]);
    const st = decodeState(fake.sent.filter((m) => m.kind === 'state').at(-1)!.data)!;
    expect(st.snap).toMatchObject({ jobSeq: 2, theme: s().prefs.theme, lang: s().prefs.language });
  });

  it('apagar la vista cierra la ventana aparte', async () => {
    stopBridge = startLiveBridge();
    await act(async () => openDetached());
    await act(async () => s().setPref('liveView', false));
    expect(useLiveWindow.getState().detached).toBe(false);
    expect(fake.calls).toContain('close');
  });

  it('cerrarla por el botón la cierra de verdad', async () => {
    await act(async () => openDetached());
    closeDetached();
    expect(useLiveWindow.getState().detached).toBe(false);
    expect(fake.calls.filter((c) => c === 'close')).toHaveLength(1);
  });

  it('sin la ventana abierta no se manda nada', async () => {
    stopBridge = startLiveBridge();
    await begin();
    await act(async () => s().onStatus(status(2)));
    await flush();
    expect(fake.sent).toHaveLength(0);
  });
});

describe('DetachedLive: la ventana aparte', () => {
  it('al abrirse escucha y pide el estado (hello); dibuja lo que le mandan; vuelve a la flotante con el botón', async () => {
    await mount(<DetachedLive />);
    await flush();
    expect(fake.calls).toContain('hello');
    expect(fake.handlers.get('live://route')?.size).toBe(1);
    expect(fake.handlers.get('live://state')?.size).toBe(1);
    // No trae la interfaz de la app.
    expect(document.querySelector('.shell, .floatwin, #canvas-main')).toBeNull();
    const snap = { ...EMPTY_SNAPSHOT, jobSeq: 1, route: 'ready' as const, job: 'running' as const, fraction: 0.42, total: route.total, bed: [300, 200] as [number, number], theme: 'light' as const, lang: 'en' as const };
    await act(async () => {
      fake.emit('live://route', encodeRoute(1, route)!);
      fake.emit('live://state', encodeState(snap, { jobSeq: 1, t: 4, mode: 'running', retreatSeq: 0, spindle: 0 }));
    });
    await flush();
    const canvas = document.querySelector<HTMLCanvasElement>('canvas.live-canvas')!;
    expect(canvas.getAttribute('aria-label')).toMatch(/^Live view of the job: 42%, running\./);
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(document.documentElement.lang).toBe('en');
    expect(document.querySelector('.live-foot')!.textContent).toContain('42%');
    const res = await audit(document.body);
    expect(describeViolations(res)).toEqual([]);
    await click(document.querySelector('.live-ext__back')!);
    expect(fake.calls).toContain('close');
    setLanguage('es');
  });

  it('ignora mensajes inválidos y se queda esperando', async () => {
    await mount(<DetachedLive />);
    await flush();
    await act(async () => {
      fake.emit('live://route', 'basura');
      fake.emit('live://state', '{"v":9}');
    });
    expect(document.querySelector('.live-foot')!.textContent).toContain('Inicia un trabajo');
  });

  it('al desmontarse deja de escuchar', async () => {
    const m = await mount(<DetachedLive />);
    await flush();
    await m.unmount();
    expect(fake.handlers.get('live://route')?.size).toBe(0);
    expect(fake.handlers.get('live://state')?.size).toBe(0);
  });
});
