// Verificación de las pestañas de documentos en el ESCRITORIO con Chrome headless y la máquina simulada (mock).
// jsdom no dibuja canvas, así que esto comprueba píxeles de verdad y usa teclado y ratón reales (CDP):
//
//   node scripts/tabs-verify.mjs                      # solo verifica
//   node scripts/tabs-verify.mjs --shots DIR          # además guarda capturas
//
// Abre tres documentos con ediciones distintas y comprueba que:
//   (a) Ctrl/Cmd+T abre pestañas; cada una conserva su documento, su zoom/paneo y sus cambios al ir y volver
//       (el lienzo dibuja cosas distintas en cada una, y la marca de cambios sin guardar es por pestaña);
//   (b) deshacer (Ctrl/Cmd+Z) en una pestaña no toca a las demás;
//   (c) ←/→ y Ctrl+Tab cambian de pestaña; arrastrar con el ratón reordena; el menú contextual se abre;
//   (d) cerrar con cambios pregunta (guardar / no guardar / cancelar), y la lista de varias sucias sale al cerrar la ventana;
//   (e) un trabajo lanzado desde una pestaña la marca con un icono, el diálogo de inicio dice el documento, y el
//       trabajo y la vista en vivo siguen aunque se cambie de pestaña;
//   (f) al recargar, se reabren las pestañas que tenían archivo (y no las «Sin título»).
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
  vite = spawn('npx', ['vite', '--config', 'scripts/vite.stress.config.mjs', '--port', '1435', '--strictPort'], { cwd: appDir, stdio: 'ignore' });
  url = 'http://localhost:1435/';
  let up = false;
  for (let i = 0; i < 100 && !up; i++) { await wait(300); try { up = (await fetch(url)).ok; } catch { /* arrancando */ } }
  if (!up) { vite.kill(); throw new Error('Vite no arrancó'); }
}
const PORT = 9351;
const userData = mkdtempSync(join(tmpdir(), 'ls-tabs-'));
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
const mouse = (type, x, y, extra = {}) => send('Input.dispatchMouseEvent', { type, x, y, button: 'left', buttons: type === 'mouseReleased' ? 0 : 1, clickCount: 1, ...extra });
const shot = async (name) => { if (!shotsDir) return; const r = await send('Page.captureScreenshot', { format: 'png' }); writeFileSync(join(shotsDir, name), Buffer.from(r.data, 'base64')); console.log('  captura', name); };
const failures = [];
const check = (ok, what, extra = '') => { console.log(`  ${ok ? 'OK ' : 'FALLA'} ${what}${extra ? ' · ' + extra : ''}`); if (!ok) failures.push(what); };

const centerOf = (sel) => evaluate(`(() => { const e = document.querySelector(${JSON.stringify(sel)}); if (!e) return null; const b = e.getBoundingClientRect(); return [b.left + b.width / 2, b.top + b.height / 2, b.left, b.top, b.width, b.height]; })()`);
const clickAt = async (x, y, button = 'left') => {
  await mouse('mouseMoved', x, y, { buttons: 0 });
  await mouse('mousePressed', x, y, { button, buttons: button === 'right' ? 2 : 1 });
  await mouse('mouseReleased', x, y, { button });
  await wait(150);
};
/** Pulsa una tecla con el teclado real. `mods`: 1 Alt, 2 Ctrl, 4 Meta, 8 Mayús. */
const key = async (k, code, vk, mods = 0) => {
  await send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key: k, code, windowsVirtualKeyCode: vk, modifiers: mods });
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key: k, code, windowsVirtualKeyCode: vk, modifiers: mods });
  await wait(200);
};

