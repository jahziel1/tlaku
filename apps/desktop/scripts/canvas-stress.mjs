// Prueba de estrés del lienzo en modo mock con Chrome headless (mismo método que screenshots.mjs).
//
//   node scripts/canvas-stress.mjs                       # 1 000, 10 000, 50 000 objetos y un trayecto de 1 M de nodos
//   node scripts/canvas-stress.mjs --only obj-1000       # un solo escenario (también obj-100000 y path-100k, los extras)
//   node scripts/canvas-stress.mjs --quick               # solo 1 000 objetos (humo)
//   node scripts/canvas-stress.mjs --only obj-50000 --steps hover,move-few   # solo esos pasos (la carga siempre corre)
//
// Pasos extra de la ola 6B (hover, mover 10 objetos, verificación): mover 10 objetos de un diseño enorme (arrastre, confirmar, deshacer), los tiempos
// de JavaScript de cada paso (`... JS ms`: lo que tarda en volver el evento de ratón o la acción del store, sin
// rasterizar; incluye el suelo de ~18 ms del protocolo) y una verificación píxel a píxel de que lo que dejan el
// arrastre por capas y el repintado por zonas es idéntico a repintar todo (solo con 10 000 objetos o más).
//
// Arranca su propio servidor de Vite (puerto 1431, sin recarga en caliente) salvo que se defina
// APP_URL. Mide, por escenario: carga y primer dibujo, paneo, zoom con la rueda, clic para
// seleccionar, selección por rectángulo, mover la selección (arrastre y confirmación) y deshacer.
// Cada medida es el tiempo hasta que el navegador termina el siguiente cuadro (dos rAF), y además
// cuánto tardó el `render()` del lienzo (solo JavaScript, sin la rasterización).
//
// Variables: CHROME, APP_URL, OUT (JSON de salida), STEP_TIMEOUT_S (por paso, 120), HEAVY_NODES.
//
// OJO: Chrome corre con rasterización por software (SwiftShader), como en screenshots.mjs, así que los
// tiempos de pintura son pesimistas frente a una GPU real; los tiempos de JavaScript sí son
// representativos.
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const appDir = join(here, '..');
const args = process.argv.slice(2);
const argOf = (name) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : undefined; };
const only = argOf('--only');
const quick = args.includes('--quick');
// --steps hover,move-few : solo esos pasos (además de la carga, que siempre corre)
const stepsOnly = argOf('--steps')?.split(',');
const port = 9344;
const stepTimeoutMs = Number(process.env.STEP_TIMEOUT_S ?? 120) * 1000;
const out = process.env.OUT ?? join(tmpdir(), 'canvas-stress.json');
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

const bin = process.env.CHROME ?? ['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', '/usr/bin/google-chrome', '/usr/bin/chromium'].find(existsSync);
if (!bin) throw new Error('No se encontró Chrome/Chromium. Define CHROME=/ruta/al/binario');

const ALL = [
  { id: 'obj-1000', kind: 'objects', n: 1000 },
  { id: 'obj-10000', kind: 'objects', n: 10000 },
  { id: 'obj-50000', kind: 'objects', n: 50000 },
  { id: 'path-1m', kind: 'bigpath', nodes: 1048576 },
];
const EXTRA = [
  { id: 'obj-100000', kind: 'objects', n: 100000 },
  { id: 'path-100k', kind: 'bigpath', nodes: 102400 },
];
let scenarios = quick ? ALL.slice(0, 1) : ALL;
if (only) scenarios = [...ALL, ...EXTRA].filter((s) => s.id === only);
if (!scenarios.length) throw new Error(`sin escenario ${only}`);

// ---- servidor de Vite -------------------------------------------------------------------------------------------------
let vite = null;
let url = process.env.APP_URL;
if (!url) {
  const vport = 1431;
  vite = spawn('npx', ['vite', '--config', 'scripts/vite.stress.config.mjs', '--port', String(vport), '--strictPort'], { cwd: appDir, stdio: 'ignore' });
  url = `http://localhost:${vport}/`;
  let up = false;
  for (let i = 0; i < 100 && !up; i++) {
    await wait(300);
    try { up = (await fetch(url)).ok; } catch { /* arrancando */ }
  }
  if (!up) { vite.kill(); throw new Error('Vite no arrancó'); }
}

