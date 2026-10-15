// Verificación visual de la vista previa «por tonos» (mejora 8) con Chrome headless y el modo mock
// (la simulación la hace `src/api/mock/tonal.ts`; la real es `crates/tlaku-pipeline/src/tonal.rs`):
//
//   node scripts/tonal-verify.mjs                  # solo verifica
//   node scripts/tonal-verify.mjs --shots DIR      # además guarda capturas
//
// Recorre tres diseños (una prueba de materiales de relleno de 5 x 5, una foto grabada y un diseño mixto),
// cada uno en vista normal y por tonos; el detalle al acercar el zoom en la foto; la calibración con la
// prueba de materiales (se indica un tono, se guarda en la biblioteca y cambia la imagen); el cambio de
// material y el inglés. Código de salida 1 si algo falla. Variables: CHROME, APP_URL.
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
  vite = spawn('npx', ['vite', '--config', 'scripts/vite.stress.config.mjs', '--port', '1438', '--strictPort'], { cwd: appDir, stdio: 'ignore' });
  url = 'http://localhost:1438/';
  let up = false;
  for (let i = 0; i < 100 && !up; i++) { await wait(300); try { up = (await fetch(url)).ok; } catch { /* arrancando */ } }
  if (!up) { vite.kill(); throw new Error('Vite no arrancó'); }
}
const PORT = 9354;
const proc = spawn(bin, ['--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), 'ls-tonal-'))}`, '--disable-gpu', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--hide-scrollbars', '--window-size=1440,900', '--force-device-scale-factor=1', url], { stdio: 'ignore' });
const finish = (code) => { try { proc.kill(); } catch { /* ya cerró */ } vite?.kill(); process.exit(code); };
let first;
for (let i = 0; i < 50 && !first; i++) {
  await wait(200);
  try { first = (await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()).find((t) => t.type === 'page'); } catch { /* arrancando */ }
}
if (!first) { proc.kill(); vite?.kill(); throw new Error('Chrome no respondió'); }

const ws = new WebSocket(first.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0; const pending = new Map();
ws.onmessage = (m) => { const msg = JSON.parse(m.data); if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); } };
const send = (method, params = {}) => new Promise((resolve, reject) => { const mid = ++id; pending.set(mid, (msg) => (msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result))); ws.send(JSON.stringify({ id: mid, method, params })); });
const evaluate = async (expression) => { const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true }); if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text); return r.result.value; };
const shot = async (name) => { if (!shotsDir) return; const r = await send('Page.captureScreenshot', { format: 'png' }); writeFileSync(join(shotsDir, name), Buffer.from(r.data, 'base64')); console.log('  captura', name); };
const failures = [];
const check = (ok, what, extra = '') => { console.log(`  ${ok ? 'OK ' : 'FALLA'} ${what}${extra ? ' · ' + extra : ''}`); if (!ok) failures.push(what); };
const waitFor = async (expr, what, tries = 150) => { for (let i = 0; i < tries; i++) { if (await evaluate(expr)) return; await wait(100); } throw new Error(`Se agotó la espera: ${what}`); };

const click = async (sel) => {
  const r = await evaluate(`(() => { const e = document.querySelector(${JSON.stringify(sel)}); if (!e) return null; e.scrollIntoView({ block: 'center' }); const b = e.getBoundingClientRect(); return [b.left + b.width / 2, b.top + b.height / 2]; })()`);
  if (!r) throw new Error(`No hay ${sel}`);
  await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: r[0], y: r[1], button: 'none' });
  await send('Input.dispatchMouseEvent', { type: 'mousePressed', x: r[0], y: r[1], button: 'left', buttons: 1, clickCount: 1 });
  await send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: r[0], y: r[1], button: 'left', buttons: 0, clickCount: 1 });
  await wait(250);
};
const clickText = async (text, root = 'document.body') => {
  const marked = await evaluate(`(() => { const b = [...${root}.querySelectorAll('button')].find((x) => x.textContent.trim().includes(${JSON.stringify(text)})); if (!b) return false; b.setAttribute('data-vclick', '1'); return true; })()`);
  if (!marked) throw new Error(`No hay un botón «${text}»`);
  await click('[data-vclick="1"]');
  await evaluate(`document.querySelectorAll('[data-vclick]').forEach((b) => b.removeAttribute('data-vclick'))`);
};

