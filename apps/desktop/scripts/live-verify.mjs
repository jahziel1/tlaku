// Verificación de la vista en vivo en el ESCRITORIO con Chrome headless y la máquina simulada (mock).
// jsdom no dibuja canvas, así que esto comprueba píxeles de verdad:
//
//   node scripts/live-verify.mjs                      # solo verifica
//   node scripts/live-verify.mjs --shots DIR          # además guarda capturas (30 %, 60 %, 100 %, a mitad de trabajo, con zoom)
//
// Con el trabajo de ejemplo (engranaje con relleno, imagen, cortes) y la vista en vivo encendida comprueba que:
//   - el diseño de debajo (objetos, tiradores, recuadros) queda tapado: la cama del lienzo en vivo es opaca;
//   - al ~30 % y al ~70 % crece la cantidad de píxeles «quemados» (oscuros) dentro del área del trabajo;
//   - un punto sobre una línea que ya se hizo (10 % del recorrido) está quemado y otro que falta (90 %) no lo está,
//     y esos puntos caen donde el diseño dibuja ese trazo (se mira también el lienzo del diseño sin la vista);
//   - al terminar (100 %) lo último también está quemado;
//   - con zoom y paneo lo quemado sigue alineado (se rearman las capas al cambiar mucho el zoom);
//   - encendida a mitad de un trabajo se ve lo ya hecho de inmediato.
// Código de salida 1 si algo falla. Variables: CHROME, APP_URL.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const appDir = join(here, '..');
const args = process.argv.slice(2);
const shotsDir = args.includes('--shots') ? args[args.indexOf('--shots') + 1] : null;
if (shotsDir) mkdirSync(shotsDir, { recursive: true });
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const bin = process.env.CHROME ?? ['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', '/usr/bin/google-chrome', '/usr/bin/chromium'].find(existsSync);
if (!bin) throw new Error('No se encontró Chrome/Chromium. Define CHROME=/ruta/al/binario');

let vite = null;
let url = process.env.APP_URL;
if (!url) {
  vite = spawn('npx', ['vite', '--config', 'scripts/vite.stress.config.mjs', '--port', '1434', '--strictPort'], { cwd: appDir, stdio: 'ignore' });
  url = 'http://localhost:1434/';
  let up = false;
  for (let i = 0; i < 100 && !up; i++) { await wait(300); try { up = (await fetch(url)).ok; } catch { /* arrancando */ } }
  if (!up) { vite.kill(); throw new Error('Vite no arrancó'); }
}
const proc = spawn(bin, ['--headless=new', '--remote-debugging-port=9349', `--user-data-dir=${mkdtempSync(join(tmpdir(), 'ls-verify-'))}`, '--disable-gpu', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--hide-scrollbars', '--window-size=1440,900', '--force-device-scale-factor=1', 'about:blank'], { stdio: 'ignore' });
let target;
for (let i = 0; i < 50 && !target; i++) { await wait(200); try { target = (await (await fetch('http://127.0.0.1:9349/json')).json()).find((t) => t.type === 'page'); } catch { /* arrancando */ } }
if (!target) { proc.kill(); vite?.kill(); throw new Error('Chrome no respondió'); }
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0; const pending = new Map();
ws.onmessage = (m) => { const msg = JSON.parse(m.data); if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); } };
const send = (method, params = {}) => new Promise((resolve, reject) => { const mid = ++id; pending.set(mid, (msg) => (msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result))); ws.send(JSON.stringify({ id: mid, method, params })); });
const evaluate = async (expression) => { const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true }); if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text); return r.result.value; };
const shot = async (name) => { if (!shotsDir) return; const r = await send('Page.captureScreenshot', { format: 'png' }); writeFileSync(join(shotsDir, name), Buffer.from(r.data, 'base64')); console.log('  captura', name); };

