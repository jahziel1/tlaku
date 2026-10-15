// Rendimiento de la vista en vivo con un trabajo grande, en Chrome headless (mismo método que canvas-stress.mjs).
//
//   node scripts/live-bench.mjs                  # relleno de 100 000 y de 400 000 movimientos
//   node scripts/live-bench.mjs --rows 25000     # un solo caso (pasadas de 4 movimientos cada una)
//
// Mide, dentro de la página y con los módulos reales de `src/features/live`:
//   - armar la ruta, trazar lo pendiente (una vez) y poner al día todo lo quemado de golpe (vista encendida a mitad);
//   - una corrida de ~20 s: cada cuadro avanza el seguimiento, traza SOLO lo nuevo en la capa de lo quemado y dibuja
//     (dos drawImage + cabezal) sobre un lienzo de 1440x900. Se reporta el costo de nuestro trabajo por cuadro.
// Chrome corre con rasterización por software (SwiftShader): los tiempos de dibujo son pesimistas frente a una GPU.
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const appDir = join(here, '..');
const args = process.argv.slice(2);
const argOf = (n) => { const i = args.indexOf(n); return i >= 0 ? args[i + 1] : undefined; };
const rowsArg = argOf('--rows');
const cases = rowsArg ? [{ rows: Number(rowsArg) }] : [{ rows: 25000 }, { rows: 100000 }];
const port = 9345;
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const bin = process.env.CHROME ?? ['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', '/usr/bin/google-chrome', '/usr/bin/chromium'].find(existsSync);
if (!bin) throw new Error('No se encontró Chrome/Chromium. Define CHROME=/ruta/al/binario');

let vite = null;
let url = process.env.APP_URL;
if (!url) {
  const vport = 1432;
  vite = spawn('npx', ['vite', '--config', 'scripts/vite.stress.config.mjs', '--port', String(vport), '--strictPort'], { cwd: appDir, stdio: 'ignore' });
  url = `http://localhost:${vport}/`;
  let up = false;
  for (let i = 0; i < 100 && !up; i++) {
    await wait(300);
    try { up = (await fetch(url)).ok; } catch { /* arrancando */ }
  }
  if (!up) { vite.kill(); throw new Error('Vite no arrancó'); }
}

const proc = spawn(bin, [
  '--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), 'ls-live-'))}`,
  '--disable-gpu', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--hide-scrollbars',
  '--window-size=1440,900', '--force-device-scale-factor=1', 'about:blank',
], { stdio: 'ignore' });
let target;
for (let i = 0; i < 50 && !target; i++) {
  await wait(200);
  try { target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === 'page'); } catch { /* arrancando */ }
}
if (!target) { proc.kill(); vite?.kill(); throw new Error('Chrome no respondió'); }
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map();
ws.onmessage = (m) => { const msg = JSON.parse(m.data); if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); } };
const send = (method, params = {}) => new Promise((resolve, reject) => {
  const mid = ++id;
  pending.set(mid, (msg) => (msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result)));
  ws.send(JSON.stringify({ id: mid, method, params }));
});
const evaluate = async (expression) => {
  const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
  return r.result.value;
};

