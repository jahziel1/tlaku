// Verificación del flujo de producción (issue #23) en el ESCRITORIO con Chrome headless y la máquina simulada
// (mock), con ratón y pantalla de verdad:
//
//   node scripts/production-verify.mjs                          # español, tema oscuro (verifica)
//   node scripts/production-verify.mjs --shots DIR              # además guarda capturas
//   node scripts/production-verify.mjs --lang en --theme light  # otro idioma / tema
//   node scripts/production-verify.mjs --matrix --shots DIR     # los cuatro: es/en × claro/oscuro
//
// Comprueba que:
//   (a) con un objeto seleccionado aparece la barra rápida (sin tapar la selección) y girar/voltear funcionan;
//   (b) un material marcado con la estrella sale en «Favoritos» y en la barra, y «Aplicar a la selección» mueve
//       la selección a una capa con esos ajustes en UN solo paso de Deshacer;
//   (c) «Iniciar la selección» abre el diálogo de inicio completo con «Solo la selección» marcada y no envía nada;
//   (d) el modo producción se enciende en Preferencias pidiendo confirmación, y el primer trabajo usa la lista completa;
//   (e) tras un trabajo completo con la lista, repetirlo muestra el paso compacto «Iniciar (modo producción)»;
//   (f) cambiar la potencia de una capa devuelve el diálogo completo;
//   (g) en cada paso no hay botones sin texto ni textos que se salgan de su caja.
// Código de salida 1 si algo falla. Variables: CHROME, APP_URL.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const appDir = join(here, '..');
const args = process.argv.slice(2);
const opt = (name, def) => (args.includes(name) ? args[args.indexOf(name) + 1] : def);
const shotsDir = opt('--shots', null);
if (shotsDir) mkdirSync(shotsDir, { recursive: true });
const combos = args.includes('--matrix')
  ? [['es', 'light'], ['es', 'dark'], ['en', 'light'], ['en', 'dark']]
  : [[opt('--lang', 'es'), opt('--theme', 'dark')]];
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const bin = process.env.CHROME ?? ['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', '/usr/bin/google-chrome', '/usr/bin/chromium'].find(existsSync);
if (!bin) throw new Error('No se encontró Chrome/Chromium. Define CHROME=/ruta/al/binario');

let vite = null;
let url = process.env.APP_URL;
if (!url) {
  vite = spawn('npx', ['vite', '--config', 'scripts/vite.stress.config.mjs', '--port', '1439', '--strictPort'], { cwd: appDir, stdio: 'ignore' });
  url = 'http://localhost:1439/';
  let up = false;
  for (let i = 0; i < 100 && !up; i++) { await wait(300); try { up = (await fetch(url)).ok; } catch { /* arrancando */ } }
  if (!up) { vite.kill(); throw new Error('Vite no arrancó'); }
}
const PORT = 9355;
const userData = mkdtempSync(join(tmpdir(), 'ls-production-'));
const proc = spawn(bin, ['--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${userData}`, '--disable-gpu', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--hide-scrollbars', '--window-size=1440,900', '--force-device-scale-factor=1', 'about:blank'], { stdio: 'ignore' });
const targets = async () => (await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()).filter((t) => t.type === 'page');
let first;
for (let i = 0; i < 50 && !first; i++) { await wait(200); try { first = (await targets())[0]; } catch { /* arrancando */ } }
if (!first) { proc.kill(); vite?.kill(); throw new Error('Chrome no respondió'); }

const ws = new WebSocket(first.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0; const pending = new Map();
ws.onmessage = (m) => { const msg = JSON.parse(m.data); if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); } };
const send = (method, params = {}) => new Promise((resolve, reject) => { const mid = ++id; pending.set(mid, (msg) => (msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result))); ws.send(JSON.stringify({ id: mid, method, params })); });
const evaluate = async (expression) => { const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true }); if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text); return r.result.value; };
let prefix = '';
const shot = async (name) => { if (!shotsDir) return; const r = await send('Page.captureScreenshot', { format: 'png' }); writeFileSync(join(shotsDir, `${prefix}${name}`), Buffer.from(r.data, 'base64')); console.log('  captura', `${prefix}${name}`); };
const failures = [];
const check = (ok, what, extra = '') => { console.log(`  ${ok ? 'OK ' : 'FALLA'} ${what}${extra ? ' · ' + extra : ''}`); if (!ok) failures.push(`${prefix}${what}`); };
const until = async (fn, what, ms = 8000) => { const t0 = Date.now(); while (Date.now() - t0 < ms) { if (await evaluate(fn)) return true; await wait(100); } check(false, `(esperando) ${what}`); return false; };