const SETUP = `(async () => {
  const { useStore, bindMachineEvents } = await import('/src/store/index.ts');
  const { setApi } = await import('/src/api/index.ts');
  const { createMockApi } = await import('/src/api/mock/index.ts');
  const { commands } = await import('/src/commands/index.ts');
  const tabsActions = await import('/src/features/tabs/actions.ts');
  const { startLive } = await import('/src/features/live/service.ts');
  const { usePrompts } = await import('/src/features/history/prompt.ts');
  const V = (window.__t = { useStore, commands, tabsActions, usePrompts });
  const s = () => useStore.getState();
  const api = createMockApi({ latencyMs: 0, statusIntervalMs: 200, minJobMs: 9000, jobSpeedup: 1e6 });
  setApi(api);
  await bindMachineEvents(api, useStore);
  V.stopLive = startLive(useStore);
  s().setPref('uiMode', 'full');
  document.querySelectorAll('.toast button').forEach((b) => b.click());
  await s().loadDevices();
  V.mac = /Mac/.test(navigator.platform);
  V.hash = () => {
    const cv = document.querySelector('#canvas-main'); const d = cv.getContext('2d').getImageData(0, 0, cv.width, cv.height).data;
    let h = 2166136261;
    for (let i = 0; i < d.length; i += 13) h = Math.imul(h ^ d[i], 16777619) >>> 0;
    return h;
  };
  V.names = () => [...document.querySelectorAll('.tabbar [role="tab"]')].map((t) => t.querySelector('.tab__name').textContent);
  V.dirtyMarks = () => [...document.querySelectorAll('.tabbar [role="tab"]')].map((t) => !!t.querySelector('.tab__dirty'));
  V.active = () => document.querySelector('.tabbar [role="tab"][aria-selected="true"] .tab__name')?.textContent;
  V.objs = () => s().doc.objects.length;
  V.tabObjs = () => s().tabs.map((e) => (e.id === s().activeTabId ? s().doc : e.data.doc).objects.length);
  return { mac: V.mac, tabs: s().tabs.length };
})()`;

const open = async () => {
  await send('Page.navigate', { url });
  await wait(3000);
  return evaluate(SETUP);
};
const MOD = () => (macHost ? 4 : 2);
let macHost = false;
const shotName = (n, t) => `${String(n).padStart(2, '0')}-${t}.png`;

