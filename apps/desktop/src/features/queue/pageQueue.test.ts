import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { JSDOM } from 'jsdom';
import { describe, expect, it } from 'vitest';

/**
 * La página del celular (`src-tauri/src/remote/page.html`) con su script de verdad dentro de jsdom: la cola se
 * ve (solo lectura) y confirmar el siguiente trabajo exige, además de las tres casillas de siempre, la del
 * material. El celular no tiene ningún camino para avanzar la cola sin ese inicio con checklist.
 */

const HTML = readFileSync(join(__dirname, '../../../src-tauri/src/remote/page.html'), 'utf8');

const item = (name: string, copies: number, done: number, status = 'pending') => ({ name, copies, done, status });

function state(queue: unknown, over: Record<string, unknown> = {}) {
  return {
    v: 1,
    pc_online: true,
    machine: { connected: true, kind: 'grbl', state: 'idle' },
    device: 'Falcon',
    doc: 'caja.tlaku',
    job: { phase: 'idle', message: '', progress: 0, elapsed_s: 0, remaining_s: null, total_s: 90, layer: null, finished_seq: 0, error_seq: 0 },
    head: null,
    bed: [400, 300],
    caps: { pause: true, pause_reason: null, stop: true },
    start: { allowed: true, reason: null, message: '' },
    route_v: 0,
    live_v: 0,
    live: null,
    camera_v: 0,
    queue,
    ...over,
  };
}

const AWAITING = {
  state: 'awaiting',
  done: 1,
  total: 4,
  eta_s: 300,
  current: item('caja.tlaku', 2, 1),
  copy: 2,
  message: '',
  items: [item('base.tlaku', 1, 1, 'done'), item('caja.tlaku', 2, 1), item('tapa.tlaku', 1, 0)],
};

async function openPage(initial: unknown, lang = 'es-MX') {
  const posts: { path: string; body: unknown }[] = [];
  const paths: string[] = [];
  const dom = new JSDOM(HTML, { url: 'http://127.0.0.1:47802/', runScripts: 'outside-only', pretendToBeVisual: true });
  const win = dom.window as unknown as Window & typeof globalThis;
  Object.defineProperty(win.navigator, 'language', { value: lang, configurable: true });
  const json = (status: number, body: unknown) => ({ status, ok: status < 400, text: () => Promise.resolve(JSON.stringify(body)) });
  (win as unknown as { fetch: unknown }).fetch = (path: string, opt?: { method?: string; body?: string }) => {
    paths.push(path);
    if (opt?.method === 'POST') {
      posts.push({ path, body: JSON.parse(opt.body ?? '{}') });
      return Promise.resolve(json(200, {}));
    }
    return Promise.resolve(json(200, path === '/api/state' ? initial : {}));
  };
  class FakeES {
    onerror: unknown = null;
    addEventListener() {}
    close() {}
  }
  (win as unknown as { EventSource: unknown }).EventSource = FakeES;
  win.eval(HTML.slice(HTML.indexOf('<script>') + 8, HTML.lastIndexOf('</script>')));
  for (let i = 0; i < 6; i++) await new Promise((r) => setTimeout(r, 0));
  return { win, doc: win.document, posts, paths };
}

const click = (win: Window & typeof globalThis, el: Element) => el.dispatchEvent(new win.Event('click', { bubbles: true }));
const tick = (win: Window & typeof globalThis, el: HTMLInputElement, on: boolean) => {
  el.checked = on;
  el.dispatchEvent(new win.Event('change', { bubbles: true }));
};
const settle = () => new Promise((r) => setTimeout(r, 10));