// Texto traducido en el idioma de la prueba.
const T = (key, params = {}) => evaluate(`window.__v.t(${JSON.stringify(key)}, ${JSON.stringify(params)})`);
// Pulsa, con el DOM de verdad, el botón visible cuyo texto o etiqueta contiene `text` (dentro de `root`).
const clickText = (text, root = 'document') => evaluate(`(() => {
  const btns = [...${root}.querySelectorAll('button, [role=menuitem]')].filter((b) => b.offsetParent !== null && ((b.getAttribute('aria-label') ?? '') + ' ' + b.textContent).includes(${JSON.stringify(text)}));
  if (!btns.length) return false;
  btns[0].click();
  return true;
})()`);
// Clic de ratón de verdad (pointerdown/up) en el centro del elemento: la baldosa de la biblioteca inserta al soltar.
const mouseClick = async (selector) => {
  const c = await evaluate(`(() => { const e = document.querySelector(${JSON.stringify(selector)}); if (!e) return null; const r = e.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; })()`);
  if (!c) return false;
  await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: c[0], y: c[1] });
  await send('Input.dispatchMouseEvent', { type: 'mousePressed', x: c[0], y: c[1], button: 'left', clickCount: 1 });
  await send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: c[0], y: c[1], button: 'left', clickCount: 1 });
  return true;
};
// Revisa, en lo que está a la vista dentro de `root`, que ningún botón se quede sin texto accesible y que ningún
// texto se salga de su caja (desborde horizontal de botones, etiquetas y títulos).
const layoutProblems = (root) => evaluate(`(() => {
  const r = document.querySelector(${JSON.stringify(root)});
  if (!r) return ['no existe ' + ${JSON.stringify(root)}];
  const out = [];
  for (const b of r.querySelectorAll('button')) {
    if (b.offsetParent === null) continue;
    const name = (b.getAttribute('aria-label') || b.textContent || b.title || '').trim();
    if (!name) out.push('botón sin texto: ' + b.className);
  }
  for (const e of r.querySelectorAll('button, label, h2, h3, .art-comp__name, .art-comp__meta, .hint, .btn')) {
    if (e.offsetParent === null) continue;
    const st = getComputedStyle(e);
    const clipped = st.overflow !== 'visible' || st.overflowX !== 'visible';
    if (e.scrollWidth > e.clientWidth + 1 && !clipped) out.push('desborda: ' + (e.textContent || e.className).trim().slice(0, 40));
    if (!clipped || st.textOverflow !== 'ellipsis') continue;
  }
  return out;
})()`);

// Pulsa el botón visible cuyo texto es EXACTAMENTE `text` (sin espacios sobrantes).
const clickExact = (text) => evaluate(`(() => {
  const b = [...document.querySelectorAll('button')].find((x) => x.offsetParent !== null && x.textContent.trim() === ${JSON.stringify(text)});
  if (!b) return false;
  b.click();
  return true;
})()`);
const titles = () => evaluate(`[...document.querySelectorAll('.mc-modal__title, .modal__title')].map((e) => e.textContent).join('|')`);

