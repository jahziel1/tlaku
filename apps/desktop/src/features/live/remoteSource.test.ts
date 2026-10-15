import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { StatusReport } from '@/api/types';
import { FALLBACK_DEVICE } from '@/lib/device';
import { createAppStore } from '@/store';
import { clearJobStart } from './capture';
import { RemoteLiveSource } from './remoteSource';
import { buildLiveRoute, pointAt, posAtTime, type LiveRoute } from './route';
import { LiveService, useLive } from './service';
import { EMPTY_SNAPSHOT, sameSnapshot, type LiveSnapshot } from './source';
import { squareCut } from './testRoutes';
import { encodeRoute, encodeState } from './wire';

/**
 * La ventana aparte arma su propio cabezal con la ruta y los reportes que le manda la principal. Aquí se
 * corre un trabajo en el servicio de la principal y se lo «manda» a una fuente remota con el mismo reloj:
 * las dos deben dibujar lo mismo.
 */

type Store = ReturnType<typeof createAppStore>;
let ms = 0;
let store: Store;
let svc: LiveService;
let stop: () => void;
let remote: RemoteLiveSource;
let route: LiveRoute;

const at = (t: number): [number, number] => {
  const p = posAtTime(route, t);
  return pointAt(route, p.move, p.frac);
};
const status = (t: number): StatusReport => ({ state: 'run', machine_pos: at(t), work_pos: at(t), feed: 600, spindle: 0, buffer: { planner: 15, rx: 127 } });
const snap = (patch: Partial<LiveSnapshot> = {}): LiveSnapshot => ({ ...EMPTY_SNAPSHOT, jobSeq: 1, route: 'ready', job: 'running', total: route.total, ...patch });

/** Lo que manda la principal: la foto y el último reporte. */
const push = (patch: Partial<LiveSnapshot> = {}) => remote.applyState(encodeState(snap(patch), svc.reading()));

beforeEach(() => {
  ms = 1000;
  clearJobStart();
  useLive.setState({ jobSeq: 0, route: 'none', source: null, fraction: 0, emitting: false, ended: null });
  store = createAppStore();
  store.setState({ devices: [{ ...FALLBACK_DEVICE, id: 'dev' }], activeDeviceId: 'dev', preview: squareCut(), previewStatus: 'ready', previewStale: false });
  store.getState().setPref('liveView', true);
  svc = new LiveService(store, { now: () => ms });
  stop = svc.start();
  route = buildLiveRoute(squareCut())!;
  remote = new RemoteLiveSource(() => ms);
});
afterEach(() => stop());

const tick = (dt: number) => {
  ms += dt;
};

describe('fuente de la ventana aparte', () => {
  it('sin ruta dice «calculando» aunque la principal ya la tenga lista, y al llegar se pone lista', () => {
    store.getState().onJob({ state: 'started', message: '', position_lost: false });
    tick(250);
    store.getState().onStatus(status(2));
    expect(remote.applyState(encodeState(snap({ route: 'ready' }), svc.reading()))).toBe(true);
    expect(remote.snapshot().route).toBe('computing');
    expect(remote.route()).toBeNull();
    expect(remote.frame()).toBeNull();
    expect(remote.applyRoute(encodeRoute(1, svc.route!)!)).toBe(true);
    expect(remote.snapshot().route).toBe('ready');
    expect(remote.route()).not.toBeNull();
    expect(remote.frame()).not.toBeNull();
  });

  it('el cabezal de la ventana aparte coincide con el de la principal mientras corre el trabajo', () => {
    store.getState().onJob({ state: 'started', message: '', position_lost: false });
    tick(250);
    store.getState().onStatus(status(1));
    remote.applyRoute(encodeRoute(1, svc.route!)!);
    push();
    let worst = 0;
    for (let t = 1.5; t < 9; t += 0.5) {
      tick(250);
      store.getState().onStatus(status(t));
      push();
      // Entre reporte y reporte se pide un cuadro cada ~16 ms, como el bucle de dibujo.
      for (let k = 0; k < 15; k++) {
        tick(16);
        const a = svc.frame(ms)!;
        const b = remote.frame()!;
        worst = Math.max(worst, Math.hypot(a.x - b.x, a.y - b.y), Math.abs(a.t - b.t));
        expect(b.move).toBeGreaterThanOrEqual(0);
      }
    }
    // La ventana aparte recibe los reportes con un instante de retraso respecto de las dos: sin diferencia que se note (< 0,3 mm).
    expect(worst).toBeLessThan(0.3);
  });

  it('terminado: llega al final de la ruta; detenido: queda congelado', () => {
    store.getState().onJob({ state: 'started', message: '', position_lost: false });
    tick(250);
    store.getState().onStatus(status(3));
    remote.applyRoute(encodeRoute(1, svc.route!)!);
    push();
    tick(300);
    store.getState().onJob({ state: 'finished', message: '', position_lost: false });
    push({ job: 'finished', ended: 'finished', fraction: 1 });
    const f = remote.frame()!;
    expect(f.mode).toBe('done');
    expect(f.t).toBeCloseTo(route.total, 3);
    expect(remote.snapshot().fraction).toBe(1);
  });

  it('un trabajo nuevo descarta la ruta anterior', () => {
    store.getState().onJob({ state: 'started', message: '', position_lost: false });
    tick(250);
    store.getState().onStatus(status(2));
    remote.applyRoute(encodeRoute(1, svc.route!)!);
    push();
    expect(remote.route()).not.toBeNull();
    remote.applyState(encodeState(snap({ jobSeq: 2, route: 'computing' }), null));
    expect(remote.route()).toBeNull();
    expect(remote.snapshot().route).toBe('computing');
    // Un reporte del trabajo anterior que llegue tarde no mueve nada.
    remote.applyState(encodeState(snap({ jobSeq: 2, route: 'ready' }), { jobSeq: 1, t: 5, mode: 'running', retreatSeq: 0, spindle: 0 }));
    expect(remote.frame()).toBeNull();
  });

  it('un reporte que llega antes que la ruta se aplica cuando llega la ruta', () => {
    store.getState().onJob({ state: 'started', message: '', position_lost: false });
    tick(250);
    store.getState().onStatus(status(4));
    push();
    expect(remote.frame()).toBeNull();
    remote.applyRoute(encodeRoute(1, svc.route!)!);
    const f = remote.frame()!;
    expect(f).not.toBeNull();
    expect(f.t).toBeGreaterThan(3);
  });

  it('los mensajes inválidos se rechazan y no cambian nada', () => {
    expect(remote.applyRoute('basura')).toBe(false);
    expect(remote.applyState('{"v":1}')).toBe(false);
    expect(remote.snapshot()).toBe(EMPTY_SNAPSHOT);
  });

  it('la foto no cambia de identidad mientras diga lo mismo (para no pintar de más) y avisa a quien escucha', () => {
    const seen: number[] = [];
    const off = remote.subscribe(() => seen.push(1));
    remote.applyState(encodeState(snap({ route: 'computing' }), null));
    const first = remote.snapshot();
    remote.applyState(encodeState(snap({ route: 'computing' }), null));
    expect(remote.snapshot()).toBe(first);
    expect(sameSnapshot(first, remote.snapshot())).toBe(true);
    expect(seen).toHaveLength(2);
    off();
    remote.applyState(encodeState(snap({ route: 'computing', fraction: 0.5 }), null));
    expect(seen).toHaveLength(2);
  });
});