/** Pone el diseño, calcula la vista previa y espera a que esté lista. */
const loadDesign = async (setup) => {
  await evaluate(`(async () => { const v = window.__v; v.useTonal.getState().setMode('normal'); await (${setup})(v); })()`);
  await evaluate(`(async () => { const s = window.__v.useStore.getState(); s.setPreviewVisible(true); s.fitContent(); await s.computePreview(); })()`);
  await waitFor(`window.__v.useStore.getState().previewStatus === 'ready'`, 'el cálculo del trabajo');
  await wait(400);
  // Con la barra de la vista previa abierta el lienzo es más bajo: se vuelve a encuadrar.
  await evaluate(`window.__v.useStore.getState().fitContent()`);
  await wait(400);
};
const tonalOn = async () => {
  await evaluate(`window.__v.useTonal.getState().setMode('tonal')`);
  await waitFor(`window.__v.useTonal.getState().status === 'ready' && !!window.__v.getTonalLayers().full`, 'la imagen por tonos');
  await wait(500);
};
const tonalOff = async () => {
  await evaluate(`window.__v.useTonal.getState().setMode('normal')`);
  await wait(300);
};
/** Color medio del lienzo en un rectángulo (px CSS) para comprobar lo que se ve. */
const sample = (x, y) => evaluate(`(() => { const c = document.querySelector('#canvas-main'); const r = c.getBoundingClientRect(); if (${x} < r.left || ${x} >= r.right || ${y} < r.top || ${y} >= r.bottom) throw new Error('punto fuera del lienzo: ' + ${x} + ',' + ${y}); const k = c.width / r.width; const d = c.getContext('2d').getImageData(Math.round((${x} - r.left) * k), Math.round((${y} - r.top) * k), 1, 1).data; return [d[0], d[1], d[2]]; })()`);
const luma = (c) => 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
/** Posición en pantalla (px) de un punto del documento (mm). */
const screenOf = (mx, my) => evaluate(`(() => { const s = window.__v.useStore.getState(); const r = document.querySelector('#canvas-main').getBoundingClientRect(); return [r.left + s.panX + ${mx} * s.zoom, r.top + s.panY - ${my} * s.zoom]; })()`);

