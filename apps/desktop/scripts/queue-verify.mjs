// Verificación de la cola guiada de trabajos y del historial enriquecido en el ESCRITORIO con Chrome headless y
// la máquina simulada (mock), con ratón y pantalla de verdad:
//
//   node scripts/queue-verify.mjs                      # solo verifica
//   node scripts/queue-verify.mjs --shots DIR          # además guarda capturas
//
// Comprueba que:
//   (a) la cola se arma con dos pestañas (copias y avisos por paso) y empezarla NO envía nada;
//   (b) entre trabajos SIEMPRE aparece la confirmación (material, área, lista de comprobación) y «Continuar» sigue
//       deshabilitado hasta marcar todo; continuar solo abre el diálogo de inicio normal;
//   (c) al terminar un trabajo la cola NO manda el siguiente por sí sola (la máquina queda en reposo y el paso
//       vuelve a pedir confirmación) y la vista en vivo muestra el paso de la cola mientras corre;
//   (d) si el trabajo se cancela, la cola se detiene y no ofrece el siguiente;
//   (e) el historial guarda capas con sus ajustes, material, miniatura y notas, filtra, y «Repetir con estos ajustes»
//       pide confirmación y no envía nada a la máquina.
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
  vite = spawn('npx', ['vite', '--config', 'scripts/vite.stress.config.mjs', '--port', '1437', '--strictPort'], { cwd: appDir, stdio: 'ignore' });
  url = 'http://localhost:1437/';
  let up = false;
  for (let i = 0; i < 100 && !up; i++) { await wait(300); try { up = (await fetch(url)).ok; } catch { /* arrancando */ } }
  if (!up) { vite.kill(); throw new Error('Vite no arrancó'); }
}
const PORT = 9353;
const userData = mkdtempSync(join(tmpdir(), 'ls-queue-'));
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
const shot = async (name) => { if (!shotsDir) return; const r = await send('Page.captureScreenshot', { format: 'png' }); writeFileSync(join(shotsDir, name), Buffer.from(r.data, 'base64')); console.log('  captura', name); };
const failures = [];
const check = (ok, what, extra = '') => { console.log(`  ${ok ? 'OK ' : 'FALLA'} ${what}${extra ? ' · ' + extra : ''}`); if (!ok) failures.push(what); };
const until = async (fn, what, ms = 8000) => { const t0 = Date.now(); while (Date.now() - t0 < ms) { if (await evaluate(fn)) return true; await wait(100); } check(false, `(esperando) ${what}`); return false; };

// Pulsa, con el DOM de verdad, el botón visible cuyo texto contiene `text` (dentro de `root`).
const clickText = (text, root = 'document') => evaluate(`(() => {
  const btns = [...${root}.querySelectorAll('button')].filter((b) => b.offsetParent !== null && b.textContent.includes(${JSON.stringify(text)}));
  if (!btns.length) return false;
  btns[0].click();
  return true;
})()`);
const buttonState = (text, root = 'document') => evaluate(`(() => {
  const b = [...${root}.querySelectorAll('button')].filter((x) => x.offsetParent !== null && x.textContent.includes(${JSON.stringify(text)}))[0];
  return b ? { disabled: b.disabled } : null;
})()`);

const SETUP = `(async () => {
  const { useStore, bindMachineEvents } = await import('/src/store/index.ts');
  const { setApi } = await import('/src/api/index.ts');
  const { createMockApi } = await import('/src/api/mock/index.ts');
  const { commands } = await import('/src/commands/index.ts');
  const { startLive } = await import('/src/features/live/service.ts');
  const { useQueue } = await import('/src/features/queue/queueStore.ts');
  const { useJobLog } = await import('/src/features/help/jobLog.ts');
  const { usePrompts } = await import('/src/features/history/prompt.ts');
  const V = (window.__q = { useStore, commands, useQueue, useJobLog, usePrompts });
  const s = () => useStore.getState();
  const api = createMockApi({ latencyMs: 0, statusIntervalMs: 200, minJobMs: 3500, jobSpeedup: 1e6 });
  V.api = api;
  V.starts = 0;
  const startJob = api.startJob.bind(api);
  api.startJob = (a) => { V.starts += 1; return startJob(a); };
  setApi(api);
  await bindMachineEvents(api, useStore);
  V.stopLive = startLive(useStore);
  s().setPref('uiMode', 'full');
  document.querySelectorAll('.toast button').forEach((b) => b.click());
  await s().loadDevices();
  s().setSelectedPort('virtual:falcon2');
  await s().refreshPorts?.();
  await s().connect();
  await new Promise((r) => setTimeout(r, 600));
  return { connected: s().connection.connected, tabs: s().tabs.length };
})()`;