// ---- Chrome ---------------------------------------------------------------------------------------------------------------
async function launch() {
  const proc = spawn(bin, [
    '--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), 'ls-stress-'))}`,
    '--disable-gpu', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--hide-scrollbars', '--enable-precise-memory-info',
    '--window-size=1440,900', '--force-device-scale-factor=1', 'about:blank',
  ], { stdio: 'ignore' });
  let target;
  for (let i = 0; i < 50 && !target; i++) {
    await wait(200);
    try {
      const list = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
      target = list.find((t) => t.type === 'page');
    } catch { /* aún arrancando */ }
  }
  if (!target) { proc.kill(); throw new Error('Chrome no respondió'); }
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((r) => (ws.onopen = r));
  let id = 0;
  const pending = new Map();
  ws.onmessage = (m) => {
    const msg = JSON.parse(m.data);
    if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
  };
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const mid = ++id;
    const timer = setTimeout(() => { pending.delete(mid); reject(new Error(`TIMEOUT ${method} (> ${stepTimeoutMs / 1000} s)`)); }, stepTimeoutMs);
    pending.set(mid, (msg) => { clearTimeout(timer); if (msg.error) reject(new Error(msg.error.message)); else resolve(msg.result); });
    ws.send(JSON.stringify({ id: mid, method, params }));
  });
  const evaluate = async (expression) => {
    const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    return r.result.value;
  };
  return { proc, send, evaluate, close: () => { try { ws.close(); } catch { /* nada */ } proc.kill('SIGKILL'); } };
}