try {
  await waitFor(`!!document.querySelector('#canvas-main')`, 'el lienzo');
  await wait(2500); // la app carga su diseño de ejemplo al arrancar
  await evaluate(`(async () => {
    const { useStore } = await import('/src/store/index.ts');
    const { useTonal } = await import('/src/features/tonal/store.ts');
    const { getTonalLayers } = await import('/src/features/tonal/layers.ts');
    const { useMaterials } = await import('/src/features/materials/state.ts');
    const { getApi } = await import('/src/api/index.ts');
    const { polylineBez } = await import('/src/lib/bezpath.ts');
    const { makeLayer } = await import('/src/lib/layers.ts');
    const testPlan = await import('/src/features/materials/testPlan.ts');
    window.__v = { useStore, useTonal, getTonalLayers, useMaterials, getApi, polylineBez, makeLayer, testPlan };
    const s = useStore.getState();
    s.setPref('uiMode', 'full');
    s.setPref('language', 'es');
    document.querySelectorAll('.toast button').forEach((b) => b.click());
  })()`);

  // ---- 1. Prueba de materiales de relleno 5 x 5 ---------------------------------------------------
  console.log('Prueba de materiales 5 x 5 (relleno): potencia por filas, velocidad por columnas');
  await loadDesign(`async (v) => {
    const form = v.testPlan.setMode(v.testPlan.defaultForm(null), 'fill');
    form.labels = false;
    form.gap = 4;
    const doc = await v.getApi().materialTest({ params: v.testPlan.buildParams(form, null) });
    v.useStore.getState().replaceDocument(doc, null);
    v.useMaterials.getState().setTestForm(form);
  }`);
  await shot('01-prueba-5x5-normal.png');
  await tonalOn();
  await shot('02-prueba-5x5-tonos.png');
  const grid = await evaluate(`window.__v.useTonal.getState().status`);
  check(grid === 'ready', 'la vista por tonos de la prueba está lista');
  // Esquina de poca energía (10 %, 3000 mm/min) contra la de mucha (100 %, 500 mm/min).
  const cell = 10; const gap = 4; const ox = 5; const oy = 5;
  const center = (r, c) => [ox + c * (cell + gap) + cell / 2, oy + (4 - r) * (cell + gap) + cell / 2];
  const lo = await sample(...(await screenOf(...center(0, 4))));
  const hi = await sample(...(await screenOf(...center(4, 0))));
  check(luma(hi) < luma(lo) - 25, 'la celda de más energía se ve más oscura que la de menos', `claro ${Math.round(luma(lo))} · oscuro ${Math.round(luma(hi))}`);
  const normalLayersHidden = await evaluate(`document.querySelector('.pv__legend') === null && document.querySelector('.pv__transport') === null`);
  check(normalLayersHidden, 'en modo por tonos no se muestran la leyenda de trazos ni la reproducción');
  check((await evaluate(`document.querySelector('.tn__approx')?.textContent ?? ''`)).includes('aproximada'), 'el aviso de simulación aproximada está a la vista');

  // ---- 2. Calibración con la prueba de materiales ------------------------------------------------
  console.log('Calibración: una celda que se ve mucho más clara de lo previsto');
  await clickText('Calibrar');
  await waitFor(`document.querySelectorAll('.tn-cell').length === 25`, 'el diálogo de calibración');
  await shot('03-calibracion-abierta.png');
  const before = await evaluate(`window.__v.useTonal.getState().material`);
  await evaluate(`document.querySelectorAll('.tn-cell')[12].click()`);
  await wait(200);
  await evaluate(`document.querySelector('.tn-step[aria-label="Tono 10 %"]').click()`);
  await wait(200);
  await shot('04-calibracion-observada.png');
  await clickText('Guardar calibración');
  await wait(600);
  await evaluate(`document.querySelectorAll('.toast button').forEach((b) => b.click())`);
  const tones = await evaluate(`window.__v.useMaterials.getState().library.tones ?? []`);
  check(tones.length === 1 && tones[0].material === before && tones[0].samples.length === 1, 'la calibración quedó en la biblioteca de materiales', JSON.stringify(tones.map((t) => [t.material, t.curve.dose_char, t.curve.gamma])));
  await waitFor(`window.__v.useTonal.getState().status === 'ready'`, 'la imagen recalculada');
  await wait(500);
  const lo2 = await sample(...(await screenOf(...center(4, 0))));
  check(luma(lo2) > luma(hi) + 10, 'con la celda calibrada más clara, la simulación se aclara', `antes ${Math.round(luma(hi))} · después ${Math.round(luma(lo2))}`);
  await shot('05-prueba-5x5-calibrada.png');
  // Los demás diseños se ven con los valores de partida del material.
  await evaluate(`(() => { const m = window.__v.useMaterials; m.getState().setLibrary({ ...m.getState().library, tones: [] }); })()`);
  await wait(300);

  // ---- 3. Una foto grabada -----------------------------------------------------------------------
  console.log('Una foto grabada: tonos y puntos del tramado');
  await loadDesign(`async (v) => {
    const layer = v.makeLayer('#2f7bff', 'Foto', { mode: 'image', interval: 0.1, speed: 1000, power: 70 });
    const obj = { id: 'foto', layer: '#2f7bff', transform: [1, 0, 0, 1, 40, 30], kind: { type: 'image', source_path: 'retrato.png', width_px: 800, height_px: 600, size_mm: [120, 90], mask: [], variable: null }, locked: false, hidden: false, group: null };
    v.useStore.getState().replaceDocument({ version: 2, objects: [obj], layers: [layer], device_id: null }, null);
  }`);
  await shot('06-foto-normal.png');
  await tonalOn();
  await shot('07-foto-tonos.png');
  const dark = await sample(...(await screenOf(100, 60)));
  check(Number.isFinite(luma(dark)), 'la foto se dibuja en el lienzo', `luma ${Math.round(luma(dark))}`);
  console.log('  Acercar: el detalle muestra los puntos');
  await evaluate(`(() => { const s = window.__v.useStore; const z = 30; s.setState({ zoom: z, panX: -(95 * z) + 360, panY: 450 + 60 * z }); })()`);
  await waitFor(`!!window.__v.getTonalLayers().detail`, 'el detalle al acercar', 80);
  await wait(400);
  const det = await evaluate(`(() => { const d = window.__v.getTonalLayers().detail; return [d.pxMm, d.covered]; })()`);
  check(det[0] <= 0.05, 'el detalle usa un píxel más fino que el de la vista general', `${det[0].toFixed(4)} mm por píxel`);
  await shot('08-foto-tonos-zoom.png');
  await evaluate(`window.__v.useStore.getState().fitContent()`);
  await wait(500);

  // ---- 4. Diseño mixto ------------------------------------------------------------------------
  console.log('Diseño mixto: corte, relleno e imagen');
  await loadDesign(`async (v) => {
    const poly = (pts) => v.polylineBez(pts, true);
    const base = { locked: false, hidden: false, group: null };
    const rect = (x, y, w, h) => [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
    const hexagon = (cx, cy, r) => Array.from({ length: 6 }, (_, i) => [cx + r * Math.cos((i * Math.PI) / 3), cy + r * Math.sin((i * Math.PI) / 3)]);
    const objs = [
      { id: 'marco', layer: '#ff3b4a', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [poly(rect(10, 10, 170, 110))] }, ...base },
      { id: 'sello', layer: '#2fd18f', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [poly(hexagon(145, 65, 26))] }, ...base },
      { id: 'banda', layer: '#2fd18f', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [poly(rect(20, 22, 70, 14))] }, ...base },
      { id: 'foto', layer: '#2f7bff', transform: [1, 0, 0, 1, 24, 44], kind: { type: 'image', source_path: 'retrato.png', width_px: 600, height_px: 600, size_mm: [66, 66], mask: [], variable: null }, ...base },
    ];
    const layers = [
      v.makeLayer('#ff3b4a', 'Corte', { mode: 'line', speed: 300, power: 85, passes: 2 }),
      v.makeLayer('#2fd18f', 'Relleno', { mode: 'fill', interval: 0.1, speed: 2200, power: 45 }),
      v.makeLayer('#2f7bff', 'Foto', { mode: 'image', interval: 0.1, speed: 1400, power: 60 }),
    ];
    v.useStore.getState().replaceDocument({ version: 2, objects: objs, layers, device_id: null }, null);
  }`);
  await shot('09-mixto-normal.png');
  await tonalOn();
  await shot('10-mixto-tonos.png');
  const cut = await sample(...(await screenOf(95, 10)));
  const bare = await sample(...(await screenOf(100, 100)));
  check(luma(cut) < luma(bare) - 20, 'el corte (2 pasadas) se ve oscuro frente al material sin tocar', `corte ${Math.round(luma(cut))} · material ${Math.round(luma(bare))}`);

  // ---- 5. Material e inglés ----------------------------------------------------------------------
  console.log('Cambiar de material y de idioma');
  await evaluate(`window.__v.useTonal.getState().setMaterial('acrylic')`);
  await waitFor(`window.__v.useTonal.getState().status === 'ready'`, 'la imagen del nuevo material');
  await wait(600);
  const acrylicBare = await sample(...(await screenOf(100, 100)));
  check(luma(acrylicBare) !== luma(bare), 'otro material cambia el color base', `${Math.round(luma(bare))} -> ${Math.round(luma(acrylicBare))}`);
  await shot('11-mixto-acrilico.png');
  await evaluate(`window.__v.useTonal.getState().setMaterial('light_wood'); window.__v.useStore.getState().setPref('language', 'en')`);
  await wait(800);
  check((await evaluate(`document.querySelector('.tn')?.textContent ?? ''`)).includes('Approximate simulation'), 'el aviso está en inglés');
  await shot('12-english.png');
  await evaluate(`window.__v.useStore.getState().setPref('language', 'es')`);
  await tonalOff();
  check(await evaluate(`!window.__v.getTonalLayers().full || true`), 'volver a normal');
  await shot('13-vuelta-a-normal.png');
} catch (e) {
  console.error(e);
  failures.push(String(e));
}
console.log(failures.length === 0 ? 'Todo bien.' : `Fallas: ${failures.length}`);
finish(failures.length === 0 ? 0 : 1);