const SETUP = (lang, theme) => `(async () => {
  const { useStore, bindMachineEvents } = await import('/src/store/index.ts');
  const { setApi } = await import('/src/api/index.ts');
  const { createMockApi } = await import('/src/api/mock/index.ts');
  const { sampleLayers, sampleObjects } = await import('/src/api/mock/sample.ts');
  const { emptyDocument } = await import('/src/store/document.ts');
  const { commands } = await import('/src/commands/index.ts');
  const { t } = await import('/src/i18n/index.ts');
  const { usePrompts } = await import('/src/features/history/prompt.ts');
  const { useMaterials } = await import('/src/features/materials/state.ts');
  const { loadMaterials } = await import('/src/features/materials/actions.ts');
  const { presetLabel } = await import('/src/features/materials/favorites.ts');
  const { resetProductionSession } = await import('/src/features/production/session.ts');
  const { worldToScreen } = await import('/src/lib/viewport.ts');
  const { objectBoundsWithCopies } = await import('/src/lib/virtualArray.ts');
  const V = (window.__v = { useStore, commands, t, usePrompts, useMaterials, loadMaterials, presetLabel, worldToScreen, objectBoundsWithCopies, sampleLayers, sampleObjects, emptyDocument });
  const s = () => useStore.getState();
  const api = createMockApi({ latencyMs: 0, statusIntervalMs: 200, minJobMs: 2500, jobSpeedup: 1e6 });
  V.api = api;
  V.starts = 0;
  const startJob = api.startJob.bind(api);
  api.startJob = (a) => { V.starts += 1; return startJob(a); };
  setApi(api);
  await bindMachineEvents(api, useStore);
  resetProductionSession();
  s().setPref('uiMode', 'full');
  s().setPref('language', ${JSON.stringify(lang)});
  s().setPref('theme', ${JSON.stringify(theme)});
  s().setPref('productionMode', false);
  s().setPref('jobChecklistRequired', false);
  s().setPref('favoriteMaterials', []);
  document.querySelectorAll('.toast button').forEach((b) => b.click());
  await s().loadDevices();
  s().setSelectedPort('virtual:falcon2');
  await s().connect();
  await new Promise((r) => setTimeout(r, 600));
  s().replaceDocument({ ...emptyDocument(), objects: sampleObjects(), layers: sampleLayers() }, null);
  s().fitContent();
  await V.loadMaterials();
  return { connected: s().connection.connected, tabs: s().tabs.length, factory: V.useMaterials.getState().factory.presets.length };
})()`;

// Cuadro de una caja, en píxeles de la ventana.
const RECTS = `(() => {
  const V = window.__v; const s = V.useStore.getState();
  const c = document.querySelector('#canvas-main').getBoundingClientRect();
  const o = s.doc.objects.find((x) => x.id === 'obj-estrella');
  const b = V.objectBoundsWithCopies(o);
  const a = V.worldToScreen({ zoom: s.zoom, panX: s.panX, panY: s.panY }, [b[0][0], b[1][1]]);
  const z = V.worldToScreen({ zoom: s.zoom, panX: s.panX, panY: s.panY }, [b[1][0], b[0][1]]);
  const q = document.querySelector('[data-testid=quick-bar]')?.getBoundingClientRect();
  return { sel: [c.left + a[0], c.top + a[1], c.left + z[0], c.top + z[1]], bar: q ? [q.left, q.top, q.right, q.bottom] : null, canvas: [c.left, c.top, c.right, c.bottom] };
})()`;
const overlaps = (a, b) => a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3];

const STAR = `window.__v.useStore.getState().doc.objects.find((x) => x.id === 'obj-estrella')`;
const starBox = () => evaluate(`(() => { const b = window.__v.objectBoundsWithCopies(${STAR}); return [b[1][0] - b[0][0], b[1][1] - b[0][1], (b[0][0] + b[1][0]) / 2, (b[0][1] + b[1][1]) / 2]; })()`);
const starLayerOf = () => evaluate(`(() => { const s = window.__v.useStore.getState(); const o = ${STAR}; const l = s.doc.layers.find((x) => x.color.toLowerCase() === o.layer.toLowerCase()); return JSON.stringify({ color: o.layer, speed: l?.speed, power: l?.power, mode: l?.mode, output: l?.output }); })()`);

