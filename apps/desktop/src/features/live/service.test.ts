import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { JobArgs, JobPreview, StatusReport } from '@/api/types';
import { FALLBACK_DEVICE, docToWork } from '@/lib/device';
import { createAppStore } from '@/store';
import { clearJobStart, noteJobStart } from './capture';
import { addLiveConsumer, LiveService, onLiveRoute, useLive, type LiveFrame } from './service';
import { buildLiveRoute, pointAt, posAtTime, type LiveRoute } from './route';
import { squareCut, rasterFill } from './testRoutes';

type Store = ReturnType<typeof createAppStore>;

let ms = 0;
let store: Store;
let svc: LiveService;
let stop: () => void;

const status = (state: StatusReport['state'], work: [number, number], spindle = 0): StatusReport => ({
  state,
  machine_pos: work,
  work_pos: work,
  feed: 600,
  spindle,
  buffer: { planner: 15, rx: 127 },
});

function setup(opts: { preview?: JobPreview; device?: Partial<typeof FALLBACK_DEVICE>; compute?: (a: JobArgs) => Promise<JobPreview> } = {}) {
  ms = 1000;
  store = createAppStore();
  const device = { ...FALLBACK_DEVICE, id: 'dev', ...opts.device };
  store.setState({ devices: [device], activeDeviceId: 'dev' });
  const preview = opts.preview ?? squareCut();
  store.setState({ preview, previewStatus: 'ready', previewStale: false });
  svc = new LiveService(store, { now: () => ms, compute: opts.compute });
  stop = svc.start();
  return { device, preview, route: buildLiveRoute(preview)! };
}

const s = () => store.getState();
const startJob = () => s().onJob({ state: 'started', message: '', position_lost: false });
const at = (route: LiveRoute, t: number): [number, number] => {
  const p = posAtTime(route, t);
  return pointAt(route, p.move, p.frac);
};
/** Pasa `dtMs` de reloj con la interfaz dibujando a ~60 cuadros por segundo. */
function tick(dtMs: number) {
  const end = ms + dtMs;
  while (ms + 16 <= end) {
    ms += 16;
    svc.frame(ms);
  }
  ms = end;
}
/** La máquina informa su posición en el instante de trabajo `t` y pasa `dtMs` de reloj. */
function report(route: LiveRoute, t: number, dtMs = 250, state: StatusReport['state'] = 'run', spindle = 0) {
  tick(dtMs);
  s().onStatus(status(state, at(route, t), spindle));
}
const frameNow = (): LiveFrame => {
  const f = svc.frame(ms);
  if (!f) throw new Error('sin cuadro');
  return f;
};

beforeEach(() => {
  clearJobStart();
  useLive.setState({ jobSeq: 0, route: 'none', source: null, fraction: 0, emitting: false, ended: null });
});
afterEach(() => {
  stop();
  // Los consumidores externos se registran a nivel de módulo: cada prueba quita los suyos.
});

describe('apagada no consume nada', () => {
  it('con la preferencia apagada y sin celulares ni ubica, ni arma la ruta, ni anima', () => {
    const { route } = setup();
    startJob();
    for (let t = 1; t < 10; t++) report(route, t);
    expect(svc.wanted()).toBe(false);
    expect(svc.route).toBeNull();
    expect(svc.frame(ms)).toBeNull();
    expect(svc.report()).toBeNull();
    expect(useLive.getState().route).toBe('none');
    expect(useLive.getState().fraction).toBe(0);
  });
});