describe('página del celular: cola de trabajos', () => {
  it('sin cola, la tarjeta no aparece', async () => {
    const p = await openPage(state(null));
    expect(p.doc.getElementById('queue')!.hidden).toBe(true);
  });

  it('con cola muestra estado, avance, tiempo, trabajo siguiente y la lista (solo lectura)', async () => {
    const p = await openPage(state(AWAITING));
    expect(p.doc.getElementById('queue')!.hidden).toBe(false);
    expect(p.doc.getElementById('q-state')!.textContent).toBe('Esperando confirmar el siguiente');
    expect(p.doc.getElementById('q-prog')!.textContent).toBe('1 de 4');
    expect(p.doc.getElementById('q-eta')!.textContent).toBe('5:00');
    expect(p.doc.getElementById('q-current')!.textContent).toBe('Siguiente: caja.tlaku (copia 2 de 2)');
    const rows = [...p.doc.querySelectorAll('#q-list li')];
    expect(rows.map((r) => r.firstElementChild!.textContent)).toEqual(['base.tlaku', 'caja.tlaku', 'tapa.tlaku']);
    expect(rows[1]!.getAttribute('data-current')).toBe('1');
    // La tarjeta es solo texto: ningún botón ni control para mover la cola.
    expect(p.doc.querySelectorAll('#queue button, #queue input').length).toBe(0);
    expect(p.doc.getElementById('q-hint')!.textContent).toContain('nunca avanza sola');
  });

  it('en inglés', async () => {
    const p = await openPage(state(AWAITING), 'en-US');
    expect(p.doc.getElementById('q-state')!.textContent).toBe('Waiting to confirm the next one');
    expect(p.doc.getElementById('q-current')!.textContent).toBe('Next: caja.tlaku (copy 2 of 2)');
    expect(p.doc.getElementById('btn-start')!.textContent).toBe('Confirm the next job…');
  });

  it('una cola detenida muestra por qué', async () => {
    const p = await openPage(state({ ...AWAITING, state: 'stopped', message: 'Alarma 2' }));
    expect(p.doc.getElementById('q-msg')!.hidden).toBe(false);
    expect(p.doc.getElementById('q-msg')!.textContent).toBe('Alarma 2');
    expect(p.doc.getElementById('btn-start')!.textContent).toBe('Iniciar trabajo…');
  });

  it('confirmar el siguiente trabajo pide una cuarta casilla (material) y la manda en el checklist', async () => {
    const p = await openPage(state(AWAITING));
    const start = p.doc.getElementById('btn-start')!;
    expect(start.textContent).toBe('Confirmar el siguiente trabajo…');
    click(p.win, start);
    const c = ['c1', 'c2', 'c3', 'c4'].map((id) => p.doc.getElementById(id) as HTMLInputElement);
    expect(p.doc.getElementById('c4row')!.hidden).toBe(false);
    expect(p.doc.getElementById('sheet-title')!.textContent).toBe('Antes de confirmar el siguiente trabajo');
    expect(p.doc.getElementById('sheet-job')!.textContent).toContain('caja.tlaku');
    const go = p.doc.getElementById('sheet-go') as HTMLButtonElement;
    for (const box of c.slice(0, 3)) tick(p.win, box, true);
    expect(go.disabled).toBe(true); // falta el material
    tick(p.win, c[3]!, true);
    expect(go.disabled).toBe(false);
    click(p.win, go);
    await settle();
    expect(p.posts).toEqual([{ path: '/api/start', body: { checklist: { nearby: true, clear: true, air: true, material: true } } }]);
  });

  it('un inicio normal (sin cola esperando) sigue pidiendo solo las tres casillas', async () => {
    const p = await openPage(state({ ...AWAITING, state: 'idle' }));
    click(p.win, p.doc.getElementById('btn-start')!);
    expect(p.doc.getElementById('c4row')!.hidden).toBe(true);
    const go = p.doc.getElementById('sheet-go') as HTMLButtonElement;
    for (const id of ['c1', 'c2', 'c3']) tick(p.win, p.doc.getElementById(id) as HTMLInputElement, true);
    expect(go.disabled).toBe(false);
    click(p.win, go);
    await settle();
    expect(p.posts).toEqual([{ path: '/api/start', body: { checklist: { nearby: true, clear: true, air: true, material: false } } }]);
  });

  it('el celular no tiene ningún camino propio para avanzar la cola', async () => {
    const p = await openPage(state(AWAITING));
    expect(HTML).not.toMatch(/\/api\/queue/);
    expect(p.paths.every((x) => x === '/api/state' || x === '/api/events')).toBe(true);
  });
});