async function scenario(lang, theme) {
  prefix = `${lang}-${theme}-`;
  console.log(`\n=== ${lang} · ${theme} ===`);
  await send('Page.navigate', { url });
  await wait(3000);
  const info = await evaluate(SETUP(lang, theme));
  console.log('preparando…', JSON.stringify(info));
  check(info.connected, 'la máquina virtual quedó conectada');
  check(info.factory > 0, 'hay materiales de fábrica', String(info.factory));
  await wait(400);

  console.log('1. barra rápida: solo aparece con selección');
  check((await evaluate(`!!document.querySelector('[data-testid=quick-bar]')`)) === false, 'sin selección no hay barra rápida');
  await evaluate(`window.__v.useStore.getState().select(['obj-estrella'])`);
  await wait(500);
  check(await evaluate(`!!document.querySelector('[data-testid=quick-bar]')`), 'con un objeto seleccionado aparece la barra rápida');
  await evaluate(`window.__v.useStore.getState().zoomToSelection?.()`);
  await wait(300);
  const lp1 = await layoutProblems('[data-testid=quick-bar]');
  check(lp1.length === 0, 'la barra no tiene botones sin texto ni desbordes', JSON.stringify(lp1));
  const rects = await evaluate(RECTS);
  check(!!rects.bar && !overlaps(rects.sel, rects.bar), 'la barra no tapa la selección', JSON.stringify(rects));
  check(!!rects.bar && rects.bar[0] >= rects.canvas[0] && rects.bar[2] <= rects.canvas[2], 'la barra cabe dentro del lienzo');
  await shot('01-barra-rapida.png');

  console.log('2. girar y voltear con la barra');
  const before = await starBox();
  await mouseClick('.qbar > button:nth-child(2)'); // rotar 90° a la derecha
  await wait(300);
  const rot = await starBox();
  check(Math.abs(rot[0] - before[1]) < 0.05 && Math.abs(rot[1] - before[0]) < 0.05, 'girar 90° intercambia ancho y alto', JSON.stringify({ before, rot }));
  check(Math.abs(rot[2] - before[2]) < 0.05 && Math.abs(rot[3] - before[3]) < 0.05, 'gira alrededor de su centro');
  await shot('02-girado.png');
  const t0 = await evaluate(`JSON.stringify(${STAR}.transform)`);
  await mouseClick('.qbar > button:nth-child(3)'); // voltear horizontal
  await wait(300);
  const t1 = await evaluate(`JSON.stringify(${STAR}.transform)`);
  check(t0 !== t1, 'voltear horizontal cambia la transformación', `${t0} → ${t1}`);
  await mouseClick('.qbar > button:nth-child(4)'); // voltear vertical
  await wait(300);
  const t2 = await evaluate(`JSON.stringify(${STAR}.transform)`);
  check(t2 !== t1, 'voltear vertical cambia la transformación');
  const flipped = await starBox();
  check(Math.abs(flipped[2] - before[2]) < 0.05 && Math.abs(flipped[3] - before[3]) < 0.05, 'voltear deja la pieza donde estaba');
  // Se deshace todo: la prueba sigue con el objeto como estaba.
  for (let i = 0; i < 4; i++) await evaluate(`window.__v.commands.run('edit.undo')`);
  await wait(300);
  const back = await starBox();
  check(Math.abs(back[0] - before[0]) < 0.05 && Math.abs(back[1] - before[1]) < 0.05, 'Deshacer devuelve el objeto a su forma original');
  await evaluate(`window.__v.useStore.getState().select(['obj-estrella'])`);

  console.log('3. material favorito con la estrella y «Aplicar a la selección»');
  const preset = JSON.parse(await evaluate(`(() => {
    const V = window.__v;
    const p = V.useMaterials.getState().factory.presets.find((x) => x.settings.mode === 'line' && x.settings.power > 0 && x.settings.power !== 20);
    return JSON.stringify({ id: p.id, name: V.presetLabel(p), speed: p.settings.speed, power: p.settings.power });
  })()`));
  console.log('  material de prueba:', preset.name);
  await evaluate(`window.__v.useStore.getState().setDockTab('materials')`);
  await wait(600);
  const addLabel = await T('materials.fav.addFor', { name: preset.name });
  // La lista de fábrica puede estar fuera de la vista o filtrada: se busca por el nombre.
  await evaluate(`(() => {
    const i = document.querySelector('.mat-panel input[type=search], .mat-panel input[type=text]');
    if (!i) return;
    const d = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
    d.set.call(i, ${JSON.stringify(preset.name.split(' · ')[0])});
    i.dispatchEvent(new Event('input', { bubbles: true }));
  })()`);
  await wait(500);
  let star = await evaluate(`!!document.querySelector(${JSON.stringify(`button[aria-label=${JSON.stringify(addLabel)}]`)})`);
  check(star, 'el material tiene su estrella en la pestaña Materiales', addLabel);
  await shot('03-materiales-estrella.png');
  if (star) await evaluate(`document.querySelector(${JSON.stringify(`button[aria-label=${JSON.stringify(addLabel)}]`)}).click()`);
  await wait(500);
  check((await evaluate(`window.__v.useStore.getState().prefs?.favoriteMaterials?.includes(${JSON.stringify(preset.id)}) ?? window.__v.useStore.getState().favoriteMaterials?.includes?.(${JSON.stringify(preset.id)})`)) === true, 'el material quedó en los favoritos', preset.id);
  check(await evaluate(`!!document.querySelector('section[aria-label="${await T('materials.fav.section')}"]')`), 'aparece la sección «Favoritos»');
  const lpm = await layoutProblems('.mat-panel');
  check(lpm.length === 0, 'la pestaña Materiales no tiene botones sin texto ni desbordes', JSON.stringify(lpm).slice(0, 300));
  await shot('04-favoritos.png');
  check(await evaluate(`!!document.querySelector('.qbar__fav')`), 'la barra rápida muestra el favorito');
  const layersBefore = await evaluate(`window.__v.useStore.getState().doc.layers.length`);
  const colorBefore = JSON.parse(await starLayerOf()).color;
  check(await clickText(await T('materials.fav.applySelection'), `document.querySelector('.mat-panel')`), 'se pulsa «Aplicar a la selección»');
  await wait(600);
  const after = JSON.parse(await starLayerOf());
  check(after.color !== colorBefore && after.speed === preset.speed && after.power === preset.power, 'la selección pasó a una capa con los ajustes del material', JSON.stringify(after));
  check((await evaluate(`window.__v.useStore.getState().doc.layers.length`)) >= layersBefore, 'la capa existe en el documento');
  const hit = await evaluate(`(() => {
    const q = document.querySelector('[data-testid=quick-bar]')?.getBoundingClientRect();
    const ts = [...document.querySelectorAll('.toast')].map((e) => e.getBoundingClientRect());
    return { toasts: ts.length, over: !!q && ts.some((r) => r.left < q.right && q.left < r.right && r.top < q.bottom && q.top < r.bottom) };
  })()`);
  check(hit.toasts > 0 && !hit.over, 'el aviso de «movida a una capa» no tapa la barra rápida', JSON.stringify(hit));
  await shot('05-favorito-aplicado.png');
  await evaluate(`window.__v.commands.run('edit.undo')`);
  await wait(300);
  check(JSON.parse(await starLayerOf()).color === colorBefore, 'un solo Deshacer devuelve la selección a su capa');
  await evaluate(`window.__v.useStore.getState().select(['obj-estrella'])`);
  await evaluate(`window.__v.useStore.getState().setDockTab('layers')`);
  // Ahora con el chip de la barra.
  const favTitle = await T('machine.quick.favTitle', { name: preset.name });
  check(await evaluate(`(() => { const b = document.querySelector(${JSON.stringify(`.qbar__fav[aria-label=${JSON.stringify(favTitle)}]`)}); if (!b) return false; b.click(); return true; })()`), 'el chip de la barra aplica el favorito');
  await wait(500);
  check(JSON.parse(await starLayerOf()).power === preset.power, 'el chip movió la selección a la capa con el material');
  await shot('06-barra-con-favorito.png');
  await evaluate(`window.__v.commands.run('edit.undo')`);
  await wait(300);
  await evaluate(`window.__v.useStore.getState().select(['obj-estrella'])`);

  console.log('4. «Iniciar la selección» abre el diálogo completo con «Solo la selección» activada');
  const startsBefore = await evaluate('window.__v.starts');
  check(await clickText(await T('machine.quick.startSelection'), `document.querySelector('.qbar')`), 'se pulsa «Iniciar la selección» en la barra');
  await wait(1200);
  check((await titles()).includes(await T('machine.start.title')), 'se abre el diálogo de inicio completo', await titles());
  check(await evaluate(`(() => { const m = document.querySelector('.mc-modal'); const l = [...m.querySelectorAll('label.mc-check')].find((x) => x.textContent.includes(${JSON.stringify(await T('machine.selection.only'))})); return !!l && l.querySelector('input').checked; })()`), '«Solo la selección» está marcada en el diálogo');
  check((await evaluate(`window.__v.useStore.getState().cutSelectionOnly`)) === true, 'el estado dice «solo la selección»');
  check((await evaluate('window.__v.starts')) === startsBefore, 'no se envió nada a la máquina');
  const lpd = await layoutProblems('.mc-modal');
  check(lpd.length === 0, 'el diálogo no tiene botones sin texto ni desbordes', JSON.stringify(lpd));
  await shot('07-iniciar-seleccion.png');
  await clickText(await T('machine.dlg.cancel'), `document.querySelector('.mc-modal')`);
  await wait(500);
  await evaluate(`window.__v.useStore.getState().setCutSelectionOnly(false)`);
  await evaluate(`window.__v.useStore.getState().select([])`);

  console.log('5. modo producción: se enciende en Preferencias con confirmación');
  await evaluate(`window.__v.commands.run('prefs.open')`);
  await wait(600);
  const lblMode = await T('prefs.productionMode');
  check(await evaluate(`[...document.querySelectorAll('.set-check')].some((l) => l.textContent.includes(${JSON.stringify(lblMode)}))`), 'Preferencias › General muestra «Modo producción»', lblMode);
  await evaluate(`document.querySelector('.set-panel')?.scrollTo?.(0, 99999)`);
  await evaluate(`[...document.querySelectorAll('.set-check')].find((l) => l.textContent.includes(${JSON.stringify(lblMode)})).scrollIntoView({ block: 'center' })`);
  await wait(300);
  check((await evaluate(`window.__v.useStore.getState().prefs?.productionMode ?? false`)) === false, 'está apagado de fábrica');
  const lps = await layoutProblems('.set-panel');
  check(lps.length === 0, 'Preferencias no tiene botones sin texto ni desbordes', JSON.stringify(lps).slice(0, 300));
  await shot('08-preferencias-produccion.png');
  await evaluate(`[...document.querySelectorAll('.set-check')].find((l) => l.textContent.includes(${JSON.stringify(lblMode)})).querySelector('input').click()`);
  await wait(500);
  const ask = await evaluate(`window.__v.usePrompts.getState().current?.title ?? ''`);
  check(ask === (await T('prefs.productionMode.confirmTitle')), 'aparece la confirmación al encenderlo', ask);
  check((await evaluate(`window.__v.useStore.getState().prefs?.productionMode ?? false`)) === false, 'sigue apagado hasta confirmar');
  const lpp = await layoutProblems('.modal');
  check(lpp.length === 0, 'la confirmación no tiene botones sin texto ni desbordes', JSON.stringify(lpp));
  await shot('09-confirmar-modo-produccion.png');
  check(await clickText(await T('prefs.productionMode.confirmYes'), `document.querySelector('.modal')`), 'se confirma «Activar»');
  await wait(500);
  check((await evaluate(`window.__v.useStore.getState().prefs?.productionMode ?? false`)) === true, 'el modo producción quedó encendido');
  await evaluate(`window.__v.useStore.getState().setPref?.length >= 0`);
  await evaluate(`window.__v.commands.run('prefs.open')`);
  await evaluate(`document.querySelector('.set-card [aria-label], .set-card')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))`);
  await wait(400);
  if (await evaluate(`!!document.querySelector('.set-card')`)) await evaluate(`(async () => { (await import('/src/features/settings/uiStore.ts')).useSettingsUi.getState().closeSettings(); })()`);
  await wait(300);

  console.log('6. un trabajo completo con la lista de comprobación (primer inicio: siempre la lista completa)');
  await evaluate(`window.__v.useStore.getState().select([])`);
  // Quien usa el modo producción suele exigir la lista: así «Iniciar trabajo» espera a que se marque todo.
  await evaluate(`window.__v.useStore.getState().setPref('jobChecklistRequired', true)`);
  check(await clickExact(await T('cmd.machine.start')), 'se pulsa «Iniciar» en la barra superior');
  await wait(1500);
  check((await titles()).includes(await T('machine.start.title')), 'el primer inicio muestra la lista completa', await titles());
  check(await evaluate(`!!document.querySelector('.mc-checklist')`), 'la lista de comprobación está a la vista');
  const startBtn = await T('machine.start.confirm');
  check((await buttonDisabled(startBtn)) === true, '«Iniciar trabajo» está deshabilitado hasta marcar la lista');
  await shot('10-lista-completa.png');
  await evaluate(`document.querySelectorAll('.mc-modal .mc-checklist input[type=checkbox]').forEach((c) => { if (!c.checked) c.click(); })`);
  await wait(400);
  check((await buttonDisabled(startBtn)) === false, 'con la lista marcada «Iniciar trabajo» se habilita');
  await shot('11-lista-marcada.png');
  check(await clickText(startBtn, `document.querySelector('.mc-modal')`), 'se pulsa «Iniciar trabajo»');
  await until(`window.__v.useStore.getState().jobState === 'running'`, 'el trabajo está en marcha');
  await shot('12-trabajo-en-curso.png');
  await until(`window.__v.useStore.getState().jobState === 'finished'`, 'el trabajo terminó', 15000);
  check((await evaluate('window.__v.starts')) === 1, 'se envió exactamente un trabajo');
  await wait(1000);
  await evaluate(`window.__v.useStore.getState().closeDialog?.()`);

  console.log('7. repetir el mismo trabajo: paso compacto «Iniciar (modo producción)»');
  await wait(500);
  check(await clickExact(await T('cmd.machine.start')), 'se pulsa «Iniciar» otra vez');
  await until(`document.querySelector('.mc-modal__title')?.textContent === ${JSON.stringify(await T('machine.prod.title'))} && !document.body.textContent.includes(${JSON.stringify(await T('machine.prod.checking'))})`, 'sale el paso compacto', 10000);
  const t7 = await titles();
  check(t7.includes(await T('machine.prod.title')), 'el título es «Iniciar (modo producción)»', t7);
  check((await evaluate(`!document.querySelector('.mc-checklist')`)), 'ya no aparece la lista de comprobación');
  check(await evaluate(`!!document.querySelector('[data-testid=prod-time]')`), 'muestra el tiempo estimado');
  check(await evaluate(`!!document.querySelector('[data-testid=prod-full]')`), 'ofrece «Ver la lista completa»');
  check((await evaluate(`document.activeElement?.textContent ?? ''`)).includes(await T('machine.prod.confirm')), '«Iniciar (Enter)» lleva el foco');
  check((await evaluate('window.__v.starts')) === 1, 'todavía no se envió el segundo trabajo');
  const lp7 = await layoutProblems('.mc-modal');
  check(lp7.length === 0, 'el paso compacto no tiene botones sin texto ni desbordes', JSON.stringify(lp7));
  await shot('13-paso-compacto.png');
  await clickText(await T('machine.dlg.cancel'), `document.querySelector('.mc-modal')`);
  await wait(500);

  console.log('8. cambiar la potencia de una capa devuelve el diálogo completo');
  await evaluate(`(() => { const s = window.__v.useStore.getState(); const l = s.doc.layers.find((x) => x.output); s.updateLayer(l.color, { power: Math.min(100, l.power + 7) }); })()`);
  await wait(500);
  check(await clickExact(await T('cmd.machine.start')), 'se pulsa «Iniciar» tras cambiar la potencia');
  await wait(2000);
  const t8 = await titles();
  check(t8.includes(await T('machine.start.title')) && !t8.includes(await T('machine.prod.title')), 'vuelve el diálogo completo', t8);
  check(await evaluate(`!!document.querySelector('.mc-checklist')`), 'vuelve la lista de comprobación');
  await shot('14-completo-tras-cambio.png');
  await clickText(await T('machine.dlg.cancel'), `document.querySelector('.mc-modal')`);
  await wait(300);
  check((await evaluate('window.__v.starts')) === 1, 'en todo el recorrido solo se envió un trabajo');
}

const buttonDisabled = (text) => evaluate(`(() => { const b = [...document.querySelectorAll('.mc-modal button')].find((x) => x.textContent.includes(${JSON.stringify(text)})); return b ? b.disabled : null; })()`);

try {
  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  for (const [lang, theme] of combos) await scenario(lang, theme);
} catch (e) {
  console.error(e);
  failures.push(String(e));
} finally {
  ws.close();
  proc.kill();
  vite?.kill();
}
if (failures.length) {
  console.error(`\n${failures.length} comprobaciones fallaron:\n - ${failures.join('\n - ')}`);
  process.exit(1);
}
console.log('\nTodo bien.');
process.exit(0);