// ---- código que corre dentro de la página -------------------------------------------------------------------------
const PAGE_SETUP = `(async () => {
  const { useStore } = await import('/src/store/index.ts');
  const { Canvas2DRenderer } = await import('/src/features/canvas/canvas2d.ts');
  const { makeLayer } = await import('/src/lib/layers.ts');
  const { newVectorObject } = await import('/src/features/canvas/shapes.ts');
  const { polylineBez } = await import('/src/lib/bezpath.ts');
  const { translateById } = await import('/src/store/document.ts');
  const { useInteraction } = await import('/src/features/canvas/interaction.ts');
  window.__interaction = useInteraction;
  const S = (window.__st = { renders: [], hits: [], useStore });
  const orig = Canvas2DRenderer.prototype.render;
  Canvas2DRenderer.prototype.render = function (scene) {
    const t = performance.now();
    try { return orig.call(this, scene); } finally { S.renders.push(performance.now() - t); }
  };
  S.frame = () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(performance.now()))));
  const COLORS = ['#ff0000', '#0000ff', '#00aa00', '#ff8800'];
  const layers = COLORS.map((c, i) => makeLayer(c, 'C' + (i + 1)));
  const hilbert = (n, d) => {
    let x = 0, y = 0, t = d;
    for (let s = 1; s < n; s *= 2) {
      const rx = 1 & (t >> 1), ry = 1 & (t ^ rx);
      if (ry === 0) { if (rx === 1) { x = s - 1 - x; y = s - 1 - y; } [x, y] = [y, x]; }
      x += s * rx; y += s * ry; t >>= 2;
    }
    return [x, y];
  };
  /** n objetos pequeños (cuadrados cerrados de 1,2 mm) en una rejilla sobre la cama; devuelve puntos de clic sobre sus bordes. */
  S.makeObjects = (n) => {
    const cols = Math.ceil(Math.sqrt(n * (390 / 405)));
    const rows = Math.ceil(n / cols);
    const cw = 390 / cols, ch = 405 / rows, sz = Math.min(cw, ch) * 0.7;
    const objects = [];
    for (let k = 0; k < n; k++) {
      const x = 5 + (k % cols) * cw, y = 5 + Math.floor(k / cols) * ch;
      const pts = [[x, y], [x + sz, y], [x + sz, y + sz], [x, y + sz]];
      objects.push(newVectorObject('o' + k, COLORS[k % 4], [polylineBez(pts, true)]));
    }
    S.hits = Array.from({ length: 20 }, (_, i) => { const o = objects[Math.floor(((i * 7919) % 1000) / 1000 * n)]; return o.kind.paths[0][0].to; });
    // Los diez últimos objetos (para mover pocos objetos de un diseño enorme). Son los de más arriba en el orden
    // del documento: un clic en el último los toma a todos (con la tolerancia de 6 px, un vecino posterior ganaría).
    S.few = objects.slice(n - 10).map((o) => o.id);
    S.fewHit = objects[n - 1].kind.paths[0][0].to;
    return objects;
  };
  /** Un solo objeto con un trayecto de \`nodes\` nodos (curva de Hilbert). */
  S.makeBigPath = (nodes) => {
    const order = Math.round(Math.log2(Math.sqrt(nodes)));
    const side = 2 ** order, cell = 380 / side;
    const segs = new Array(side * side);
    for (let d = 0; d < side * side; d++) {
      const [x, y] = hilbert(side, d);
      segs[d] = { type: d === 0 ? 'move_to' : 'line_to', to: [10 + x * cell, 10 + y * cell] };
    }
    S.hits = Array.from({ length: 20 }, (_, i) => segs[Math.floor(((i * 7919) % 1000) / 1000 * segs.length)].to);
    return [newVectorObject('big', '#ff0000', [segs])];
  };
  S.load = async (objects) => {
    const st = useStore.getState();
    S.renders.length = 0;
    const heap0 = performance.memory?.usedJSHeapSize ?? 0;
    const t0 = performance.now();
    st.replaceDocument({ ...st.doc, objects, layers }, null);
    st.fitContent();
    const t1 = performance.now();
    await S.frame();
    const t2 = performance.now();
    return { sync_ms: t1 - t0, first_frame_ms: t2 - t0, renders: S.renders.slice(), heap_mb: (performance.memory?.usedJSHeapSize ?? 0) / 1e6, heap_before_mb: heap0 / 1e6 };
  };
  S.view = () => { const s = useStore.getState(); const r = document.querySelector('.canvas-area').getBoundingClientRect(); return { zoom: s.zoom, panX: s.panX, panY: s.panY, left: r.left, top: r.top, w: r.width, h: r.height }; };
  S.hitClient = () => { const v = S.view(); return S.hits.map(([x, y]) => [v.left + v.panX + x * v.zoom, v.top + v.panY - y * v.zoom]); };
  S.worldToClient = (x, y) => { const v = S.view(); return [v.left + v.panX + x * v.zoom, v.top + v.panY - y * v.zoom]; };
  /** Un vértice del último objeto del documento tal como está ahora (los pasos anteriores lo movieron). */
  S.fewHitNow = () => { const o = useStore.getState().doc.objects.at(-1); const [x, y] = o.kind.paths[0][0].to; const t = o.transform; return [t[0] * x + t[2] * y + t[4], t[1] * x + t[3] * y + t[5]]; };
  /** Solo la transformación del documento (sin historial ni suscriptores), en ms. */
  S.pureMove = (ids) => { const d = useStore.getState().doc; const t = performance.now(); translateById(d, ids, 1, 1); return performance.now() - t; };
  S.hover = () => { const i = window.__interaction; return i ? i.getState().hoverId : null; };
  S.sel = () => useStore.getState().selectedIds.length;
  S.begin = () => { S.renders.length = 0; S.t0 = performance.now(); };
  S.end = async () => { await S.frame(); const total = performance.now() - S.t0; const r = S.renders.slice(); return { ms: total, render_ms: r.reduce((a, b) => a + b, 0), renders: r.length, worst_render_ms: Math.max(0, ...r) }; };
  return 'ok';
})()`;

async function openPage(c) {
  await c.send('Page.enable');
  await c.send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await c.send('Page.navigate', { url });
  await wait(2500);
  await c.evaluate(`document.querySelectorAll('.toast button').forEach(b => b.click())`);
  await c.evaluate(`new Promise((ok, fail) => { const t0 = Date.now(); const tick = async () => {
    const { useStore } = await import('/src/store/index.ts');
    if (useStore.getState().doc.objects.length > 0 && useStore.getState().viewport.width > 0) return ok();
    if (Date.now() - t0 > 20000) return ok(); // sin diseño de ejemplo: seguimos
    setTimeout(tick, 200); }; tick(); })`);
  // La pregunta de primer uso («¿Es tu primera vez con un láser?») tapa el lienzo y se come los clics: se elige el modo completo.
  await c.evaluate(`(async () => { const { useStore } = await import('/src/store/index.ts'); useStore.getState().setPref('uiMode', 'full'); })()`);
  await wait(300);
  await c.evaluate(`document.querySelectorAll('.toast button').forEach(b => b.click())`); // avisos de la importación de muestra (tapan el rincón de la cama)
  await wait(300);
  await c.evaluate(PAGE_SETUP);
}