try {
  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await send('Page.navigate', { url });
  await wait(3000);
  const info = await evaluate(SETUP);
  console.log('preparando…', JSON.stringify(info));
  check(info.connected, 'la máquina virtual quedó conectada');

  console.log('1. dos documentos en pestañas y la cola vacía');
  await evaluate(`(async () => {
    const { useStore, api } = { useStore: window.__q.useStore, api: window.__q.api };
    for (const [p, n] of [['/proyectos/caja.tlaku', 'caja'], ['/proyectos/tapa.tlaku', 'tapa']]) {
      const doc = await api.loadProject({ path: p });
      useStore.getState().openInTab(doc, p);
      useStore.getState().fitContent();
      await useStore.getState().computePreview();
    }
  })()`);
  check((await evaluate('window.__q.useStore.getState().tabs.length')) === 3, 'hay tres pestañas (la de arranque y las dos documentos)');
  await evaluate(`window.__q.commands.run('queue.open')`);
  await wait(400);
  check(await evaluate(`!!document.querySelector('.mc-modal') && document.querySelector('.mc-modal__title').textContent === 'Cola de trabajos'`), 'se abre el diálogo «Cola de trabajos»');
  await shot('01-cola-vacia.png');

  console.log('2. armar la cola: pestañas abiertas, copias y aviso por paso');
  check(await clickText('Agregar las pestañas abiertas'), 'botón «Agregar las pestañas abiertas»');
  await wait(300);
  check((await evaluate('window.__q.useQueue.getState().q.items.length')) === 3, 'la cola tiene las tres pestañas');
  // La pestaña de arranque («Sin título 1») no va en la cola: se quita con su botón.
  await evaluate(`document.querySelector('.queue__actions button[aria-label="Quitar de la cola"]').click()`);
  await wait(300);
  check((await evaluate('window.__q.useQueue.getState().q.items.length')) === 2, 'quedan los dos documentos');
  await evaluate(`(() => {
    const set = (el, v) => { const d = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value'); d.set.call(el, v); el.dispatchEvent(new Event('input', { bubbles: true })); };
    const copies = document.querySelectorAll('.queue__copies input');
    set(copies[0], '2');
    const notes = document.querySelectorAll('.queue__note');
    set(notes[1], 'Cambiar a MDF de 3 mm');
  })()`);
  await wait(300);
  const q2 = await evaluate('JSON.stringify(window.__q.useQueue.getState().q.items.map((i) => [i.name, i.copies, i.note]))');
  check(q2.includes('"Cambiar a MDF de 3 mm"') && q2.includes(',2,'), 'las copias y el aviso quedaron en la cola', q2);
  await shot('02-cola-armada.png');

  console.log('3. empezar la cola NO envía nada: pide confirmar el primer trabajo');
  check(await clickText('Empezar la cola'), 'botón «Empezar la cola»');
  await wait(500);
  check((await evaluate('window.__q.useQueue.getState().q.run')) === 'awaiting', 'la cola queda esperando confirmación');
  check((await evaluate('window.__q.starts')) === 0, 'la máquina no recibió ningún trabajo');
  const stepTitle = await evaluate(`[...document.querySelectorAll('.mc-modal__title')].map((e) => e.textContent).join('|')`);
  check(stepTitle.includes('Siguiente trabajo de la cola'), 'aparece «Siguiente trabajo de la cola»', stepTitle);
  const cont = await buttonState('Continuar al diálogo de inicio');
  check(cont?.disabled === true, '«Continuar» está deshabilitado hasta marcar todo');
  await shot('03-paso-sin-marcar.png');

  console.log('4. marcar las casillas, continuar y confirmar en el diálogo de inicio de siempre');
  await evaluate(`(() => { const modals = [...document.querySelectorAll('.mc-modal')]; const m = modals[modals.length - 1]; m.querySelectorAll('input[type=checkbox]').forEach((c) => c.click()); })()`);
  await wait(300);
  check((await buttonState('Continuar al diálogo de inicio'))?.disabled === false, 'con todo marcado «Continuar» se habilita');
  await shot('04-paso-marcado.png');
  check(await clickText('Continuar al diálogo de inicio'), 'botón «Continuar al diálogo de inicio»');
  await wait(900);
  const startTitle = await evaluate(`[...document.querySelectorAll('.mc-modal__title')].map((e) => e.textContent).join('|')`);
  check(startTitle.includes('¿Láser listo y área despejada?'), 'se abre el diálogo de inicio normal', startTitle);
  check((await evaluate('window.__q.starts')) === 0, 'sigue sin enviarse nada hasta confirmar ahí');
  await shot('05-dialogo-de-inicio.png');

  console.log('5. el trabajo corre; la vista en vivo muestra el paso de la cola');
  await evaluate(`window.__q.useStore.getState().setPref('liveView', true)`);
  check(await clickText('Iniciar trabajo'), 'botón «Iniciar trabajo»');
  await until(`window.__q.useStore.getState().jobState === 'running'`, 'trabajo en marcha');
  await until(`window.__q.useQueue.getState().q.run === 'running'`, 'cola en marcha');
  await wait(1200);
  check(await evaluate(`!!document.querySelector('[data-testid="queue-live-line"]')`), 'la vista en vivo muestra la línea de la cola');
  const liveLine = await evaluate(`document.querySelector('[data-testid="queue-live-line"]')?.textContent ?? ''`);
  check(liveLine.includes('copia 1 de 2'), 'la línea dice en qué paso va', liveLine);
  await shot('06-trabajo-en-curso.png');

  console.log('6. al terminar, la cola NO manda el siguiente: vuelve a pedir confirmación');
  await until(`window.__q.useStore.getState().jobState === 'finished'`, 'el trabajo terminó', 12000);
  await until(`window.__q.useQueue.getState().q.run === 'awaiting'`, 'la cola vuelve a esperar');
  await wait(2500);
  check((await evaluate('window.__q.starts')) === 1, 'tras 2,5 s la máquina sigue con un solo trabajo enviado');
  check((await evaluate(`window.__q.useStore.getState().jobState`)) === 'finished', 'la máquina quedó en reposo');
  check((await evaluate('window.__q.useQueue.getState().stepOpen')) === true, 'la confirmación del siguiente está a la vista');
  check((await evaluate('window.__q.useQueue.getState().q.items[0].done')) === 1, 'la primera copia quedó contada');
  const again = await evaluate(`[...document.querySelectorAll('.mc-modal__title')].map((e) => e.textContent).join('|')`);
  check(again.includes('Siguiente trabajo de la cola'), 'la segunda copia también pide confirmación', again);
  await shot('07-entre-trabajos.png');

  console.log('7. cancelar un trabajo detiene la cola y no ofrece el siguiente');
  await evaluate(`(() => { const modals = [...document.querySelectorAll('.mc-modal')]; const m = modals[modals.length - 1]; m.querySelectorAll('input[type=checkbox]').forEach((c) => c.click()); })()`);
  await wait(200);
  await clickText('Continuar al diálogo de inicio');
  await wait(900);
  await clickText('Iniciar trabajo');
  await until(`window.__q.useQueue.getState().q.run === 'running'`, 'segunda copia en marcha');
  await wait(600);
  await evaluate(`window.__q.useStore.getState().cancelJob()`);
  await until(`window.__q.useQueue.getState().q.run === 'stopped'`, 'la cola se detuvo');
  await wait(1500);
  check((await evaluate('window.__q.starts')) === 2, 'no se envió nada más tras cancelar');
  check((await evaluate('window.__q.useQueue.getState().q.stopReason')) === 'cancelled', 'motivo: cancelado');
  check((await evaluate('window.__q.useQueue.getState().q.items[1].status')) === 'pending', 'el trabajo siguiente quedó pendiente');
  await evaluate(`window.__q.commands.run('queue.open')`);
  await wait(500);
  await shot('08-cola-detenida.png');
  await evaluate(`window.__q.useQueue.getState().setDialogOpen(false)`);
  await wait(200);

  console.log('8. el historial: capas con ajustes, material, miniatura y notas');
  const entries = await evaluate(`(async () => { await window.__q.useJobLog.getState().load(); return window.__q.useJobLog.getState().entries.length; })()`);
  check(entries >= 2, 'el historial tiene los trabajos de la cola', String(entries));
  const hasLayers = await evaluate(`window.__q.useJobLog.getState().entries.some((e) => (e.layers?.length ?? 0) > 0 && e.thumb && e.thumb.startsWith('<svg'))`);
  check(hasLayers, 'las entradas traen capas con ajustes y miniatura');
  await evaluate(`window.__q.commands.run('help.jobLog')`);
  await wait(700);
  check(await evaluate(`document.querySelector('#help-joblog-title')?.textContent === 'Historial de trabajos'`), 'se abre el «Historial de trabajos»');
  check((await evaluate(`document.querySelectorAll('.joblog tbody tr').length`)) >= 2, 'la tabla lista los trabajos');
  check((await evaluate(`document.querySelectorAll('.joblog__thumb').length`)) >= 1, 'se ven miniaturas');
  await shot('09-historial.png');
  await evaluate(`document.querySelector('.joblog__open').click()`);
  await wait(500);
  check(await evaluate(`!!document.querySelector('[data-testid="joblog-detail"]')`), 'el detalle del trabajo se abre');
  await evaluate(`(() => {
    const t = document.querySelector('.joblog__notes textarea');
    const d = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value');
    d.set.call(t, 'Salió bien; subir 5 % de potencia');
    t.dispatchEvent(new Event('input', { bubbles: true }));
    t.dispatchEvent(new Event('focusout', { bubbles: true }));
    t.blur();
  })()`);
  await wait(500);
  check((await evaluate(`window.__q.useJobLog.getState().entries.some((e) => e.notes === 'Salió bien; subir 5 % de potencia')`)), 'las notas se guardan');
  await shot('10-detalle.png');
  // Filtro: por texto y por resultado.
  await evaluate(`(() => {
    const i = document.querySelector('.joblog__search');
    const d = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
    d.set.call(i, 'caja');
    i.dispatchEvent(new Event('input', { bubbles: true }));
  })()`);
  await wait(300);
  const rows = await evaluate(`[...document.querySelectorAll('.joblog tbody tr')].map((r) => r.textContent).join('|')`);
  check(rows.includes('caja') && !rows.includes('tapa'), 'el filtro de texto deja solo los trabajos de «caja»', rows.slice(0, 120));
  await shot('11-historial-filtrado.png');

  console.log('9. «Repetir con estos ajustes»: pide confirmación y no envía nada');
  const startsBefore = await evaluate('window.__q.starts');
  await evaluate(`(async () => {
    const s = window.__q.useStore.getState();
    // Cambia un ajuste de la capa para que haya algo que revertir.
    s.applyDocChange('layers.history.edit', (d) => ({ ...d, layers: d.layers.map((l, i) => (i === 0 ? { ...l, speed: l.speed + 777 } : l)) }));
  })()`);
  await evaluate(`[...document.querySelectorAll('.joblog__actions button')].find((b) => b.textContent.includes('Repetir con estos ajustes')).click()`);
  await wait(900);
  const prompt = await evaluate(`window.__q.usePrompts.getState().current ? window.__q.usePrompts.getState().current.title : ''`);
  check(prompt === 'Repetir con estos ajustes', 'aparece la confirmación', prompt);
  await shot('12-repetir-confirmacion.png');
  await evaluate(`window.__q.usePrompts.getState().answer('apply')`);
  await wait(600);
  check((await evaluate('window.__q.starts')) === startsBefore, 'aplicar los ajustes no envió nada a la máquina');
  check((await evaluate(`window.__q.useStore.getState().jobState !== 'running'`)), 'la máquina sigue sin trabajo en marcha');
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