const crop = async (name, pt, size = 120, scale = 4) => {
  if (!shotsDir) return;
  const url = await evaluate(`(() => { const V = window.__v; const lv = V.live(); const [sx, sy] = V.screen(${JSON.stringify(pt)}); const t = document.createElement('canvas'); t.width = ${size * scale}; t.height = ${size * scale}; const c = t.getContext('2d'); c.imageSmoothingEnabled = false; c.drawImage(lv, sx - ${size / 2}, sy - ${size / 2}, ${size}, ${size}, 0, 0, ${size * scale}, ${size * scale}); return t.toDataURL('image/png'); })()`);
  writeFileSync(join(shotsDir, name), Buffer.from(url.split(',')[1], 'base64'));
  console.log('  recorte', name);
};
const failures = [];
const check = (ok, what, extra = '') => { console.log(`  ${ok ? 'OK ' : 'FALLA'} ${what}${extra ? ' · ' + extra : ''}`); if (!ok) failures.push(what); };

// Utilidades dentro de la página.
const SETUP = `(async () => {
  const { useStore } = await import('/src/store/index.ts');
  const { setApi, getApi } = await import('/src/api/index.ts');
  const { createMockApi } = await import('/src/api/mock/index.ts');
  const { startLive, getLiveService, useLive } = await import('/src/features/live/service.ts');
  const { buildLiveRoute, posAtTime, isLaserMove } = await import('/src/features/live/route.ts');
  const { bindMachineEvents } = await import('/src/store/index.ts');
  const V = (window.__v = { useStore, useLive, getLiveService });
  const s = () => useStore.getState();
  // Máquina simulada rápida: el trabajo dura ~20 s y informa cada 200 ms (como una máquina real).
  const api = createMockApi({ latencyMs: 0, statusIntervalMs: 200, minJobMs: 20000, jobSpeedup: 1e6 });
  setApi(api);
  await bindMachineEvents(api, useStore);
  V.stopLive = startLive(useStore);
  s().setPref('uiMode', 'full');
  document.querySelectorAll('.toast button').forEach((b) => b.click());
  await s().loadDevices(); await s().refreshPorts(); s().setSelectedPort('virtual:falcon2'); await s().connect();
  await new Promise((r) => setTimeout(r, 600));
  s().setDockTab && s().setDockTab('machine');
  await s().computePreview();
  s().fitContent();
  await new Promise((r) => setTimeout(r, 400));
  const route = buildLiveRoute(s().preview);
  V.route = route;
  // Un punto sobre un trazo con láser cerca del instante \`f\` del recorrido (mm del documento).
  V.pointAt = (f) => {
    const t = f * route.total;
    let k = posAtTime(route, t).move;
    for (let d = 0; d < route.count; d++) {
      for (const j of [k + d, k - d]) if (j >= 0 && j < route.count && isLaserMove(route, j)) {
        const p = route.model.pos; return [(p[2*j] + p[2*j+2]) / 2, (p[2*j+1] + p[2*j+3]) / 2, j];
      }
    }
    return null;
  };
  V.screen = ([x, y]) => { const st = s(); return [st.panX + x * st.zoom, st.panY - y * st.zoom]; };
  // Luminancia más baja en un cuadro de 7x7 px alrededor de un punto del lienzo \`cv\`.
  V.minLum = (cv, [sx, sy], r = 3) => {
    const dpr = cv.width / (s().viewport.width || cv.width);
    const x = Math.round(sx * dpr) - r, y = Math.round(sy * dpr) - r;
    const d = cv.getContext('2d').getImageData(Math.max(0, x), Math.max(0, y), 2 * r + 1, 2 * r + 1).data;
    let m = 255;
    for (let i = 0; i < d.length; i += 4) m = Math.min(m, 0.2126 * d[i] + 0.7152 * d[i+1] + 0.0722 * d[i+2]);
    return m;
  };
  V.live = () => document.querySelector('canvas.live-canvas');
  V.main = () => document.querySelector('#canvas-main');
  // Píxeles «quemados» (oscuros) dentro de la caja del trabajo, sin contar el cabezal.
  V.burned = () => {
    const cv = V.live(); const st = s();
    const [[x0, y0], [x1, y1]] = route.bounds;
    const a = V.screen([x0, y1]), b = V.screen([x1, y0]);
    const dpr = cv.width / st.viewport.width;
    const px = Math.max(0, Math.floor(a[0] * dpr)), py = Math.max(0, Math.floor(a[1] * dpr));
    const w = Math.min(cv.width - px, Math.ceil((b[0] - a[0]) * dpr)), h = Math.min(cv.height - py, Math.ceil((b[1] - a[1]) * dpr));
    const d = cv.getContext('2d').getImageData(px, py, w, h).data;
    let n = 0;
    for (let i = 0; i < d.length; i += 4) if (0.2126 * d[i] + 0.7152 * d[i+1] + 0.0722 * d[i+2] < 100) n++;
    return n;
  };
  // La cama del lienzo en vivo es opaca (tapa el diseño de debajo): fracción de píxeles opacos dentro de la cama.
  V.opaque = () => {
    const cv = V.live(); const st = s(); const dev = st.devices.find((x) => x.id === st.activeDeviceId) ?? st.devices[0];
    const a = V.screen([0, dev.bed[1]]), b = V.screen([dev.bed[0], 0]);
    const dpr = cv.width / st.viewport.width;
    const px = Math.max(0, Math.ceil(a[0] * dpr)), py = Math.max(0, Math.ceil(a[1] * dpr));
    const w = Math.min(cv.width - px, Math.floor((b[0] - a[0]) * dpr)), h = Math.min(cv.height - py, Math.floor((b[1] - a[1]) * dpr));
    const d = cv.getContext('2d').getImageData(px, py, w, h).data;
    let n = 0, tot = 0;
    for (let i = 3; i < d.length; i += 4) { tot++; if (d[i] >= 250) n++; }
    return n / tot;
  };
  V.waitFraction = (f) => new Promise((ok, fail) => { const t0 = Date.now(); const tick = () => { if (useLive.getState().fraction >= f) return ok(useLive.getState().fraction); if (Date.now() - t0 > 60000) return fail(new Error('espera ' + f)); setTimeout(tick, 10); }; tick(); });
  V.waitProgress = (f) => new Promise((ok, fail) => { const t0 = Date.now(); const tick = () => { const p = s().progress; if (p && p.total && p.acked / p.total >= f) return ok(p.acked / p.total); if (Date.now() - t0 > 60000) return fail(new Error('espera progreso ' + f)); setTimeout(tick, 10); }; tick(); });
  V.waitDone = () => new Promise((ok, fail) => { const t0 = Date.now(); const tick = () => { if (s().jobState === 'finished') return ok(); if (Date.now() - t0 > 90000) return fail(new Error('no terminó')); setTimeout(tick, 20); }; tick(); });
  return { total: route.total, moves: route.count, bed: s().devices[0].bed, zoom: s().zoom };
})()`;