const stats = (xs) => {
  if (!xs.length) return { n: 0 };
  const s = [...xs].sort((a, b) => a - b);
  const q = (p) => s[Math.min(s.length - 1, Math.round((s.length - 1) * p))];
  return { n: s.length, median: +q(0.5).toFixed(1), p95: +q(0.95).toFixed(1), max: +s[s.length - 1].toFixed(1) };
};

async function runScenario(sc) {
  const res = { id: sc.id, steps: {}, notes: [] };
  const c = await launch();
  const step = async (name, fn) => {
    if (stepsOnly && !['baseline', 'load'].includes(name) && !stepsOnly.includes(name)) return;
    const t0 = Date.now();
    try {
      res.steps[name] = await fn();
    } catch (e) {
      res.steps[name] = { error: String(e.message ?? e) };
      res.notes.push(`${name}: ${e.message}`);
      throw e;
    } finally {
      console.log(`  ${sc.id} · ${name}: ${((Date.now() - t0) / 1000).toFixed(1)} s`);
    }
  };
  const mouse = (type, x, y, extra = {}) => c.send('Input.dispatchMouseEvent', { type, x, y, button: 'none', buttons: 0, ...extra });
  // `call_ms`: lo que tarda `fn` en volver (con eventos de ratón por CDP = los manejadores de JavaScript, sin la rasterización).
  const timed = async (fn) => { await c.evaluate('window.__st.begin()'); const t0 = performance.now(); await fn(); const call_ms = performance.now() - t0; const r = await c.evaluate('window.__st.end()'); return { ...r, call_ms }; };
  try {
    await openPage(c);
    // 0. suelo del método: dos rAF sin hacer nada
    await step('baseline', async () => {
      const ms = [];
      for (let i = 0; i < 20; i++) ms.push((await timed(async () => {})).ms);
      return { frame_ms: stats(ms) };
    });
    // 1. carga
    await step('load', async () => {
      await c.evaluate(`window.__st.__objs = ${sc.kind === 'objects' ? `window.__st.makeObjects(${sc.n})` : `window.__st.makeBigPath(${sc.nodes})`}; 0`);
      const r = await c.evaluate('window.__st.load(window.__st.__objs)');
      await wait(600);
      const settle = await c.evaluate('(()=>{const r=window.__st.renders.slice(); window.__st.renders.length=0; return r;})()');
      return { ...r, settle_renders_ms: settle.map((x) => +x.toFixed(1)) };
    });
    // 2. paneo
    await step('pan', async () => {
      const ms = [], rms = [];
      for (let i = 0; i < 15; i++) {
        const r = await timed(() => c.evaluate('window.__st.useStore.getState().panBy(9, 4)'));
        ms.push(r.ms); rms.push(r.render_ms);
      }
      return { frame_ms: stats(ms), render_ms: stats(rms) };
    });
    // 3. zoom con la rueda (hacia el centro del lienzo): 10 acercando y 10 alejando
    await step('zoom', async () => {
      await c.evaluate('window.__st.useStore.getState().fitContent()');
      await wait(400);
      const v = await c.evaluate('window.__st.view()');
      const cx = v.left + v.w / 2, cy = v.top + v.h / 2;
      const inMs = [], inR = [], outMs = [], outR = [];
      for (let i = 0; i < 10; i++) {
        const r = await timed(() => c.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: cx, y: cy, deltaX: 0, deltaY: -120 }));
        inMs.push(r.ms); inR.push(r.render_ms);
      }
      const zoomIn = (await c.evaluate('window.__st.view()')).zoom;
      await c.evaluate('window.__st.renders.length=0');
      await wait(500); // repintado nítido tras quedarse quieto
      const settleRender = await c.evaluate('window.__st.renders.reduce((a,b)=>a+b,0)');
      for (let i = 0; i < 10; i++) {
        const r = await timed(() => c.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: cx, y: cy, deltaX: 0, deltaY: 120 }));
        outMs.push(r.ms); outR.push(r.render_ms);
      }
      await wait(500);
      return { zoom_final_px_per_mm: +zoomIn.toFixed(2), in_frame_ms: stats(inMs), in_render_ms: stats(inR), settle_render_ms: +settleRender.toFixed(1), out_frame_ms: stats(outMs), out_render_ms: stats(outR) };
    });
    // 4. clic para seleccionar (20 sobre bordes y 5 en vacío)
    await step('click', async () => {
      await c.evaluate('window.__st.useStore.getState().fitContent()');
      await wait(400);
      const pts = await c.evaluate('window.__st.hitClient()');
      const ms = [], rms = [], cms = [];
      let hits = 0;
      for (const [x, y] of pts) {
        await c.evaluate('window.__st.useStore.getState().clearSelection()');
        const r = await timed(async () => {
          await mouse('mouseMoved', x, y);
          await mouse('mousePressed', x, y, { button: 'left', buttons: 1, clickCount: 1 });
          await mouse('mouseReleased', x, y, { button: 'left', buttons: 0, clickCount: 1 });
        });
        ms.push(r.ms); rms.push(r.render_ms); cms.push(r.call_ms);
        if ((await c.evaluate('window.__st.sel()')) > 0) hits++;
      }
      return { clicks: pts.length, selected: hits, frame_ms: stats(ms), render_ms: stats(rms), call_ms: stats(cms) };
    });
    // 4b. hover: pasar el ratón (sin botones) sobre 20 bordes de objetos y 5 puntos vacíos; cada medida incluye el hit-test del cuadro
    await step('hover', async () => {
      await c.evaluate('window.__st.useStore.getState().clearSelection(); window.__st.useStore.getState().fitContent()');
      await wait(500);
      const pts = await c.evaluate('window.__st.hitClient()');
      const empty = await c.evaluate('[[402, 410], [-3, -3], [200, 420], [405, 200], [-4, 100]].map(([x, y]) => window.__st.worldToClient(x, y))');
      const ms = [], rms = [], cms = [];
      let hovered = 0;
      for (const [x, y] of [...pts, ...empty]) {
        const r = await timed(() => mouse('mouseMoved', x, y));
        ms.push(r.ms); rms.push(r.render_ms); cms.push(r.call_ms);
        if (await c.evaluate('!!window.__st.hover()')) hovered++;
      }
      return { points: pts.length + empty.length, hovered, frame_ms: stats(ms), render_ms: stats(rms), call_ms: stats(cms) };
    });
    // 5. rectángulo de selección: pequeño, un cuarto de la cama, toda la cama (cruzando)
    // Siempre se parte de un rincón vacío fuera del diseño (con miles de objetos, un punto interior
    // cae a menos de 6 px de alguno y el gesto sería mover y no un rectángulo).
    const boxes = [['pequeño 20x20 mm (rincón)', 402, 418, 382, 398], ['cuarto de cama (rincón al centro)', 402, 418, 200, 208], ['toda la cama', 402, 418, -2, -2]];
    await step('marquee', async () => {
      const rows = {};
      for (const [label, x0, y0, x1, y1] of boxes) {
        await c.evaluate('window.__st.useStore.getState().clearSelection(); window.__st.useStore.getState().fitContent()');
        await wait(300);
        // de derecha a izquierda = cruce (selecciona lo que toque la caja)
        const [ax, ay] = await c.evaluate(`window.__st.worldToClient(${x0}, ${y0})`);
        const [bx, by] = await c.evaluate(`window.__st.worldToClient(${x1}, ${y1})`);
        const mv = [], mvr = [];
        await mouse('mouseMoved', ax, ay);
        await mouse('mousePressed', ax, ay, { button: 'left', buttons: 1, clickCount: 1 });
        for (let i = 1; i <= 8; i++) {
          const r = await timed(() => mouse('mouseMoved', ax + ((bx - ax) * i) / 8, ay + ((by - ay) * i) / 8, { buttons: 1 }));
          mv.push(r.ms); mvr.push(r.render_ms);
        }
        const up = await timed(() => mouse('mouseReleased', bx, by, { button: 'left', buttons: 0, clickCount: 1 }));
        rows[label] = { selected: await c.evaluate('window.__st.sel()'), drag_frame_ms: stats(mv), drag_render_ms: stats(mvr), release_ms: +up.ms.toFixed(1), release_render_ms: +up.render_ms.toFixed(1) };
      }
      return rows;
    });
    // 6. mover la selección: seleccionar todo y arrastrar 10 pasos desde un objeto; luego deshacer
    await step('move', async () => {
      await c.evaluate('window.__st.useStore.getState().fitContent()');
      await wait(300);
      const selAll = await timed(() => c.evaluate('window.__st.useStore.getState().selectAll()'));
      const [hx, hy] = (await c.evaluate('window.__st.hitClient()'))[3];
      const mv = [], mvr = [], mvc = [];
      await mouse('mouseMoved', hx, hy);
      await mouse('mousePressed', hx, hy, { button: 'left', buttons: 1, clickCount: 1 });
      for (let i = 1; i <= 10; i++) {
        const r = await timed(() => mouse('mouseMoved', hx + i * 4, hy + i * 3, { buttons: 1 }));
        mv.push(r.ms); mvr.push(r.render_ms); mvc.push(r.call_ms);
      }
      const hist0 = await c.evaluate('window.__st.useStore.getState().past.length');
      const up = await timed(() => mouse('mouseReleased', hx + 40, hy + 30, { button: 'left', buttons: 0, clickCount: 1 }));
      const moved = (await c.evaluate('window.__st.useStore.getState().past.length')) > hist0;
      const undo = await timed(() => c.evaluate('window.__st.useStore.getState().undo()'));
      const moveDirect = await timed(() => c.evaluate("(()=>{const s=window.__st.useStore.getState(); s.moveObjects(s.selectedIds, 5, 5, 'stress'); return 0;})()"));
      // Solo la acción del store (JavaScript síncrono, sin pintar), tres veces.
      const sync = [];
      for (let i = 0; i < 3; i++) sync.push(await c.evaluate("(()=>{const s=window.__st.useStore.getState(); const t=performance.now(); s.moveObjects(s.selectedIds, 1+" + i + ", 1, 'stress-sync'+" + i + "); return performance.now()-t;})()"));
      const pastBefore = await c.evaluate('window.__st.useStore.getState().past.length');
      await c.evaluate("(()=>{const s=window.__st.useStore.getState(); s.moveObjects(s.selectedIds, 0.5, 0.5, 'one'); s.moveObjects(s.selectedIds, 0.5, 0.5, 'one'); return 0;})()");
      const oneStep = (await c.evaluate('window.__st.useStore.getState().past.length')) - pastBefore;
      const undoSync = await c.evaluate("(()=>{const t=performance.now(); window.__st.useStore.getState().undo(); return performance.now()-t;})()");
      const pure = await c.evaluate("window.__st.pureMove(window.__st.useStore.getState().selectedIds)");
      return { move_pure_ms: +pure.toFixed(1), move_sync_ms: stats(sync), undo_sync_ms: +undoSync.toFixed(1), merged_steps: oneStep, drag_call_ms: stats(mvc), commit_call_ms: +up.call_ms.toFixed(1), moved, selected: await c.evaluate('window.__st.sel()'), select_all_ms: +selAll.ms.toFixed(1), drag_frame_ms: stats(mv), drag_render_ms: stats(mvr), commit_ms: +up.ms.toFixed(1), commit_render_ms: +up.render_ms.toFixed(1), undo_ms: +undo.ms.toFixed(1), undo_render_ms: +undo.render_ms.toFixed(1), move_command_ms: +moveDirect.ms.toFixed(1) };
    });
    // 7. mover pocos objetos (10) de un diseño enorme: arrastre, confirmar y deshacer
    if (sc.kind === 'objects') {
      await step('move-few', async () => {
        await c.evaluate('window.__st.useStore.getState().clearSelection(); window.__st.useStore.getState().fitContent()');
        await wait(600);
        await c.evaluate('window.__st.useStore.getState().select(window.__st.few)');
        await wait(300);
        const [hx, hy] = await c.evaluate('window.__st.worldToClient(...window.__st.fewHitNow())');
        const mv = [], mvr = [], mvc = [];
        await mouse('mouseMoved', hx, hy);
        await mouse('mousePressed', hx, hy, { button: 'left', buttons: 1, clickCount: 1 });
        for (let i = 1; i <= 10; i++) {
          const r = await timed(() => mouse('mouseMoved', hx + i * 4, hy + i * 3, { buttons: 1 }));
          mv.push(r.ms); mvr.push(r.render_ms); mvc.push(r.call_ms);
        }
        const selected = await c.evaluate('window.__st.sel()');
        const hist0 = await c.evaluate('window.__st.useStore.getState().past.length');
        const up = await timed(() => mouse('mouseReleased', hx + 40, hy + 30, { button: 'left', buttons: 0, clickCount: 1 }));
        const moved = (await c.evaluate('window.__st.useStore.getState().past.length')) === hist0 + 1;
        const undo = await timed(() => c.evaluate('window.__st.useStore.getState().undo()'));
        const sync = [];
        for (let i = 0; i < 3; i++) sync.push(await c.evaluate("(()=>{const s=window.__st.useStore.getState(); const t=performance.now(); s.moveObjects(window.__st.few, 1+" + i + ", 1, 'few-sync'+" + i + "); return performance.now()-t;})()"));
        const nudge = await timed(() => c.evaluate("window.__st.useStore.getState().moveObjects(window.__st.few, 2, 0, 'few-nudge')"));
        const pure = await c.evaluate('window.__st.pureMove(window.__st.few)');
        return { selected, move_pure_ms: +pure.toFixed(1), moved_one_step: moved, drag_frame_ms: stats(mv), drag_render_ms: stats(mvr), drag_call_ms: stats(mvc), commit_ms: +up.ms.toFixed(1), commit_render_ms: +up.render_ms.toFixed(1), commit_call_ms: +up.call_ms.toFixed(1), undo_ms: +undo.ms.toFixed(1), move_sync_ms: stats(sync), nudge_ms: +nudge.ms.toFixed(1) };
      });
    }
    // 8. verificación: lo que dejan el arrastre por capas y el repintado por zonas es idéntico, píxel a píxel, a repintar todo
    if (sc.kind === 'objects' && sc.n >= 10000) {
      await step('verify', async () => {
        const snap = () => c.evaluate(`(() => { const cv = document.querySelector('canvas.canvas'); return Array.from(cv.getContext('2d').getImageData(0, 0, cv.width, cv.height).data); })()`);
        // mover la vista y devolverla fuerza un repintado completo con la misma vista (tras asentarse)
        const full = async () => {
          await c.evaluate('window.__st.useStore.setState({ panX: window.__st.useStore.getState().panX + 37 })'); await wait(500);
          await c.evaluate('window.__st.useStore.setState({ panX: window.__st.useStore.getState().panX - 37 })'); await wait(700);
          return snap();
        };
        const same = (a, b) => { let n = 0; for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) n++; return n; };
        await c.evaluate("(() => { const st = window.__st; const [fx, fy] = st.fewHitNow(); const z = 3.2; st.useStore.setState({ zoom: z, panX: 450 - fx * z, panY: 350 + fy * z }); })()");
        await wait(800);
        await c.evaluate('window.__st.useStore.getState().select(window.__st.few)');
        await wait(600);
        const [hx, hy] = await c.evaluate('window.__st.worldToClient(...window.__st.fewHitNow())');
        const drag = async (steps, dx, dy) => {
          await mouse('mouseMoved', hx, hy);
          await mouse('mousePressed', hx, hy, { button: 'left', buttons: 1, clickCount: 1 });
          for (let i = 1; i <= steps; i++) await mouse('mouseMoved', hx + (dx * i) / steps, hy + (dy * i) / steps, { buttons: 1 });
          await wait(300);
        };
        const out = {};
        await drag(8, 60, 40);
        await mouse('mouseReleased', hx + 60, hy + 40, { button: 'left', buttons: 0, clickCount: 1 });
        await wait(500);
        const afterMove = await snap();
        out.mover_10 = same(afterMove, await full());
        await c.evaluate('window.__st.useStore.getState().undo()'); await wait(500);
        const afterUndo = await snap();
        out.deshacer = same(afterUndo, await full());
        await c.evaluate('window.__st.useStore.getState().selectAll()'); await wait(500);
        await drag(5, 30, 20);
        await mouse('mouseReleased', hx + 30, hy + 20, { button: 'left', buttons: 0, clickCount: 1 });
        await wait(800);
        const afterAll = await snap();
        out.mover_todo = same(afterAll, await full());
        return { bytes_distintos: out, bytes_total: afterAll.length };
      });
    }
    res.heap_mb = +(await c.evaluate('(performance.memory?.usedJSHeapSize ?? 0) / 1e6')).toFixed(0);
    res.status = 'ok';
  } catch (e) {
    res.status = /TIMEOUT/.test(String(e.message)) ? 'colgado' : 'error';
    res.error = String(e.message ?? e);
  } finally {
    c.close();
    await wait(500);
  }
  return res;
}