try {
  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  const info = await open();
  macHost = info.mac;
  console.log('preparando…', JSON.stringify(info));

  console.log('1. arranque: una pestaña vacía');
  check((await evaluate('window.__t.names().join("|")')) === 'Sin título 1', 'hay una pestaña «Sin título 1»');
  check((await evaluate('document.querySelector(\'.tabbar [role="tablist"]\').getAttribute("aria-label")')) === 'Documentos abiertos', 'la barra es un tablist con nombre');
  await shot(shotName(1, 'inicio'));

  console.log('2. tres documentos con ediciones distintas (Ctrl/Cmd+T abre cada pestaña)');
  // Pestaña 1: importa y borra una forma.
  await evaluate(`(async () => { await window.__t.commands.run('file.import'); await new Promise((r) => setTimeout(r, 300)); const s = window.__t.useStore.getState(); s.fitContent(); s.deleteObjects([s.doc.objects[0].id]); })()`);
  // Pestaña 2 (con el teclado real).
  await key('t', 'KeyT', 84, MOD());
  check((await evaluate('window.__t.useStore.getState().tabs.length')) === 2, 'Ctrl/Cmd+T abre una segunda pestaña');
  await evaluate(`(async () => { await window.__t.commands.run('file.import'); await new Promise((r) => setTimeout(r, 300)); const s = window.__t.useStore.getState(); s.fitContent(); s.moveObjects(s.doc.objects.map((o) => o.id), 40, 25); s.zoomAt(1.8, 300, 300); })()`);
  // Pestaña 3.
  await key('t', 'KeyT', 84, MOD());
  await evaluate(`(async () => { await window.__t.commands.run('file.import'); await new Promise((r) => setTimeout(r, 300)); const s = window.__t.useStore.getState(); s.fitContent(); const keep = s.doc.objects[1].id; s.deleteObjects(s.doc.objects.filter((o) => o.id !== keep).map((o) => o.id)); s.zoomAt(0.6, 200, 200); })()`);
  await wait(300);
  const names3 = await evaluate('window.__t.names().join("|")');
  check(names3 === 'Sin título 1|Sin título 2|Sin título 3', 'tres pestañas con su número', names3);
  check((await evaluate('window.__t.dirtyMarks().every(Boolean)')), 'las tres muestran la marca de cambios sin guardar');
  await shot(shotName(2, 'tres-pestanas'));

  console.log('3. cada pestaña conserva su documento, su zoom y su dibujo al ir y volver');
  const hashes = [];
  const counts = [];
  const zooms = [];
  for (let i = 0; i < 3; i++) {
    const tabs = await evaluate(`[...document.querySelectorAll('.tabbar [role="tab"]')].map((t) => { const b = t.getBoundingClientRect(); return [b.left + b.width / 2 - 10, b.top + b.height / 2]; })`);
    await clickAt(tabs[i][0], tabs[i][1]);
    await wait(350);
    hashes.push(await evaluate('window.__t.hash()'));
    counts.push(await evaluate('window.__t.objs()'));
    zooms.push(await evaluate('window.__t.useStore.getState().zoom'));
    await shot(shotName(3 + i, `pestana-${i + 1}`));
  }
  check(new Set(hashes).size === 3, 'el lienzo dibuja algo distinto en cada pestaña', hashes.join(','));
  check(counts[1] === 4 && counts[2] === 1 && counts[0] > counts[1], 'cada pestaña tiene sus objetos (la primera, los de arranque más lo importado menos uno)', counts.join(','));
  check(new Set(zooms.map((z) => z.toFixed(3))).size === 3, 'cada pestaña conserva su zoom', zooms.map((z) => z.toFixed(2)).join(','));
  // Volver a la primera da lo mismo que la primera vez.
  const tabsNow = await evaluate(`[...document.querySelectorAll('.tabbar [role="tab"]')].map((t) => { const b = t.getBoundingClientRect(); return [b.left + b.width / 2 - 10, b.top + b.height / 2]; })`);
  await clickAt(tabsNow[0][0], tabsNow[0][1]);
  await wait(350);
  check((await evaluate('window.__t.hash()')) === hashes[0], 'al volver a la primera, el lienzo es idéntico');

  console.log('4. deshacer en una pestaña no toca a las otras');
  await clickAt(tabsNow[2][0], tabsNow[2][1]);
  await wait(300);
  const before = await evaluate('window.__t.tabObjs().join(",")');
  await key('z', 'KeyZ', 90, MOD());
  const afterUndo = await evaluate('window.__t.tabObjs().join(",")');
  const [b1, b2, b3] = before.split(',').map(Number);
  const [a1, a2, a3] = afterUndo.split(',').map(Number);
  check(a3 > b3 && a1 === b1 && a2 === b2, 'Ctrl/Cmd+Z en la tercera deshace solo la tercera', `${before} -> ${afterUndo}`);
  await key('z', 'KeyZ', 90, MOD());
  await clickAt(tabsNow[0][0], tabsNow[0][1]);
  await wait(300);
  const u1 = await evaluate('window.__t.useStore.getState().past.length');
  await key('z', 'KeyZ', 90, MOD());
  check((await evaluate('window.__t.useStore.getState().past.length')) === u1 - 1, 'y en la primera deshace un paso de la primera');
  check((await evaluate('window.__t.useStore.getState().canRedo()')), 'la primera puede rehacer');
  await clickAt(tabsNow[1][0], tabsNow[1][1]);
  await wait(300);
  check(!(await evaluate('window.__t.useStore.getState().canRedo()')), 'la segunda no tiene nada que rehacer');
  await shot(shotName(6, 'deshacer-por-pestana'));

  console.log('5. teclado: flechas, Ctrl+Tab; arrastrar; menú contextual');
  await evaluate(`document.querySelector('.tabbar [role="tab"][aria-selected="true"]').focus()`);
  await key('ArrowRight', 'ArrowRight', 39);
  check((await evaluate('window.__t.active()')) === 'Sin título 3', '→ pasa a la siguiente pestaña');
  await key('ArrowRight', 'ArrowRight', 39);
  check((await evaluate('window.__t.active()')) === 'Sin título 1', '→ en la última da la vuelta');
  await key('Home', 'Home', 36);
  await key('Tab', 'Tab', 9, 2);
  check((await evaluate('window.__t.active()')) === 'Sin título 2', 'Ctrl+Tab va a la siguiente');
  await key('Tab', 'Tab', 9, 2 | 8);
  check((await evaluate('window.__t.active()')) === 'Sin título 1', 'Ctrl+Mayús+Tab va a la anterior');
  // Arrastrar la primera sobre la tercera con el ratón real.
  const r = await evaluate(`[...document.querySelectorAll('[data-tab-slot]')].map((t) => { const b = t.getBoundingClientRect(); return [b.left, b.right, b.top + b.height / 2]; })`);
  await mouse('mouseMoved', r[0][0] + 40, r[0][2], { buttons: 0 });
  await mouse('mousePressed', r[0][0] + 40, r[0][2]);
  for (let i = 1; i <= 10; i++) await mouse('mouseMoved', r[0][0] + 40 + ((r[2][0] + 40 - (r[0][0] + 40)) * i) / 10, r[0][2]);
  await mouse('mouseReleased', r[2][0] + 40, r[0][2]);
  await wait(250);
  const order = await evaluate('window.__t.names().join("|")');
  check(order === 'Sin título 2|Sin título 3|Sin título 1', 'arrastrar la primera a la derecha la reordena', order);
  check((await evaluate('window.__t.active()')) === 'Sin título 1', 'reordenar no cambia la pestaña activa');
  // Menú contextual.
  const mid = await centerOf('[data-tab-slot]:nth-child(2) [role="tab"]');
  await clickAt(mid[0], mid[1], 'right');
  check((await evaluate('document.querySelectorAll(\'[role="menu"] [role="menuitem"]\').length')) === 6, 'el clic derecho abre el menú de la pestaña con sus 6 opciones');
  await shot(shotName(7, 'menu-contextual'));
  await key('Escape', 'Escape', 27);
  check((await evaluate('document.querySelector(\'[role="menu"]\') === null')), 'Esc cierra el menú');

  console.log('6. cerrar con cambios pregunta; varias sucias al cerrar la ventana');
  const close2 = await centerOf('[data-tab-slot]:nth-child(1) .tab__close'); // «Sin título 2», con cambios
  await clickAt(close2[0], close2[1]);
  await wait(250);

  check((await evaluate('!!document.querySelector(".modal") && document.querySelector(".modal").textContent.includes("cambios sin guardar")')), 'cerrar una pestaña sucia pregunta guardar / no guardar / cancelar');
  check((await evaluate('[...document.querySelectorAll(".modal__foot button")].map((b) => b.textContent).join("|")')) === 'Guardar|No guardar|Cancelar', 'con los tres botones');
  await shot(shotName(8, 'cerrar-con-cambios'));
  await evaluate('[...document.querySelectorAll(".modal__foot button")].find((b) => b.textContent === "Cancelar").click()');
  await wait(200);
  check((await evaluate('window.__t.useStore.getState().tabs.length')) === 3, 'cancelar la deja abierta');
  await evaluate('(() => { window.__t.tabsActions.confirmCloseWindow().then((ok) => (window.__t.closeResult = ok)); })()');
  await wait(300);
  const dirtyN = await evaluate('window.__t.dirtyMarks().filter(Boolean).length');
  check(dirtyN >= 2 && (await evaluate('document.querySelector(".modal__list")?.children.length')) === dirtyN, `al cerrar la ventana con ${dirtyN} pestañas sucias sale la lista de las ${dirtyN}`);
  check((await evaluate('[...document.querySelectorAll(".modal__foot button")].map((b) => b.textContent).join("|")')) === 'Guardar todas|Descartar todas|Cancelar', 'con Guardar todas / Descartar todas / Cancelar');
  await shot(shotName(9, 'cerrar-ventana-varias-sucias'));
  await evaluate('[...document.querySelectorAll(".modal__foot button")].find((b) => b.textContent === "Cancelar").click()');
  await wait(300);
  check((await evaluate('window.__t.closeResult')) === false, 'cancelar no cierra nada');
  // Cerrar con Ctrl/Cmd+W y «No guardar».
  await key('w', 'KeyW', 87, MOD());
  await wait(200);
  await evaluate('[...document.querySelectorAll(".modal__foot button")].find((b) => b.textContent === "No guardar").click()');
  await wait(300);
  check((await evaluate('window.__t.useStore.getState().tabs.length')) === 2, 'Ctrl/Cmd+W + «No guardar» cierra la pestaña activa');

  console.log('7. el trabajo en curso es de la pestaña que lo lanzó');
  await evaluate(`(async () => { const s = window.__t.useStore.getState(); s.setSelectedPort('virtual:falcon2'); await s.refreshPorts?.(); await s.connect(); await new Promise((r) => setTimeout(r, 600)); })()`);
  // La pestaña con diseño (la que tiene objetos) pasa a ser la activa y se le pone nombre, para que se vea en el diálogo.
  await evaluate(`(() => { const s = window.__t.useStore.getState(); const withObjs = s.tabs.find((e) => (e.id === s.activeTabId ? s.doc : e.data.doc).objects.length > 0); s.activateTab(withObjs.id); window.__t.useStore.getState().markSaved('/proyectos/placa.tlaku'); })()`);
  await wait(200);
  const nameBefore = await evaluate('window.__t.active()');
  await evaluate(`(async () => { const s = window.__t.useStore.getState(); s.fitContent(); await s.computePreview(); window.__t.useStore.getState().requestStart(); })()`);
  await wait(500);
  const sub = await evaluate('document.querySelector("[role=dialog]")?.textContent ?? ""');
  check(sub.includes('placa.tlaku'), 'el diálogo de inicio dice el documento que se va a enviar', nameBefore);
  await shot(shotName(10, 'dialogo-inicio'));
  await evaluate('window.__t.useStore.getState().confirmStart()');
  await wait(1200);
  await evaluate(`window.__t.useStore.getState().setPref('liveView', true)`);
  await wait(500);
  const jobTab = await evaluate('window.__t.useStore.getState().jobOrigin?.tabId');
  const tabIds = await evaluate('window.__t.useStore.getState().tabs.map((t) => t.id)');
  check(!!jobTab && tabIds.includes(jobTab), 'el trabajo anota la pestaña de origen');
  check((await evaluate('document.querySelectorAll(".tab__job").length')) === 1, 'esa pestaña muestra el icono de trabajo en curso');
  // Cambiar de pestaña y editar la otra mientras corre.
  await evaluate(`(() => { const s = window.__t.useStore.getState(); const other = s.tabs.find((t) => t.id !== s.activeTabId).id; s.activateTab(other); })()`);
  await wait(500);
  await evaluate(`window.__t.useStore.getState().deleteObjects(window.__t.useStore.getState().doc.objects.slice(0, 1).map((o) => o.id))`);
  await wait(500);
  const running = await evaluate('window.__t.useStore.getState().jobState');
  check(running === 'running', 'el trabajo sigue en marcha tras cambiar de pestaña y editar la otra', running);
  check((await evaluate('document.querySelectorAll(".tab__job").length')) === 1, 'el icono sigue en la pestaña de origen, no en la activa');
  check((await evaluate('document.querySelector(\'.tabbar [role="tab"][aria-selected="true"] .tab__job\') === null')), 'la pestaña activa (otra) no lo tiene');
  check((await evaluate('document.querySelector(\'[data-testid="live-window"]\') !== null')), 'la vista en vivo sigue visible');
  await shot(shotName(11, 'trabajo-en-otra-pestana'));
  const topbar = await evaluate('document.querySelector(\'.topbar [role="toolbar"][aria-label*="Trabajo de"]\')?.getAttribute("aria-label")');
  check(!!topbar, 'la barra de Vista previa / Encuadrar / Iniciar dice de qué pestaña es', topbar);
  await evaluate(`(async () => { await window.__t.useStore.getState().cancelJob(); })()`);
  await wait(800);
  check((await evaluate('document.querySelectorAll(".tab__job").length')) === 0, 'al terminar el trabajo el icono desaparece');

  console.log('8. la sesión: al recargar se reabren las pestañas con archivo');
  await evaluate(`(() => { const s = window.__t.useStore.getState(); const ids = s.tabs.map((t) => t.id); s.runInTab(ids[0], () => window.__t.useStore.getState().markSaved('/proyectos/uno.tlaku')); })()`);
  await evaluate(`window.__t.useStore.getState().newTab()`);
  await wait(600);
  const sessionBefore = await evaluate('localStorage.getItem("tlaku.tabs.session")');
  console.log('   sesión guardada:', sessionBefore);
  await send('Page.navigate', { url });
  await wait(3500);
  await evaluate(SETUP);
  const namesAfter = await evaluate('window.__t.names().join("|")');
  check(namesAfter.includes('placa.tlaku') || namesAfter.includes('uno.tlaku'), 'tras recargar reaparecen las pestañas con archivo', namesAfter);
  check(!namesAfter.includes('Sin título 2'), 'las «Sin título» no se reabren', namesAfter);
  await shot(shotName(12, 'sesion-restaurada'));
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