try {
  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await send('Page.navigate', { url });
  await wait(3000);
  console.log('preparando…', await evaluate(SETUP));
  if (process.env.DEBUG_EVAL) { console.log(JSON.stringify(await evaluate(process.env.DEBUG_EVAL))); throw new Error('debug'); }
  const A = await evaluate('window.__v.pointAt(0.10)');
  const B = await evaluate('window.__v.pointAt(0.90)');
  const C = await evaluate('window.__v.pointAt(0.50)');
  console.log('puntos de prueba (mm, movimiento):', { A, B, C });

  console.log('1. el diseño dibuja ese trazo (lienzo del diseño, sin la vista en vivo)');
  const designLum = await evaluate(`(() => { const V = window.__v; return V.minLum(V.main(), V.screen(${JSON.stringify(A)}), 4); })()`);
  const bedLum = await evaluate(`(() => { const V = window.__v; return V.minLum(V.main(), V.screen([395, 410]), 1); })()`);
  check(Math.abs(designLum - bedLum) > 6, 'el lienzo del diseño tiene un trazo en el punto A', `luminancia ${designLum.toFixed(0)} vs cama ${bedLum.toFixed(0)}`);
  await shot('00-design-only.png');

  console.log('2. trabajo con la vista en vivo encendida');
  await evaluate(`(async () => { const V = window.__v; V.useStore.getState().setPref('liveView', true); await V.useStore.getState().startJob(); })()`);
  await evaluate('window.__v.waitFraction(0.30)');
  const at30 = await evaluate(`(() => { const V = window.__v; const lv = V.live(); return { n: V.burned(), a: V.minLum(lv, V.screen(${JSON.stringify(A)})), b: V.minLum(lv, V.screen(${JSON.stringify(B)})), opaque: V.opaque(), frac: V.useLive.getState().fraction }; })()`);
  await shot('01-desktop-30.png');
  check(at30.opaque > 0.99, 'la cama de la vista en vivo es opaca (el diseño y sus tiradores quedan tapados)', `${(at30.opaque * 100).toFixed(1)} %`);
  check(at30.a < 100, 'al ~30 % el punto A (ya hecho) está quemado', `luminancia ${at30.a.toFixed(0)}`);
  check(at30.b > 120, 'al ~30 % el punto B (falta) no está quemado', `luminancia ${at30.b.toFixed(0)}`);
  check(at30.n > 500, 'al ~30 % hay píxeles quemados en el área del trabajo', `${at30.n}`);
  await evaluate('window.__v.waitFraction(0.60)');
  await shot('02-desktop-60.png');
  await evaluate('window.__v.waitFraction(0.70)');
  const at70 = await evaluate(`(() => { const V = window.__v; const lv = V.live(); return { n: V.burned(), a: V.minLum(lv, V.screen(${JSON.stringify(A)})), b: V.minLum(lv, V.screen(${JSON.stringify(B)})), c: V.minLum(lv, V.screen(${JSON.stringify(C)})) }; })()`);
  check(at70.n > at30.n * 1.15, 'de ~30 % a ~70 % crecen los píxeles quemados', `${at30.n} -> ${at70.n}`);
  check(at70.a < 100 && at70.c < 100, 'al ~70 % A y C (hechos) están quemados', `A ${at70.a.toFixed(0)}, C ${at70.c.toFixed(0)}`);
  check(at70.b > 120, 'al ~70 % B (falta) sigue sin quemar', `luminancia ${at70.b.toFixed(0)}`);
  await evaluate('window.__v.waitDone()');
  await wait(600);
  if (process.env.DEBUG_AFTER) { console.log(JSON.stringify(await evaluate(process.env.DEBUG_AFTER))); }
  const at100 = await evaluate(`(() => { const V = window.__v; const lv = V.live(); return { n: V.burned(), b: V.minLum(lv, V.screen(${JSON.stringify(B)})), frac: V.useLive.getState().fraction }; })()`);
  await shot('03-desktop-100.png');
  await crop('03b-crop-A-100.png', A);
  check(at100.frac === 1, 'al terminar la vista marca 100 %');
  check(at100.b < 100, 'al terminar B también está quemado', `luminancia ${at100.b.toFixed(0)}`);
  check(at100.n > at70.n, 'al terminar hay más píxeles quemados que al 70 %', `${at70.n} -> ${at100.n}`);

  console.log('3. zoom y desplazamiento: lo quemado sigue alineado');
  const zoomed = await evaluate(`(async () => { const V = window.__v; const st = V.useStore.getState(); const p = V.screen(${JSON.stringify(A)}); st.zoomAt(2.6, p[0], p[1]); await new Promise((r) => setTimeout(r, 1500)); const lv = V.live(); const q = V.screen(${JSON.stringify(A)}); return { zoom: V.useStore.getState().zoom, built: window.__liveLayers().builtZoom, a: V.minLum(lv, q), inside: q[0] > 0 && q[0] < lv.width && q[1] > 0 && q[1] < lv.height }; })()`);
  await shot('05-desktop-zoom-in.png');
  check(zoomed.inside && zoomed.a < 100, 'con zoom ×2,6 el punto A sigue quemado en su sitio (capas rearmadas)', `zoom ${zoomed.zoom.toFixed(2)}, luminancia ${zoomed.a.toFixed(0)}`);
  check(Math.abs(zoomed.built / zoomed.zoom - 1) < 0.01, 'las capas se rearmaron al zoom nuevo', `capas a ${zoomed.built.toFixed(2)}, zoom ${zoomed.zoom.toFixed(2)}`);
  const panned = await evaluate(`(async () => { const V = window.__v; V.useStore.getState().panBy(60, -40); await new Promise((r) => setTimeout(r, 500)); const lv = V.live(); return V.minLum(lv, V.screen(${JSON.stringify(A)})); })()`);
  check(panned < 100, 'tras desplazar el lienzo el punto A sigue quemado en su sitio', `luminancia ${panned.toFixed(0)}`);
  await evaluate(`(async () => { const V = window.__v; V.useStore.getState().fitContent(); V.useStore.getState().zoomAt(0.3); await new Promise((r) => setTimeout(r, 1500)); })()`);
  const out = await evaluate(`(() => { const V = window.__v; return V.minLum(V.live(), V.screen(${JSON.stringify(A)}), 2); })()`);
  await shot('06-desktop-zoom-out.png');
  check(out < 140, 'con zoom ×0,3 el punto A sigue marcado', `luminancia ${out.toFixed(0)}`);

  console.log('4. apagada: el diseño vuelve como estaba');
  await evaluate(`(async () => { const V = window.__v; V.useStore.getState().setPref('liveView', false); V.useStore.getState().fitContent(); await new Promise((r) => setTimeout(r, 500)); })()`);
  check(await evaluate('document.querySelector("canvas.live-canvas") === null'), 'sin la vista en vivo no queda ningún lienzo ni cuadro encima');

  console.log('5. encendida a mitad de un trabajo (segunda corrida, con la vista apagada al empezar)');
  await evaluate(`(async () => { const V = window.__v; V.useStore.getState().fitContent(); await V.useStore.getState().startJob(); })()`);
  await evaluate('window.__v.waitProgress(0.45)');
  check(await evaluate('window.__v.getLiveService().route === null'), 'apagada no se armó la ruta ni se siguió el trabajo');
  await evaluate(`window.__v.useStore.getState().setPref('liveView', true)`);
  await wait(900);
  const mid = await evaluate(`(() => { const V = window.__v; const lv = V.live(); return { a: V.minLum(lv, V.screen(${JSON.stringify(A)})), b: V.minLum(lv, V.screen(${JSON.stringify(B)})), n: V.burned(), frac: V.useLive.getState().fraction }; })()`);
  await shot('04-desktop-midjob-on.png');
  check(mid.a < 100, 'activada a mitad el punto A (hecho) aparece quemado enseguida', `luminancia ${mid.a.toFixed(0)}, avance ${(mid.frac * 100).toFixed(0)} %`);
  check(mid.b > 120, 'activada a mitad el punto B (falta) no está quemado', `luminancia ${mid.b.toFixed(0)}`);
  check(mid.n > 500, 'activada a mitad hay píxeles quemados', `${mid.n}`);
  await evaluate('window.__v.waitDone()');
} finally {
  try { ws.close(); } catch { /* nada */ }
  proc.kill('SIGKILL');
  vite?.kill();
}
if (failures.length) { console.log(`\n${failures.length} comprobaciones fallaron`); process.exit(1); }
console.log('\nTodas las comprobaciones pasaron');
