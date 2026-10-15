import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { JSDOM } from 'jsdom';
import { describe, expect, it } from 'vitest';
import { fakeContext, type Calls } from './fakeCanvas';
import { buildLivePayload } from './remoteRoute';
import { buildLiveRoute } from './route';
import { LiveTracker } from './tracker';
import { squareCut } from './testRoutes';

/**
 * La página del celular (`src-tauri/src/remote/page.html`) con su script de verdad dentro de jsdom,
 * con `fetch`, `EventSource` y el lienzo simulados.
 */

const HTML = readFileSync(join(__dirname, '../../../src-tauri/src/remote/page.html'), 'utf8');

describe('el seguimiento del cabezal del celular es el mismo de la PC', () => {
  const block = HTML.slice(HTML.indexOf('/* <live-core> */'), HTML.indexOf('/* </live-core> */'));
  const Js = new Function(`${block}; return LiveTracker;`)() as new (total: number) => {
    t: number;
    rate: number;
    mode: string;
    tLoc: number;
    report(r: { t: number; running: boolean; retreat?: boolean }, wall: number): void;
    step(wall: number): number;
    freeze(): void;
    finish(): void;
  };

  it('dan lo mismo en miles de pasos al azar (informes, pausas, retrocesos, congelar, terminar)', () => {
    let seed = 12345;
    const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32);
    for (let run = 0; run < 40; run++) {
      const total = 50 + rnd() * 500;
      const a = new LiveTracker(total);
      const b = new Js(total);
      let wall = rnd() * 10;
      let truth = rnd() * total * 0.5;
      const speed = 0.3 + rnd() * 3;
      for (let i = 0; i < 400; i++) {
        wall += 1 / 60;
        truth += speed / 60;
        const dice = rnd();
        if (dice < 0.06) {
          const r = { t: Math.max(0, truth - rnd() * 0.3 - (rnd() < 0.1 ? rnd() * 40 : 0)), running: rnd() > 0.15, retreat: rnd() < 0.03 };
          a.report(r, wall);
          b.report(r, wall);
        } else if (dice < 0.062) {
          a.freeze();
          b.freeze();
        } else if (dice < 0.063) {
          a.finish();
          b.finish();
        }
        const ta = a.step(wall);
        const tb = b.step(wall);
        expect(tb).toBeCloseTo(ta, 10);
        expect(b.mode).toBe(a.mode);
        expect(b.rate).toBeCloseTo(a.rate, 10);
      }
    }
  });
});

// ---------------------------------------------------------------------------------------------

const route = buildLiveRoute(squareCut(100))!;
const payload = buildLivePayload(route, [400, 300])!;
const ROUTE = { version: 1, ...payload };

interface Page {
  win: Window & typeof globalThis;
  doc: Document;
  fetched: string[];
  es: { emit: (state: unknown) => void };
  /** Cuentas de dibujo por lienzo. */
  calls: Map<Element, Calls>;
  rafs: (() => void)[];
  setNow: (ms: number) => void;
  flush: (ms: number) => void;
}

function state(over: Record<string, unknown> = {}) {
  return {
    v: 1,
    pc_online: true,
    machine: { connected: true, kind: 'grbl', state: 'run' },
    device: 'Falcon',
    doc: 'prueba.tlaku',
    job: { phase: 'running', message: '', progress: 0.4, elapsed_s: 10, remaining_s: 20, total_s: 40, layer: null, finished_seq: 0, error_seq: 0 },
    head: [10, 10],
    bed: [400, 300],
    caps: { pause: true, pause_reason: null, stop: true },
    start: { allowed: false, reason: 'job_active', message: '' },
    route_v: 0,
    live_v: 0,
    live: null,
    camera_v: 0,
    ...over,
  };
}

