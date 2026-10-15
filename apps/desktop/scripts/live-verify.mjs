// Verificación de la vista en vivo en el ESCRITORIO con Chrome headless y la máquina simulada (mock).
// jsdom no dibuja canvas, así que esto comprueba píxeles de verdad y usa el ratón real (CDP):
//
//   node scripts/live-verify.mjs                      # solo verifica
//   node scripts/live-verify.mjs --shots DIR          # además guarda capturas
//
// La vista en vivo es una ventana flotante («imagen en imagen») que NO tapa el lienzo del diseño.
// Con el trabajo de ejemplo (engranaje con relleno, imagen, cortes) comprueba que:
//   (a) el lienzo del diseño NO queda tapado: sus píxeles son los mismos con la vista apagada, encendida,
//       al 30 % y al terminar; en una captura de pantalla un punto del diseño fuera de la ventana flotante
//       sigue igual, y el elemento que está sobre ese punto es el lienzo del diseño;
//   (b) dentro de la ventana flotante lo quemado crece al 30 %, 70 % y 100 %, un punto hecho está quemado y
//       uno que falta no, y lo quemado cae donde el diseño dibuja ese trazo (con la vista propia de la ventana);
//   (c) arrastrar (barra de título), redimensionar (esquina), agrandar/restaurar, zoom con la rueda, arrastrar
//       dentro de la cama, «encajar», que se reacomode al achicar la ventana de la app, y cerrar (= apagar la
//       vista) funcionan con el ratón real, sin tocar el zoom ni el paneo del diseño;
//   (d) la posición y el tamaño se recuerdan tras recargar la página;
//   (e) «Abrir en ventana aparte» abre otra ventana (aquí una pestaña emergente que hace de ventana
//       nativa), la flotante se oculta, la aparte dibuja lo mismo y crece igual, y al cerrarla vuelve la flotante.
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
const PORT = 9349;
const proc = spawn(bin, ['--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), 'ls-verify-'))}`, '--disable-gpu', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--hide-scrollbars', '--window-size=1440,900', '--force-device-scale-factor=1', 'about:blank'], { stdio: 'ignore' });
const targets = async () => (await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()).filter((t) => t.type === 'page');
let first;
for (let i = 0; i < 50 && !first; i++) { await wait(200); try { first = (await targets())[0]; } catch { /* arrancando */ } }
if (!first) { proc.kill(); vite?.kill(); throw new Error('Chrome no respondió'); }

/** Conexión CDP a una pestaña. */
async function connect(target) {
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((r) => (ws.onopen = r));
  let id = 0; const pending = new Map();
  ws.onmessage = (m) => { const msg = JSON.parse(m.data); if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); } };
  const send = (method, params = {}) => new Promise((resolve, reject) => { const mid = ++id; pending.set(mid, (msg) => (msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result))); ws.send(JSON.stringify({ id: mid, method, params })); });
  const evaluate = async (expression) => { const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true }); if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text); return r.result.value; };
  const mouse = (type, x, y, extra = {}) => send('Input.dispatchMouseEvent', { type, x, y, button: 'left', buttons: type === 'mouseReleased' ? 0 : 1, clickCount: 1, ...extra });
  return { ws, send, evaluate, mouse };
}
const main = await connect(first);
const { send, evaluate, mouse } = main;
const shot = async (name, page = main) => { if (!shotsDir) return null; const r = await page.send('Page.captureScreenshot', { format: 'png' }); writeFileSync(join(shotsDir, name), Buffer.from(r.data, 'base64')); console.log('  captura', name); return r.data; };
const failures = [];
const check = (ok, what, extra = '') => { console.log(`  ${ok ? 'OK ' : 'FALLA'} ${what}${extra ? ' · ' + extra : ''}`); if (!ok) failures.push(what); };

/** Clic con el ratón real en el centro de un elemento (selector). */
const clickEl = async (sel, page = main) => {
  const r = await page.evaluate(`(() => { const b = document.querySelector(${JSON.stringify(sel)}).getBoundingClientRect(); return [b.left + b.width / 2, b.top + b.height / 2]; })()`);
  await page.mouse('mouseMoved', r[0], r[1], { buttons: 0 });
  await page.mouse('mousePressed', r[0], r[1]);
  await page.mouse('mouseReleased', r[0], r[1]);
  await wait(120);
};
/** Arrastra con el ratón real desde (x, y) sumando (dx, dy), en pasos. */
const dragFrom = async (x, y, dx, dy, page = main) => {
  await page.mouse('mouseMoved', x, y, { buttons: 0 });
  await page.mouse('mousePressed', x, y);
  for (let i = 1; i <= 8; i++) await page.mouse('mouseMoved', x + (dx * i) / 8, y + (dy * i) / 8);
  await page.mouse('mouseReleased', x + dx, y + dy);
  await wait(200);
};
const rectOf = (sel, page = main) => page.evaluate(`(() => { const e = document.querySelector(${JSON.stringify(sel)}); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.left, y: b.top, w: b.width, h: b.height }; })()`);
const WIN = '[data-testid="live-window"]';
const near = (a, b, tol = 2) => Math.abs(a - b) <= tol;