const BENCH = (rows) => `(async () => {
  const { rasterFill } = await import('/src/features/live/testRoutes.ts');
  const { buildLiveRoute, posAtTime } = await import('/src/features/live/route.ts');
  const { LiveLayers } = await import('/src/features/live/layers.ts');
  const { LiveTracker } = await import('/src/features/live/tracker.ts');
  const { drawLive, FALLBACK_PALETTE } = await import('/src/features/live/draw.ts');
  const { buildLivePayload } = await import('/src/features/live/remoteRoute.ts');
  const out = {};
  let t = performance.now();
  const preview = rasterFill(${rows}, 200, 0.05);
  out.moves = preview.moves.length / 4;
  out.gen_ms = +(performance.now() - t).toFixed(0);
  t = performance.now();
  const route = buildLiveRoute(preview);
  out.build_route_ms = +(performance.now() - t).toFixed(0);
  t = performance.now();
  const payload = buildLivePayload(route, [400, 300]);
  out.phone_payload_ms = +(performance.now() - t).toFixed(0);
  out.phone_payload_kb = +(JSON.stringify(payload).length / 1024).toFixed(0);
  out.phone_runs = payload.runs.length;
  const cv = document.createElement('canvas');
  cv.width = 1440; cv.height = 900;
  document.body.append(cv);
  const ctx = cv.getContext('2d');
  const style = { fallbackCut: '#c0392b', fallbackEngrave: '#2e86c1', travel: '#6b5a48' };
  const mk = () => new LiveLayers(route, style, () => document.createElement('canvas'));
  // 1. preparar las capas y ponerse al día de golpe (vista encendida con el trabajo casi al final)
  t = performance.now();
  let layers = mk();
  out.layer_px = layers.geo.width + 'x' + layers.geo.height;
  out.create_layers_ms = +(performance.now() - t).toFixed(0);
  t = performance.now();
  while (!layers.renderPending(1000)) { /* por tandas de 1 s */ }
  out.pending_prerender_ms = +(performance.now() - t).toFixed(0);
  t = performance.now();
  const last = route.count - 1;
  let frames = 0;
  while (!layers.advance(last, 1, 5)) frames++;   // 5 ms por cuadro, como la app
  out.catch_up_ms = +(performance.now() - t).toFixed(0);
  out.catch_up_frames = frames + 1;
  // 2. una corrida de 20 s con el cabezal recorriendo todo el trabajo
  layers = mk();
  while (!layers.renderPending(1000)) { /* ya pre-renderizado */ }
  const trk = new LiveTracker(route.total);
  const view = { zoom: 3.4, panX: 40, panY: 880 };
  const DURATION = 20000, speed = route.total / (DURATION / 1000);
  const work = [], gaps = [];
  const t0 = performance.now();
  let nextReport = 0, prev = t0;
  await new Promise((resolve) => {
    const frame = () => {
      const now = performance.now();
      const el = now - t0;
      const w0 = performance.now();
      if (el >= nextReport) { nextReport += 250; trk.report({ t: (el / 1000) * speed, running: true }, now / 1000); }
      const tt = trk.step(now / 1000);
      const p = posAtTime(route, tt);
      layers.advance(p.move, p.frac, 5);
      drawLive({ ctx, width: 1440, height: 900, view, bed: [400, 300], layers, frame: { t: tt, fraction: tt / route.total, x: 0, y: 0, move: p.move, frac: p.frac, emitting: true, mode: 'running' }, palette: FALLBACK_PALETTE });
      work.push(performance.now() - w0);
      gaps.push(now - prev); prev = now;
      if (el < DURATION) requestAnimationFrame(frame); else resolve();
    };
    requestAnimationFrame(frame);
  });
  const q = (xs, p) => { const s = [...xs].sort((a, b) => a - b); return +s[Math.min(s.length - 1, Math.round((s.length - 1) * p))].toFixed(2); };
  out.run = { frames: work.length, work_ms_median: q(work, 0.5), work_ms_p95: q(work, 0.95), work_ms_max: q(work, 1), frame_gap_ms_median: q(gaps, 0.5), frame_gap_ms_p95: q(gaps, 0.95), frames_over_20ms: gaps.filter((g) => g > 20).length, burned_to: layers.burnedTo.move + '/' + route.count };
  return out;
})()`;

const results = [];
try {
  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await send('Page.navigate', { url });
  await wait(2500);
  for (const c of cases) {
    console.log(`rellenó ${c.rows} pasadas…`);
    const r = await evaluate(BENCH(c.rows));
    results.push(r);
    console.log(JSON.stringify(r, null, 2));
  }
} finally {
  try { ws.close(); } catch { /* nada */ }
  proc.kill('SIGKILL');
  vite?.kill();
}