async function openPage(opts: { stored?: string | 'throws'; initial?: unknown; lang?: string } = {}): Promise<Page> {
  const fetched: string[] = [];
  const calls = new Map<Element, Calls>();
  const rafs: (() => void)[] = [];
  const instances: { listeners: Record<string, ((e: { data: string }) => void)[]> }[] = [];
  let now = 5000;
  const dom = new JSDOM(HTML, {
    url: 'http://127.0.0.1:47802/',
    runScripts: 'outside-only',
    pretendToBeVisual: true,
  });
  const win = dom.window as unknown as Window & typeof globalThis;
  Object.defineProperty(win.navigator, 'language', { value: opts.lang ?? 'es-MX', configurable: true });
  if (opts.stored === 'throws') {
    Object.defineProperty(win, 'localStorage', { get() { throw new Error('bloqueado'); }, configurable: true });
  } else if (opts.stored !== undefined) {
    win.localStorage.setItem('tlaku.remote.live', opts.stored);
  }
  win.performance.now = () => now;
  (win as unknown as { requestAnimationFrame: (cb: (t: number) => void) => number }).requestAnimationFrame = (cb) => {
    rafs.push(() => cb(now));
    return rafs.length;
  };
  (win as unknown as { cancelAnimationFrame: (id: number) => void }).cancelAnimationFrame = () => undefined;
  win.HTMLCanvasElement.prototype.getContext = function (this: HTMLCanvasElement) {
    let c = calls.get(this);
    if (!c) {
      const f = fakeContext();
      c = f.calls;
      calls.set(this, c);
      (this as unknown as { __ctx: unknown }).__ctx = f.ctx;
    }
    return (this as unknown as { __ctx: unknown }).__ctx as never;
  } as never;
  const json = (status: number, body: unknown) => ({ status, ok: status < 400, text: () => Promise.resolve(JSON.stringify(body)) });
  (win as unknown as { fetch: unknown }).fetch = (path: string) => {
    fetched.push(path);
    if (path === '/api/state') return Promise.resolve(json(200, opts.initial ?? state()));
    if (path === '/api/live') return Promise.resolve(json(200, ROUTE));
    return Promise.resolve(json(404, {}));
  };
  class FakeES {
    listeners: Record<string, ((e: { data: string }) => void)[]> = {};
    onerror: unknown = null;
    constructor() {
      instances.push(this);
    }
    addEventListener(name: string, fn: (e: { data: string }) => void) {
      (this.listeners[name] ??= []).push(fn);
    }
    close() {}
  }
  (win as unknown as { EventSource: unknown }).EventSource = FakeES;
  win.eval(HTML.slice(HTML.indexOf('<script>') + 8, HTML.lastIndexOf('</script>')));
  const settle = async () => {
    for (let i = 0; i < 6; i++) await new Promise((r) => setTimeout(r, 0));
  };
  await settle();
  return {
    win,
    doc: win.document,
    fetched,
    es: {
      emit: (st) => {
        for (const es of instances) for (const fn of es.listeners.state ?? []) fn({ data: JSON.stringify(st) });
      },
    },
    calls,
    rafs,
    setNow: (ms) => (now = ms),
    flush: (ms) => {
      now = ms;
      const run = rafs.splice(0);
      for (const f of run) f();
    },
  };
}

const settle = () => new Promise((r) => setTimeout(r, 10));