// Utilidades dentro de la página (la principal).
const SETUP = `(async () => {
  const { useStore } = await import('/src/store/index.ts');
  const { setApi } = await import('/src/api/index.ts');
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
  // Pantalla de la vista propia de la ventana flotante / del lienzo del diseño (px CSS).
  V.lv = () => window.__liveView();
  V.screen = ([x, y]) => { const v = window.__liveView(); return [v.panX + x * v.zoom, v.panY - y * v.zoom]; };
  V.dscreen = ([x, y]) => { const st = s(); return [st.panX + x * st.zoom, st.panY - y * st.zoom]; };
  // Luminancia más baja en un cuadro de (2r+1) px alrededor de un punto de pantalla del lienzo \`cv\`.
  V.minLum = (cv, [sx, sy], cssW, r = 3) => {
    const dpr = cv.width / cssW;
    const x = Math.round(sx * dpr) - r, y = Math.round(sy * dpr) - r;
    const d = cv.getContext('2d').getImageData(Math.max(0, x), Math.max(0, y), 2 * r + 1, 2 * r + 1).data;
    let m = 255;
    for (let i = 0; i < d.length; i += 4) m = Math.min(m, 0.2126 * d[i] + 0.7152 * d[i+1] + 0.0722 * d[i+2]);
    return m;
  };
  V.live = () => document.querySelector('.livewin canvas.live-canvas');
  V.main = () => document.querySelector('#canvas-main');
  V.liveLum = (mm, r = 3) => V.minLum(V.live(), V.screen(mm), V.lv().w, r);
  V.designLum = (mm, r = 3) => V.minLum(V.main(), V.dscreen(mm), s().viewport.width, r);
  // Resumen del lienzo del diseño: suma de comprobación de una muestra de sus píxeles (no depende de la ventana flotante).
  V.designHash = () => {
    const cv = V.main(); const d = cv.getContext('2d').getImageData(0, 0, cv.width, cv.height).data;
    let h = 2166136261;
    for (let i = 0; i < d.length; i += 13) h = Math.imul(h ^ d[i], 16777619) >>> 0;
    return h;
  };
  // Compara el lienzo del diseño con la foto de la línea base: cuántos píxeles cambiaron y qué caja ocupan.
  V.snapBase = () => { const cv = V.main(); V.baseData = cv.getContext('2d').getImageData(0, 0, cv.width, cv.height); };
  V.diffBase = () => {
    const cv = V.main(); const a = V.baseData.data; const b = cv.getContext('2d').getImageData(0, 0, cv.width, cv.height).data;
    let n = 0, x0 = 1e9, y0 = 1e9, x1 = -1, y1 = -1;
    for (let i = 0; i < a.length; i += 4) if (a[i] !== b[i] || a[i+1] !== b[i+1] || a[i+2] !== b[i+2]) { const p = i / 4, x = p % cv.width, y = Math.floor(p / cv.width); n++; x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, y); y1 = Math.max(y1, y); }
    return { n, w: n ? x1 - x0 + 1 : 0, h: n ? y1 - y0 + 1 : 0 };
  };
  // Píxeles «quemados» (oscuros) dentro de la caja del trabajo en la ventana flotante, sin contar el cabezal.
  V.burned = (cv = V.live(), lv = V.lv(), sc = V.screen) => {
    const [[x0, y0], [x1, y1]] = route.bounds;
    const a = sc([x0, y1]), b = sc([x1, y0]);
    const dpr = cv.width / lv.w;
    const px = Math.max(0, Math.floor(a[0] * dpr)), py = Math.max(0, Math.floor(a[1] * dpr));
    const w = Math.min(cv.width - px, Math.ceil((b[0] - a[0]) * dpr)), h = Math.min(cv.height - py, Math.ceil((b[1] - a[1]) * dpr));
    const d = cv.getContext('2d').getImageData(px, py, w, h).data;
    let n = 0;
    for (let i = 0; i < d.length; i += 4) if (0.2126 * d[i] + 0.7152 * d[i+1] + 0.0722 * d[i+2] < 100) n++;
    return n;
  };
  // Qué hay en un punto de la pantalla (el elemento de más arriba).
  V.topAt = (x, y) => { const e = document.elementFromPoint(x, y); return e ? (e.id || e.className || e.tagName) : null; };
  // Un punto del diseño (trazo con láser) cuya posición en pantalla queda FUERA del rectángulo \`r\` y dentro del lienzo.
  V.pointOutside = (r) => {
    const cv = V.main().getBoundingClientRect();
    const n = route.count;
    for (let i = 0; i < 400; i++) {
      const j = Math.floor((i + 0.5) * n / 400);
      if (!isLaserMove(route, j)) continue;
      const p = route.model.pos; const mm = [(p[2*j] + p[2*j+2]) / 2, (p[2*j+1] + p[2*j+3]) / 2];
      const [sx, sy] = V.dscreen(mm); const X = cv.left + sx, Y = cv.top + sy;
      if (X < cv.left + 10 || X > cv.right - 10 || Y < cv.top + 10 || Y > cv.bottom - 10) continue;
      if (X > r.x - 12 && X < r.x + r.w + 12 && Y > r.y - 12 && Y < r.y + r.h + 12) continue;
      if (V.designLum(mm, 1) > 120) continue;
      return { mm, x: X, y: Y };
    }
    return null;
  };
  // Color de un píxel de una captura (PNG en base64) en (x, y) CSS.
  V.shotPixel = (b64, x, y) => new Promise((ok, fail) => { const im = new Image(); im.onload = () => { const c = document.createElement('canvas'); c.width = im.width; c.height = im.height; const g = c.getContext('2d'); g.drawImage(im, 0, 0); ok(Array.from(g.getImageData(Math.round(x), Math.round(y), 1, 1).data.slice(0, 3))); }; im.onerror = fail; im.src = 'data:image/png;base64,' + b64; });
  V.waitFraction = (f) => new Promise((ok, fail) => { const t0 = Date.now(); const tick = () => { if (useLive.getState().fraction >= f) return ok(useLive.getState().fraction); if (Date.now() - t0 > 60000) return fail(new Error('espera ' + f)); setTimeout(tick, 10); }; tick(); });
  V.waitProgress = (f) => new Promise((ok, fail) => { const t0 = Date.now(); const tick = () => { const p = s().progress; if (p && p.total && p.acked / p.total >= f) return ok(p.acked / p.total); if (Date.now() - t0 > 60000) return fail(new Error('espera progreso ' + f)); setTimeout(tick, 10); }; tick(); });
  V.waitDone = () => new Promise((ok, fail) => { const t0 = Date.now(); const tick = () => { if (s().jobState === 'finished') return ok(); if (Date.now() - t0 > 90000) return fail(new Error('no terminó')); setTimeout(tick, 20); }; tick(); });
  return { total: route.total, moves: route.count, bed: s().devices[0].bed, zoom: s().zoom };
})()`;