// ---- ejecución y salida --------------------------------------------------------------------------------------------
const results = [];
try {
  for (const sc of scenarios) {
    console.log(`Escenario ${sc.id}`);
    results.push(await runScenario(sc));
    writeFileSync(out, JSON.stringify(results, null, 2));
  }
} finally {
  vite?.kill();
}

const f = (x) => (x === undefined || x === null ? '-' : typeof x === 'number' ? String(x) : x);
const med = (o) => (o && o.median !== undefined ? `${o.median} (p95 ${o.p95}, máx ${o.max})` : '-');
console.log(`\nSuelo del método (dos cuadros sin trabajo): ${results.map((r) => `${r.id} ${med(r.steps.baseline?.frame_ms)}`).join(' · ')} ms`);
console.log('\n| escenario | estado | carga (1er cuadro) | render en carga | paneo ms | zoom + ms | zoom - ms | clic ms | clic JS ms | clics que seleccionaron |');
console.log('|---|---|---|---|---|---|---|---|---|---|');
for (const r of results) {
  const s = r.steps;
  console.log(`| ${r.id} | ${r.status}${r.error ? ' ' + r.error.slice(0, 40) : ''} | ${f(s.load?.first_frame_ms?.toFixed?.(0))} ms | ${f(s.load?.renders?.map((x) => x.toFixed(0)).join('+'))} | ${med(s.pan?.frame_ms)} | ${med(s.zoom?.in_frame_ms)} | ${med(s.zoom?.out_frame_ms)} | ${med(s.click?.frame_ms)} | ${med(s.click?.call_ms)} | ${f(s.click?.selected)}/${f(s.click?.clicks)} |`);
}
console.log('\n| escenario | rectángulo | seleccionados | arrastre ms | soltar ms |');
console.log('|---|---|---|---|---|');
for (const r of results) {
  for (const [k, v] of Object.entries(r.steps.marquee ?? {})) {
    console.log(`| ${r.id} | ${k} | ${v.selected} | ${med(v.drag_frame_ms)} | ${v.release_ms} |`);
  }
}
console.log('\n| escenario | mover: seleccionados (movió) | seleccionar todo ms | arrastre ms | arrastre JS ms | arrastre render ms | confirmar ms | confirmar render ms | confirmar JS ms | deshacer ms | deshacer render ms | moveObjects ms | moveObjects JS ms | solo documento ms | deshacer JS ms | pasos al fundir |');
console.log('|' + '---|'.repeat(16));
for (const r of results) {
  const m = r.steps.move;
  if (m && !m.error) console.log(`| ${r.id} | ${m.selected} (${m.moved}) | ${m.select_all_ms} | ${med(m.drag_frame_ms)} | ${med(m.drag_call_ms)} | ${med(m.drag_render_ms)} | ${m.commit_ms} | ${m.commit_render_ms} | ${m.commit_call_ms} | ${m.undo_ms} | ${m.undo_render_ms} | ${m.move_command_ms} | ${med(m.move_sync_ms)} | ${m.move_pure_ms} | ${m.undo_sync_ms} | ${m.merged_steps} |`);
}
console.log('\n| escenario | hover: puntos | con objeto | cuadro ms | hover JS ms (render) |');
console.log('|---|---|---|---|---|');
for (const r of results) {
  const h = r.steps.hover;
  if (h && !h.error) console.log(`| ${r.id} | ${h.points} | ${h.hovered} | ${med(h.frame_ms)} | ${med(h.render_ms)} |`);
}
console.log('\n| escenario | mover 10 objetos: seleccionados | un solo paso | arrastre ms | arrastre JS ms | confirmar ms | confirmar render ms | confirmar JS ms | deshacer ms | moveObjects JS ms | solo documento ms | empujar ms |');
console.log('|---|---|---|---|---|---|---|---|---|---|---|---|');
for (const r of results) {
  const m = r.steps['move-few'];
  if (m && !m.error) console.log(`| ${r.id} | ${m.selected} | ${m.moved_one_step} | ${med(m.drag_frame_ms)} | ${med(m.drag_call_ms)} | ${m.commit_ms} | ${m.commit_render_ms} | ${m.commit_call_ms} | ${m.undo_ms} | ${med(m.move_sync_ms)} | ${m.move_pure_ms} | ${m.nudge_ms} |`);
}
console.log('\n| escenario | verificación (bytes distintos entre lo parchado y repintar todo): mover 10 | deshacer | mover todo |');
console.log('|---|---|---|---|');
for (const r of results) {
  const v = r.steps.verify;
  if (v && !v.error) console.log(`| ${r.id} | ${v.bytes_distintos.mover_10} | ${v.bytes_distintos.deshacer} | ${v.bytes_distintos.mover_todo} |`);
}
console.log(`\nJSON completo: ${out}`);