describe('página del celular: vista en vivo', () => {
  it('apagada de fábrica: muestra el interruptor pero no baja la ruta ni anima nada', async () => {
    const p = await openPage();
    p.es.emit(state({ live_v: 1, live: { t: 10, running: true, retreat_seq: 0, src: 'position' } }));
    await settle();
    const toggle = p.doc.getElementById('live-on') as HTMLInputElement;
    expect(p.doc.getElementById('livecard')!.hidden).toBe(false);
    expect(toggle.checked).toBe(false);
    expect(p.doc.getElementById('livebox')!.hidden).toBe(true);
    expect(p.fetched).not.toContain('/api/live');
    expect(p.rafs).toHaveLength(0);
    expect(p.win.localStorage.getItem('tlaku.remote.live')).toBeNull();
    // El recorrido de siempre sigue a la vista.
    expect(p.doc.getElementById('route')!.hidden).toBe(false);
  });

  it('al encenderla a mitad del trabajo baja la ruta una vez y se pone al día con lo ya hecho', async () => {
    const p = await openPage();
    p.es.emit(state({ live_v: 1, live: { t: route.total * 0.6, running: true, retreat_seq: 0, src: 'position' } }));
    await settle();
    const toggle = p.doc.getElementById('live-on') as HTMLInputElement;
    toggle.checked = true;
    toggle.dispatchEvent(new p.win.Event('change'));
    await settle();
    expect(p.fetched.filter((x) => x === '/api/live')).toHaveLength(1);
    expect(p.win.localStorage.getItem('tlaku.remote.live')).toBe('1');
    p.es.emit(state({ live_v: 1, live: { t: route.total * 0.6, running: true, retreat_seq: 0, src: 'position' } }));
    await settle();
    expect(p.fetched.filter((x) => x === '/api/live')).toHaveLength(1); // no se vuelve a bajar
    expect(p.doc.getElementById('livebox')!.hidden).toBe(false);
    expect(p.doc.getElementById('route')!.hidden).toBe(true);
    p.flush(5016);
    const cv = p.doc.getElementById('livecv')!;
    const shown = p.calls.get(cv)!;
    expect(shown.drawImage).toBe(2); // pendiente + quemado
    expect(shown.arc).toBeGreaterThan(0); // cabezal
    // Lo ya hecho está en la capa de lo quemado: 60 % del tiempo = viaje no (no se dibuja) + lo que lleve del cuadrado.
    const burned = [...p.calls.entries()].filter(([el]) => el !== cv).map(([, c]) => c).find((c) => c.styles.some((x) => x.startsWith('rgb(')) && c.lineTo > 0)!;
    expect(burned.lineTo).toBeGreaterThanOrEqual(2);
    expect(burned.styles[0]).toMatch(/^rgb\(/);
    // Alternativa textual.
    expect(cv.getAttribute('role')).toBe('img');
    expect(cv.getAttribute('aria-label')).toMatch(/^Vista en vivo del trabajo: \d+ %, /);
  });

  it('se recuerda: con la elección guardada, la ruta se baja al primer estado con ruta', async () => {
    const p = await openPage({ stored: '1', initial: state({ live_v: 1, live: { t: 5, running: true, retreat_seq: 0, src: 'position' } }) });
    await settle();
    expect((p.doc.getElementById('live-on') as HTMLInputElement).checked).toBe(true);
    expect(p.fetched).toContain('/api/live');
  });

  it('apagarla en cualquier momento detiene el dibujo y no baja nada más', async () => {
    const p = await openPage({ stored: '1', initial: state({ live_v: 1, live: { t: 5, running: true, retreat_seq: 0, src: 'position' } }) });
    await settle();
    expect(p.rafs.length).toBeGreaterThan(0);
    const toggle = p.doc.getElementById('live-on') as HTMLInputElement;
    toggle.checked = false;
    toggle.dispatchEvent(new p.win.Event('change'));
    expect(p.win.localStorage.getItem('tlaku.remote.live')).toBe('0');
    p.flush(5100);
    expect(p.rafs).toHaveLength(0);
    expect(p.doc.getElementById('livebox')!.hidden).toBe(true);
    const before = p.fetched.length;
    p.es.emit(state({ live_v: 2, live: { t: 9, running: true, retreat_seq: 0, src: 'position' } }));
    await settle();
    expect(p.fetched.length).toBe(before);
    expect(p.rafs).toHaveLength(0);
  });

  it('con el almacenamiento bloqueado funciona igual (apagada) y no falla', async () => {
    const p = await openPage({ stored: 'throws' });
    p.es.emit(state({ live_v: 1 }));
    await settle();
    expect((p.doc.getElementById('live-on') as HTMLInputElement).checked).toBe(false);
    const toggle = p.doc.getElementById('live-on') as HTMLInputElement;
    toggle.checked = true;
    toggle.dispatchEvent(new p.win.Event('change'));
    await settle();
    expect(p.fetched).toContain('/api/live');
  });

  it('en pausa la animación se detiene; cancelado queda congelado; terminado va al final', async () => {
    const p = await openPage({ stored: '1', initial: state({ live_v: 1, live: { t: 5, running: true, retreat_seq: 0, src: 'position' } }) });
    await settle();
    p.flush(5016);
    p.setNow(5250);
    p.es.emit(state({ live_v: 1, machine: { connected: true, kind: 'grbl', state: 'hold' }, job: { ...state().job, phase: 'paused' }, live: { t: 5.2, running: false, retreat_seq: 0, src: 'position' } }));
    await settle();
    for (let ms = 5300; ms < 7500; ms += 100) p.flush(ms);
    expect(p.rafs).toHaveLength(0); // se asentó y dejó de pedir cuadros
    const cv = p.doc.getElementById('livecv')!;
    const calls = p.calls.get(cv)!;
    const drawn = calls.drawImage;
    // Cancelado: queda donde estaba y no pide más cuadros.
    p.es.emit(state({ live_v: 1, job: { ...state().job, phase: 'cancelled' }, live: { t: 5.2, running: false, retreat_seq: 0, src: 'position' } }));
    await settle();
    p.flush(8000);
    p.flush(8100);
    expect(p.rafs).toHaveLength(0);
    expect(calls.drawImage).toBeGreaterThanOrEqual(drawn);
    // Terminado: el cabezal acaba en el final de la ruta (el último vértice).
    p.es.emit(state({ live_v: 1, job: { ...state().job, phase: 'finished', finished_seq: 1 }, live: { t: route.total, running: false, retreat_seq: 0, src: 'position' } }));
    await settle();
    p.flush(9000);
    expect(p.doc.getElementById('livecv')!.getAttribute('aria-label')).toMatch(/100 %/);
  });

  it('si la PC sigue el trabajo pero no manda ruta (demasiado grande), lo dice pasados unos segundos', async () => {
    const live = { t: 5, running: true, retreat_seq: 0, src: 'position' };
    const p = await openPage({ stored: '1', initial: state({ live_v: 0, live }) });
    await settle();
    const note = () => p.doc.getElementById('live-note')!.textContent;
    expect(note()).toMatch(/Esperando la ruta/);
    const real = p.win.Date.now();
    p.win.Date.now = () => real + 6000;
    p.es.emit(state({ live_v: 0, live }));
    await settle();
    expect(note()).toMatch(/demasiado grande/);
    expect(p.fetched).not.toContain('/api/live'); // sin ruta no se pide nada
  });

  it('en inglés los textos nuevos están en inglés', async () => {
    const p = await openPage({ lang: 'en-US' });
    expect(p.doc.getElementById('h-live')!.textContent).toBe('Live view');
    expect(p.doc.getElementById('live-hint')!.textContent).toMatch(/View only/);
    const es = await openPage({ lang: 'es-MX' });
    expect(es.doc.getElementById('h-live')!.textContent).toBe('Vista en vivo');
  });

  it('es solo lectura: la página no manda nada a la máquina al mostrar la vista', async () => {
    const p = await openPage({ stored: '1', initial: state({ live_v: 1, live: { t: 5, running: true, retreat_seq: 0, src: 'position' } }) });
    await settle();
    p.flush(5016);
    // Solo se hacen lecturas: ninguna petición de acción.
    expect(p.fetched.every((x) => x === '/api/state' || x === '/api/live' || x === '/api/route')).toBe(true);
  });
});