describe('sigue la posición real de la máquina', () => {
  it('ubica la MPos en la ruta y el cabezal queda sobre ella', () => {
    const { route } = setup();
    s().setPref('liveView', true);
    startJob();
    report(route, 2);
    report(route, 2.5);
    report(route, 3);
    tick(150);
    const f = frameNow();
    // El dibujo sigue a la última posición reportada (la extrapola y suaviza) y el cabezal está sobre la ruta.
    expect(Math.abs(f.t - 3)).toBeLessThan(0.7);
    const [x, y] = at(route, f.t);
    expect(f.x).toBeCloseTo(x, 3);
    expect(f.y).toBeCloseTo(y, 3);
    expect(svc.report()).toMatchObject({ src: 'position', running: true, retreat_seq: 0 });
    expect(svc.report()!.t).toBeCloseTo(3, 1);
    expect(useLive.getState().source).toBe('position');
    expect(useLive.getState().route).toBe('ready');
  });

  it('interpola entre reportes con fluidez y nunca se adelanta mucho', () => {
    const { route } = setup();
    s().setPref('liveView', true);
    startJob();
    report(route, 2);
    report(route, 2.25);
    const seen: number[] = [];
    for (let i = 0; i < 12; i++) {
      tick(16);
      seen.push(frameNow().t);
    }
    expect(Math.min(...seen.slice(1).map((v, i) => v - seen[i]!))).toBeGreaterThanOrEqual(0);
    expect(seen.at(-1)!).toBeLessThan(2.25 + 0.9);
  });

  it('transforma el reporte según el origen del perfil', () => {
    const { route, device } = setup({ device: { origin: 'top_right', bed: [400, 300] } });
    s().setPref('liveView', true);
    startJob();
    const t = route.total * 0.5;
    ms += 250;
    s().onStatus(status('run', docToWork(device, at(route, t))));
    expect(svc.report()!.src).toBe('position');
    expect(svc.report()!.t).toBeCloseTo(t, 1);
  });

  it('con una posición que no encaja usa el progreso del streamer', () => {
    const { route } = setup();
    s().setPref('liveView', true);
    startJob();
    s().onProgress({ acked: 50, total: 100 });
    for (let i = 0; i < 5; i++) {
      ms += 250;
      s().onStatus(status('run', [900, 900]));
    }
    expect(svc.report()!.src).toBe('progress');
    expect(svc.report()!.t).toBeCloseTo(route.total * 0.5, 1);
  });

  it('el K40 y el galvo (sin posición mientras trabajan) usan el reloj y lo dicen', () => {
    const { route } = setup({ device: { kind: 'lhy_micro' } });
    s().setPref('liveView', true);
    startJob();
    ms += 250;
    s().onStatus(status('run', [0, 0]));
    expect(svc.report()!.src).toBe('clock');
    expect(route.total).toBeGreaterThan(0);
    expect(useLive.getState().source).toBe('clock');
  });
});

describe('vista activada a mitad del trabajo', () => {
  it('se pone al día con lo ya hecho en el siguiente reporte', () => {
    const { route } = setup();
    startJob();
    s().onProgress({ acked: 60, total: 100 });
    for (let t = 1; t < 14; t++) report(route, t);
    expect(svc.frame(ms)).toBeNull();
    s().setPref('liveView', true);
    report(route, 14);
    const f = frameNow();
    expect(f.t).toBeCloseTo(14, 1);
    expect(f.move).toBe(posAtTime(route, 14).move);
    expect(useLive.getState().fraction).toBeCloseTo(14 / route.total, 2);
  });

  it('si el trabajo pasa por el mismo punto varias veces, el progreso del streamer elige la vuelta', () => {
    const preview = rasterFill(30, 20, 0.1);
    const { route } = setup({ preview });
    startJob();
    s().onProgress({ acked: 90, total: 100 });
    s().setPref('liveView', true);
    // Cualquier punto de una fila se repite en las demás pasadas con otro Y; usar uno de los últimos renglones.
    report(route, route.total * 0.9);
    expect(svc.report()!.t).toBeGreaterThan(route.total * 0.8);
  });
});