const open = async () => {
  await send('Page.navigate', { url });
  await wait(3000);
  return evaluate(SETUP);
};

try {
  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  console.log('preparando…', await open());
  if (process.env.DEBUG_EVAL) { console.log(JSON.stringify(await evaluate(process.env.DEBUG_EVAL))); throw new Error('debug'); }
  const A = await evaluate('window.__v.pointAt(0.10)');
  const B = await evaluate('window.__v.pointAt(0.90)');
  const C = await evaluate('window.__v.pointAt(0.50)');
  console.log('puntos de prueba (mm, movimiento):', { A, B, C });

  console.log('0. línea base: el diseño con la vista en vivo apagada');
  const base = await evaluate(`(() => { const V = window.__v; V.snapBase(); return { hash: V.designHash(), a: V.designLum(${JSON.stringify(A)}, 4), bed: V.designLum([395, 410], 1), zoom: V.useStore.getState().zoom, panX: V.useStore.getState().panX, panY: V.useStore.getState().panY }; })()`);
  check(Math.abs(base.a - base.bed) > 6, 'el lienzo del diseño tiene un trazo en el punto A', `luminancia ${base.a.toFixed(0)} vs cama ${base.bed.toFixed(0)}`);
  check(await evaluate('document.querySelector(\'[data-testid="live-window"]\') === null'), 'apagada no hay ventana flotante');
  const baseShot = await shot('00-design-only.png');

  console.log('1. encendida: la ventana flotante aparece sin tapar el diseño');
  await evaluate(`window.__v.useStore.getState().setPref('liveView', true)`);
  await wait(500);
  const w0 = await rectOf(WIN);
  check(!!w0, 'al encender aparece la ventana flotante');
  const area = await rectOf('.canvas-area');
  check(w0.x >= area.x && w0.y >= area.y && w0.x + w0.w <= area.x + area.w + 1 && w0.y + w0.h <= area.y + area.h + 1, 'aparece dentro del área del lienzo, en una esquina', `ventana ${JSON.stringify(w0)} · área ${JSON.stringify(area)}`);
  check(await evaluate(`(() => { const a = document.querySelector('.canvas-area'); return !a.querySelector('.live-canvas, .live-foot, [data-testid="live-window"]') && !a.contains(document.querySelector(${JSON.stringify(WIN)})); })()`), 'el área del lienzo del diseño no contiene nada de la vista en vivo');
  const idle = await evaluate(`window.__v.designHash()`);
  check(idle === base.hash, 'los píxeles del lienzo del diseño son los mismos con la vista encendida (sin trabajo)', `${base.hash} -> ${idle}`);
  await shot('01-float-idle.png');

  console.log('2. trabajo con la vista encendida');
  const probe = await evaluate(`window.__v.pointOutside(${JSON.stringify(w0)})`);
  check(!!probe, 'hay un trazo del diseño fuera de la ventana flotante para comparar en pantalla', JSON.stringify(probe));
  const probeBase = baseShot && probe ? await evaluate(`window.__v.shotPixel(${JSON.stringify(baseShot)}, ${probe.x}, ${probe.y})`) : null;
  await evaluate(`(async () => { await window.__v.useStore.getState().startJob(); })()`);
  await evaluate('window.__v.waitFraction(0.30)');
  const at30 = await evaluate(`(() => { const V = window.__v; return { n: V.burned(), a: V.liveLum(${JSON.stringify(A)}), b: V.liveLum(${JSON.stringify(B)}), frac: V.useLive.getState().fraction, diff: V.diffBase(), dz: V.useStore.getState().zoom, dpx: V.useStore.getState().panX }; })()`);
  const s30 = await shot('02-float-30.png');
  check(at30.a < 100, 'al ~30 % el punto A (ya hecho) está quemado en la ventana', `luminancia ${at30.a.toFixed(0)}`);
  check(at30.b > 120, 'al ~30 % el punto B (falta) no está quemado', `luminancia ${at30.b.toFixed(0)}`);
  check(at30.n > 300, 'al ~30 % hay píxeles quemados en la ventana', `${at30.n}`);
  // Con o sin la vista en vivo, durante un trabajo el diseño dibuja la marca del cabezal de la máquina (un cuadrito); fuera de eso, nada cambia.
  check(at30.diff.n < 600 && at30.diff.w <= 60 && at30.diff.h <= 60, 'a mitad del trabajo (30 %) el lienzo del diseño solo cambia por la marca del cabezal (la misma que con la vista apagada)', `${at30.diff.n} píxeles en ${at30.diff.w}x${at30.diff.h}`);
  check(at30.dz === base.zoom && at30.dpx === base.panX, 'el zoom y el paneo del diseño no cambian');
  if (probe && s30 && probeBase) {
    const px = await evaluate(`window.__v.shotPixel(${JSON.stringify(s30)}, ${probe.x}, ${probe.y})`);
    check(px.every((c, i) => Math.abs(c - probeBase[i]) <= 3), 'en la pantalla, un punto del diseño fuera de la ventana sigue igual que con la vista apagada', `${probeBase} -> ${px}`);
  }
  check(await evaluate(`(() => { const e = document.elementFromPoint(${probe?.x ?? 0}, ${probe?.y ?? 0}); return e && e.id === 'canvas-main'; })()`), 'el elemento que está sobre ese punto del diseño es el lienzo del diseño (no la vista en vivo)');
  await evaluate('window.__v.waitFraction(0.60)');
  await shot('03-float-60.png');
  await evaluate('window.__v.waitFraction(0.70)');
  const at70 = await evaluate(`(() => { const V = window.__v; return { n: V.burned(), a: V.liveLum(${JSON.stringify(A)}), b: V.liveLum(${JSON.stringify(B)}), c: V.liveLum(${JSON.stringify(C)}) }; })()`);
  check(at70.n > at30.n * 1.15, 'de ~30 % a ~70 % crecen los píxeles quemados', `${at30.n} -> ${at70.n}`);
  check(at70.a < 100 && at70.c < 100, 'al ~70 % A y C (hechos) están quemados', `A ${at70.a.toFixed(0)}, C ${at70.c.toFixed(0)}`);
  check(at70.b > 120, 'al ~70 % B (falta) sigue sin quemar', `luminancia ${at70.b.toFixed(0)}`);
  await evaluate('window.__v.waitDone()');
  await wait(700);
  const at100 = await evaluate(`(() => { const V = window.__v; return { n: V.burned(), b: V.liveLum(${JSON.stringify(B)}), frac: V.useLive.getState().fraction, hash: V.designHash() }; })()`);
  await shot('04-float-100.png');
  check(at100.frac === 1, 'al terminar la vista marca 100 %');
  check(at100.b < 100, 'al terminar B también está quemado', `luminancia ${at100.b.toFixed(0)}`);
  check(at100.n > at70.n, 'al terminar hay más píxeles quemados que al 70 %', `${at70.n} -> ${at100.n}`);
  check(at100.hash === base.hash, 'al terminar el lienzo del diseño sigue idéntico', `${base.hash} -> ${at100.hash}`);
  // Lo quemado coincide con el diseño: donde el diseño dibuja un trazo con láser, la ventana lo tiene quemado.
  const match = await evaluate(`(() => { const V = window.__v; const r = V.route; const p = r.model.pos; let ok = 0, tot = 0;
    for (let i = 0; i < 300; i++) { const j = Math.floor((i + 0.5) * r.count / 300); const c = r.model.cls[j]; if (c !== 1 && c !== 2) continue;
      const mm = [(p[2*j] + p[2*j+2]) / 2, (p[2*j+1] + p[2*j+3]) / 2];
      if (V.designLum(mm, 2) > 140) continue; tot++; if (V.liveLum(mm, 2) < 110) ok++; }
    return { ok, tot }; })()`);
  check(match.tot > 40 && match.ok / match.tot >= 0.9, 'lo quemado en la ventana coincide con los trazos del diseño', `${match.ok}/${match.tot}`);
  await crop('06-crop-A-100.png', A);

  console.log('3. arrastrar, redimensionar, agrandar, zoom propio y cerrar (ratón real)');
  const head = await rectOf(`${WIN} .floatwin__head`);
  await dragFrom(head.x + 40, head.y + head.h / 2, -520, -300);
  const w1 = await rectOf(WIN);
  check(near(w1.x, w0.x - 520, 3) && near(w1.y, w0.y - 300, 3), 'arrastrar la barra de título mueve la ventana', `de ${w0.x},${w0.y} a ${w1.x},${w1.y}`);
  check(w1.w === w0.w && w1.h === w0.h, 'mover no cambia el tamaño');
  await shot('07-float-dragged.png');
  const h1 = await rectOf(`${WIN} .floatwin__head`);
  await dragFrom(h1.x + 40, h1.y + h1.h / 2, -3000, -3000);
  const w2 = await rectOf(WIN);
  check(w2.x >= -0.5 && w2.y >= -0.5, 'no se puede sacar de la ventana de la app (esquina superior izquierda)', `${w2.x},${w2.y}`);
  const h2 = await rectOf(`${WIN} .floatwin__head`);
  await dragFrom(h2.x + 40, h2.y + h2.h / 2, 5000, 5000);
  const w3 = await rectOf(WIN);
  check(w3.x + w3.w <= 1440.5 && w3.y + w3.h <= 900.5, 'ni por la esquina inferior derecha', `${w3.x + w3.w},${w3.y + w3.h}`);
  await shot('08-float-other-corner.png');
  // Cambiar el tamaño desde la esquina.
  const g = await rectOf(`${WIN} .floatwin__grip`);
  await dragFrom(g.x + 8, g.y + 8, -100, -80);
  const w4 = await rectOf(WIN);
  check(near(w4.w, w3.w - 100, 3) && near(w4.h, w3.h - 80, 3), 'arrastrar la esquina cambia el tamaño', `${w3.w}x${w3.h} -> ${w4.w}x${w4.h}`);
  const g2 = await rectOf(`${WIN} .floatwin__grip`);
  await dragFrom(g2.x + 8, g2.y + 8, -2000, -2000);
  const w5 = await rectOf(WIN);
  check(w5.w >= 279.5 && w5.h >= 219.5 && w5.w <= 281 && w5.h <= 221, 'tiene un tamaño mínimo razonable', `${w5.w}x${w5.h}`);
  await wait(300);
  check(await evaluate(`(() => { const v = document.querySelector('[data-testid="live-view"]').getBoundingClientRect(); const f = document.querySelector('.live-foot').getBoundingClientRect(); return v.height > 40 && f.height > 20; })()`), 'con el tamaño mínimo la cama y la franja de avance siguen visibles');
  await shot('09-float-small.png');
  // Zoom propio con la rueda y arrastre dentro de la cama; el diseño no se entera.
  const g3 = await rectOf(`${WIN} .floatwin__grip`);
  await dragFrom(g3.x + 8, g3.y + 8, 260, 200);
  await wait(400);
  const fit0 = await evaluate('window.__v.lv()');
  const vr = await rectOf('[data-testid="live-view"]');
  const cx = vr.x + vr.w / 2, cy = vr.y + vr.h / 2;
  await mouse('mouseWheel', cx, cy, { deltaX: 0, deltaY: -600, buttons: 0 });
  await wait(1500);
  const z1 = await evaluate('(() => ({ v: window.__v.lv(), built: window.__liveLayers().builtZoom, dz: window.__v.useStore.getState().zoom, dpx: window.__v.useStore.getState().panX, a: window.__v.liveLum(' + JSON.stringify(A) + ') }))()');
  check(z1.v.zoom > fit0.zoom * 1.5, 'la rueda acerca la cama dentro de la ventana', `zoom ${fit0.zoom.toFixed(2)} -> ${z1.v.zoom.toFixed(2)}`);
  check(z1.dz === base.zoom && z1.dpx === base.panX, 'el zoom del diseño no cambió con la rueda de la ventana');
  check(Math.abs(z1.built / z1.v.zoom - 1) < 0.01, 'las capas se rearmaron al zoom nuevo', `capas a ${z1.built.toFixed(2)}, zoom ${z1.v.zoom.toFixed(2)}`);
  await shot('10-float-zoomed-in.png');
  await dragFrom(cx, cy, 40, -30);
  const z2 = await evaluate('window.__v.lv()');
  check(near(z2.panX, z1.v.panX + 40, 2) && near(z2.panY, z1.v.panY - 30, 2), 'arrastrar dentro de la cama la desplaza', `${z1.v.panX.toFixed(0)},${z1.v.panY.toFixed(0)} -> ${z2.panX.toFixed(0)},${z2.panY.toFixed(0)}`);
  await clickEl(`${WIN} .live-panel__fit`);
  await wait(300);
  const z3 = await evaluate('window.__v.lv()');
  check(near(z3.zoom, fit0.zoom, 0.01), '«Encajar» devuelve la cama completa a la ventana', `zoom ${z3.zoom.toFixed(2)}`);
  const bedOk = await evaluate(`(() => { const v = window.__v.lv(); const dev = window.__v.useStore.getState().devices[0]; const a = window.__v.screen([0, dev.bed[1]]), b = window.__v.screen([dev.bed[0], 0]); return a[0] >= 0 && a[1] >= 0 && b[0] <= v.w && b[1] <= v.h; })()`);
  check(bedOk, 'encajada, la cama completa cabe en la ventana');
  // Agrandar y restaurar.
  const before = await rectOf(WIN);
  await clickEl(`${WIN} button[aria-label="Agrandar"]`);
  await wait(500);
  const mx = await rectOf(WIN);
  const ar = await rectOf('.canvas-area');
  check(mx.w > before.w * 1.5 && mx.w <= ar.w && mx.h <= ar.h + 1 && mx.w >= ar.w - 40, 'agrandar ocupa casi toda el área del lienzo', `${before.w}x${before.h} -> ${mx.w}x${mx.h} (área ${ar.w}x${ar.h})`);
  await shot('11-float-maximized.png');
  check(await evaluate(`window.__v.useStore.getState().prefs.liveWindow.max === true`), 'queda anotado en las preferencias');
  await clickEl(`${WIN} button[aria-label="Restaurar tamaño"]`);
  await wait(400);
  const rs = await rectOf(WIN);
  check(near(rs.x, before.x, 2) && near(rs.y, before.y, 2) && near(rs.w, before.w, 2) && near(rs.h, before.h, 2), 'restaurar vuelve al tamaño y lugar de antes', JSON.stringify(rs));
  // Reacomodo al achicar la ventana de la app.
  await send('Emulation.setDeviceMetricsOverride', { width: 800, height: 600, deviceScaleFactor: 1, mobile: false });
  await wait(600);
  const sm = await rectOf(WIN);
  check(sm.x >= -0.5 && sm.y >= -0.5 && sm.x + sm.w <= 800.5 && sm.y + sm.h <= 600.5, 'al achicar la ventana de la app la flotante se reacomoda dentro', JSON.stringify(sm));
  await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await wait(600);

  console.log('4. se recuerda la posición y el tamaño tras recargar');
  const h3 = await rectOf(`${WIN} .floatwin__head`);
  await dragFrom(h3.x + 40, h3.y + h3.h / 2, -180, -90);
  const saved = await rectOf(WIN);
  const pref = await evaluate(`window.__v.useStore.getState().prefs.liveWindow`);
  check(!!pref && near(pref.x, saved.x, 1) && near(pref.w, saved.w, 1), 'la preferencia liveWindow guarda posición y tamaño', JSON.stringify(pref));
  await wait(700);
  await evaluate(`new Promise((r) => setTimeout(r, 600))`);
  await open();
  await wait(800);
  const after = await rectOf(WIN);
  check(!!after && near(after.x, saved.x, 1) && near(after.y, saved.y, 1) && near(after.w, saved.w, 1) && near(after.h, saved.h, 1), 'tras recargar la ventana aparece donde se dejó', `${JSON.stringify(saved)} -> ${JSON.stringify(after)}`);

  console.log('5. cerrar apaga la vista');
  await clickEl(`${WIN} button[aria-label="Cerrar la vista en vivo"]`);
  await wait(300);
  check(await evaluate('document.querySelector(\'[data-testid="live-window"]\') === null && window.__v.useStore.getState().prefs.liveView === false'), 'cerrar quita la ventana y apaga la vista (la misma preferencia del interruptor)');
  const off = await evaluate('window.__v.designHash()');
  check(off === base.hash, 'apagada, el lienzo del diseño sigue igual que al principio');

  console.log('6. encendida a mitad de un trabajo (segunda corrida, con la vista apagada al empezar)');
  await evaluate(`(async () => { const V = window.__v; V.useStore.getState().fitContent(); await V.useStore.getState().startJob(); })()`);
  await evaluate('window.__v.waitProgress(0.45)');
  check(await evaluate('window.__v.getLiveService().route === null'), 'apagada no se armó la ruta ni se siguió el trabajo');
  await evaluate(`window.__v.useStore.getState().setPref('liveView', true)`);
  await wait(900);
  const mid = await evaluate(`(() => { const V = window.__v; return { a: V.liveLum(${JSON.stringify(A)}), b: V.liveLum(${JSON.stringify(B)}), n: V.burned(), frac: V.useLive.getState().fraction }; })()`);
  await shot('12-float-midjob-on.png');
  check(mid.a < 100, 'activada a mitad el punto A (hecho) aparece quemado enseguida', `luminancia ${mid.a.toFixed(0)}, avance ${(mid.frac * 100).toFixed(0)} %`);
  check(mid.b > 120, 'activada a mitad el punto B (falta) no está quemado', `luminancia ${mid.b.toFixed(0)}`);
  check(mid.n > 300, 'activada a mitad hay píxeles quemados', `${mid.n}`);
  await evaluate('window.__v.waitDone()');

  console.log('7. ventana aparte (otra ventana nativa; aquí una pestaña emergente)');
  await evaluate(`(async () => { const V = window.__v; V.useStore.getState().fitContent(); await V.useStore.getState().startJob(); })()`);
  await evaluate('window.__v.waitFraction(0.15)');
  await clickEl(`${WIN} button[aria-label="Abrir en ventana aparte"]`);
  let popTarget = null;
  for (let i = 0; i < 40 && !popTarget; i++) { await wait(250); popTarget = (await targets()).find((t) => t.url.includes('liveWindow=1')); }
  check(!!popTarget, 'se abre una ventana aparte');
  if (popTarget) {
    const pop = await connect(popTarget);
    await pop.send('Page.enable');
    await pop.send('Emulation.setDeviceMetricsOverride', { width: 900, height: 640, deviceScaleFactor: 1, mobile: false });
    await wait(1500);
    check(await evaluate('document.querySelector(\'[data-testid="live-window"]\') === null'), 'en la ventana principal la flotante se oculta mientras la aparte está abierta');
    check(await pop.evaluate('!!document.querySelector("canvas.live-canvas") && !!document.querySelector(".live-foot")'), 'la ventana aparte muestra la cama y la franja de avance');
    check(await pop.evaluate(`(() => { const s = document.querySelectorAll('.floatwin, .shell, #canvas-main'); return s.length === 0; })()`), 'la ventana aparte no trae el diseño, el panel ni la interfaz de la app');
    const bed = await evaluate('window.__v.useStore.getState().devices[0].bed');
    // Píxeles oscuros dentro de la cama (el fondo de la ventana también es oscuro, por eso solo se cuenta la cama).
    const popBurn = () => pop.evaluate(`(() => {
      const lv = window.__liveView(); const cv = document.querySelector('canvas.live-canvas'); const dpr = cv.width / lv.w;
      const x0 = Math.max(0, Math.ceil(lv.panX * dpr)), y0 = Math.max(0, Math.ceil((lv.panY - ${bed[1]} * lv.zoom) * dpr));
      const w = Math.min(cv.width - x0, Math.floor(${bed[0]} * lv.zoom * dpr)), h = Math.min(cv.height - y0, Math.floor(${bed[1]} * lv.zoom * dpr));
      const d = cv.getContext('2d').getImageData(x0, y0, w, h).data; let k = 0;
      for (let i = 0; i < d.length; i += 4) if (0.2126 * d[i] + 0.7152 * d[i+1] + 0.0722 * d[i+2] < 100) k++;
      return { n: k, zoom: lv.zoom, w: lv.w, h: lv.h, pct: (document.querySelector('.live-foot__pct') || {}).textContent };
    })()`);
    await evaluate('window.__v.waitFraction(0.35)');
    await wait(500);
    const p35 = await popBurn();
    await shot('13-detached-35.png', pop);
    await evaluate('window.__v.waitFraction(0.80)');
    await wait(500);
    const p80 = await popBurn();
    await shot('14-detached-80.png', pop);
    check(p35.n > 300 && p80.n > p35.n * 1.3, 'en la ventana aparte lo quemado crece con el avance', `${p35.n} -> ${p80.n} · ${p35.pct} -> ${p80.pct}`);
    const mainPct = await evaluate('Math.round(window.__v.useLive.getState().fraction * 100)');
    const shownPct = parseInt(p80.pct, 10);
    check(Math.abs(shownPct - mainPct) <= 6, 'el porcentaje de la ventana aparte sigue al de la principal', `${shownPct} % vs ${mainPct} %`);
    await shot('15-main-while-detached.png');
    await evaluate('window.__v.waitDone()');
    await wait(600);
    const pEnd = await popBurn();
    check(pEnd.pct && pEnd.pct.startsWith('100'), 'al terminar la ventana aparte marca 100 %', pEnd.pct);
    await shot('16-detached-100.png', pop);
    // «Volver a la ventana flotante» cierra la aparte y vuelve la flotante.
    await clickEl('.live-ext__back', pop);
    let back = false;
    for (let i = 0; i < 30 && !back; i++) { await wait(200); back = await evaluate('document.querySelector(\'[data-testid="live-window"]\') !== null'); }
    check(back, 'al cerrar la ventana aparte vuelve la flotante (la vista sigue activa)');
    await shot('17-back-to-float.png');
    try { pop.ws.close(); } catch { /* ya cerrada */ }
  }
} finally {
  try { main.ws.close(); } catch { /* nada */ }
  proc.kill('SIGKILL');
  vite?.kill();
}
if (failures.length) { console.log(`\n${failures.length} comprobaciones fallaron`); process.exit(1); }
console.log('\nTodas las comprobaciones pasaron');

/** Recorte ampliado de la ventana flotante alrededor de un punto del documento. */
async function crop(name, pt, size = 100, scale = 4) {
  if (!shotsDir) return;
  const data = await evaluate(`(() => { const V = window.__v; const lv = V.live(); const [sx, sy] = V.screen(${JSON.stringify(pt)}); const dpr = lv.width / V.lv().w; const t = document.createElement('canvas'); t.width = ${size * scale}; t.height = ${size * scale}; const c = t.getContext('2d'); c.imageSmoothingEnabled = false; c.drawImage(lv, sx * dpr - ${size / 2}, sy * dpr - ${size / 2}, ${size}, ${size}, 0, 0, ${size * scale}, ${size * scale}); return t.toDataURL('image/png'); })()`);
  writeFileSync(join(shotsDir, name), Buffer.from(data.split(',')[1], 'base64'));
  console.log('  recorte', name);
}