describe('pausa, cancelado y fin', () => {
  it('en pausa (Hold) la animación se detiene', () => {
    const { route } = setup();
    s().setPref('liveView', true);
    startJob();
    for (let t = 1; t <= 4; t += 0.5) report(route, t);
    s().onJob({ state: 'paused', message: '', position_lost: false });
    report(route, 4.2, 250, 'hold');
    tick(1500);
    const a = frameNow();
    tick(1500);
    const b = frameNow();
    expect(a.mode).toBe('hold');
    expect(b.t).toBe(a.t);
    expect(a.emitting).toBe(false);
    expect(svc.report()!.running).toBe(false);
  });

  it('cancelado: queda congelada donde quedó', () => {
    const { route } = setup();
    s().setPref('liveView', true);
    startJob();
    for (let t = 1; t <= 5; t += 0.5) report(route, t);
    tick(100);
    const before = frameNow().t;
    s().onJob({ state: 'cancelled', message: '', position_lost: false });
    tick(3000);
    expect(frameNow().t).toBeCloseTo(before, 6);
    report(route, 9); // un reporte tardío no la mueve
    expect(frameNow().mode).toBe('frozen');
    expect(frameNow().t).toBeCloseTo(before, 6);
    expect(useLive.getState().ended).toBe('cancelled');
  });

  it('terminado: llega al 100 % y el cabezal acaba en la última posición de la ruta', () => {
    const { route } = setup();
    s().setPref('liveView', true);
    startJob();
    for (let t = 1; t < route.total; t += 1) report(route, t);
    s().onJob({ state: 'finished', message: '', position_lost: false });
    tick(50);
    const f = frameNow();
    const last = route.model.pos;
    expect(f.fraction).toBe(1);
    expect(f.x).toBeCloseTo(last[2 * route.count]!, 3);
    expect(f.y).toBeCloseTo(last[2 * route.count + 1]!, 3);
    expect(useLive.getState().fraction).toBe(1);
    expect(useLive.getState().ended).toBe('finished');
  });

  it('con una alarma el trabajo se da por fallido y queda congelada', () => {
    const { route } = setup();
    s().setPref('liveView', true);
    startJob();
    for (let t = 1; t <= 3; t += 0.5) report(route, t);
    report(route, 3.2, 250, 'alarm');
    expect(useLive.getState().ended).toBe('error');
    expect(frameNow().mode).toBe('frozen');
  });

  it('editar el diseño después de terminar la deja sin sentido y se descarta', () => {
    const { route } = setup();
    s().setPref('liveView', true);
    startJob();
    report(route, 2);
    s().onJob({ state: 'finished', message: '', position_lost: false });
    expect(svc.active).toBe(true);
    store.setState({ doc: { ...s().doc, objects: [] } });
    expect(svc.active).toBe(false);
    expect(useLive.getState().jobSeq).toBe(0);
  });
});

describe('con pestañas', () => {
  it('cambiar de pestaña o editar otra no descarta el trabajo terminado; editar la pestaña de origen sí', () => {
    const { route, preview } = setup();
    const origin = s().activeTabId;
    s().setPref('liveView', true);
    s().noteJobOrigin({ tabId: origin, name: 'a', path: null, preview });
    startJob();
    report(route, 2);
    s().onJob({ state: 'finished', message: '', position_lost: false });
    expect(svc.active).toBe(true);
    s().newTab(); // otra pestaña: cambia el documento visible, pero no se editó nada
    expect(svc.active).toBe(true);
    store.setState({ doc: { ...s().doc, layers: [] } }); // editar la otra pestaña
    s().applyImport({ objects: [], new_layers: [], warnings: [] });
    expect(svc.active).toBe(true);
    s().activateTab(origin);
    expect(svc.active).toBe(true);
    store.setState({ doc: { ...s().doc, objects: [] } }); // ahora sí: editar el diseño del trabajo
    expect(svc.active).toBe(false);
  });

  it('la ruta sale de la vista previa con la que se lanzó, aunque al llegar `started` haya otra pestaña activa', () => {
    const { preview } = setup();
    s().setPref('liveView', true);
    const origin = s().activeTabId;
    s().noteJobOrigin({ tabId: origin, name: 'a', path: null, preview });
    s().newTab(); // el usuario cambia de pestaña antes de que la máquina avise que arrancó
    expect(s().previewStatus).toBe('idle');
    startJob();
    expect(svc.route).not.toBeNull();
    expect(svc.route!.total).toBeCloseTo(buildLiveRoute(preview)!.total, 6);
  });

  it('sin pestaña de origen conocida (celular, reanudación) vale la vista previa de la activa, como antes', () => {
    const { preview } = setup();
    s().setPref('liveView', true);
    expect(s().jobOrigin).toBeNull();
    startJob();
    expect(svc.route!.total).toBeCloseTo(buildLiveRoute(preview)!.total, 6);
  });
});

describe('vista encendida después de terminar', () => {
  it('muestra el trabajo terminado al 100 % con el cabezal al final', () => {
    const { route } = setup();
    startJob();
    for (let t = 1; t < 10; t++) report(route, t);
    s().onJob({ state: 'finished', message: '', position_lost: false });
    expect(svc.frame(ms)).toBeNull(); // apagada: nada
    s().setPref('liveView', true);
    svc.poke();
    const f = svc.frame(ms + 16)!;
    expect(f.fraction).toBe(1);
    expect(f.mode).toBe('done');
    expect(f.x).toBeCloseTo(route.model.pos[2 * route.count]!, 3);
  });
});

describe('retroceso real de la máquina', () => {
  it('si la máquina vuelve atrás de verdad se avisa (para el celular también)', () => {
    const { route } = setup();
    s().setPref('liveView', true);
    startJob();
    report(route, 20);
    report(route, 21);
    for (let i = 0; i < 4; i++) report(route, 2, 400);
    expect(svc.report()!.retreat_seq).toBe(1);
    expect(frameNow().t).toBeLessThan(5);
  });
});

describe('qué láser está emitiendo', () => {
  it('enciende en un movimiento de corte en marcha y no en un desplazamiento', () => {
    const { route } = setup();
    s().setPref('liveView', true);
    startJob();
    report(route, route.model.times[0]! / 2, 250, 'run', 0);
    expect(frameNow().emitting).toBe(false); // el viaje
    report(route, route.model.times[1]! - 1, 250, 'run', 800);
    report(route, route.model.times[1]! - 0.9, 250, 'run', 800);
    expect(frameNow().emitting).toBe(true);
  });
});

describe('de dónde sale la ruta', () => {
  it('con argumentos ajustados (tiling) calcula la vista previa de esos argumentos y no usa la en caché', async () => {
    const other = rasterFill(20, 20, 0.2);
    const compute = vi.fn().mockResolvedValue(other);
    setup({ compute });
    const args = { doc: s().doc, device: s().devices[0]!, options: s().jobOptions } as JobArgs;
    noteJobStart(args, true);
    s().setPref('liveView', true);
    startJob();
    await vi.waitFor(() => expect(useLive.getState().route).toBe('ready'));
    expect(compute).toHaveBeenCalledWith(args);
    expect(svc.route!.count).toBe(buildLiveRoute(other)!.count);
  });

  it('sin vista previa al día ni argumentos, la vista no está disponible', () => {
    setup();
    store.setState({ previewStale: true });
    s().setPref('liveView', true);
    startJob();
    expect(useLive.getState().route).toBe('unavailable');
  });

  it('desde la posición actual no se fía de la vista previa en caché: la calcula con los argumentos', async () => {
    const fresh = squareCut(40);
    const compute = vi.fn().mockResolvedValue(fresh);
    setup({ compute });
    const args = { doc: s().doc, device: s().devices[0]!, options: { ...s().jobOptions, start_from: 'current_position' } } as JobArgs;
    noteJobStart(args, false);
    s().setPref('liveView', true);
    startJob();
    await vi.waitFor(() => expect(compute).toHaveBeenCalled());
  });

  it('avisa a quien escucha cuando llega la ruta y cuando empieza otro trabajo', () => {
    setup();
    const seen: (LiveRoute | null)[] = [];
    const off = onLiveRoute((r) => seen.push(r));
    s().setPref('liveView', true);
    startJob();
    expect(seen.at(-1)).not.toBeNull();
    s().onJob({ state: 'finished', message: '', position_lost: false });
    startJob();
    expect(seen).toContain(null);
    off();
  });
});

describe('celulares conectados', () => {
  it('un consumidor externo (celular) hace que se siga el trabajo aunque la preferencia esté apagada', () => {
    const { route } = setup();
    let phone = false;
    const off = addLiveConsumer(() => phone);
    startJob();
    report(route, 3);
    expect(svc.report()).toBeNull();
    phone = true;
    svc.poke();
    expect(svc.wanted()).toBe(true);
    report(route, 4);
    expect(svc.report()!.t).toBeCloseTo(4, 1);
    off();
  });
});
