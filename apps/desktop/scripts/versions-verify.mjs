// Verificación visual del historial de versiones con Chrome headless y el modo mock (el motor real es
// `src-tauri/src/versions/`; el simulado, `src/api/mock/versions.ts`):
//
//   node scripts/versions-verify.mjs                  # solo verifica
//   node scripts/versions-verify.mjs --shots DIR      # además guarda capturas
//
// Importa el diseño de ejemplo, lo guarda varias veces con ediciones distintas, guarda una versión con nombre
// por el menú, abre «Historial de versiones…», mira una versión (vista previa y comparación), la restaura como
// pestaña nueva y reemplazando el documento (con confirmación), deshace, abre los ajustes y repite en inglés y
// tema claro. Código de salida 1 si algo falla. Variables: CHROME, APP_URL.
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
const PORT = 9358;
const proc = spawn(bin, ['--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), 'ls-versions-'))}`, '--disable-gpu', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--hide-scrollbars', '--window-size=1440,900', '--force-device-scale-factor=1', 'about:blank'], { stdio: 'ignore' });
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
const click = (selector) => evaluate(`(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) throw new Error('sin ${selector}'); el.click(); })()`);
const clickText = (selector, text) => evaluate(`(() => { const el = [...document.querySelectorAll(${JSON.stringify(selector)})].find(e => e.textContent.trim().startsWith(${JSON.stringify(text)})); if (!el) throw new Error('sin ${text}'); el.click(); })()`);
const app = (code) => evaluate(`(async () => {
  const { useStore } = await import('/src/store/index.ts');
  const { commands } = await import('/src/commands/index.ts');
  const { getApi } = await import('/src/api/index.ts');
  const st = () => useStore.getState();
  const api = getApi();
  ${code}
})()`);
/** Escribe en el campo enfocado/indicado como el teclado real (para que React lo vea). */
const typeInto = async (selector, text) => { await click(selector); await send('Input.insertText', { text }); await wait(100); };

try {
  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await send('Page.navigate', { url });
  await wait(3500);
  await app(`st().setPref('language', 'es'); st().setPref('uiMode', 'full'); st().setPref('theme', 'dark'); document.querySelectorAll('.toast button').forEach((b) => b.click());`);

  console.log('1. un proyecto guardado varias veces');
  await app(`await commands.run('file.import'); await new Promise((r) => setTimeout(r, 400)); st().fitContent();`);
  await app(`await commands.run('file.save'); await new Promise((r) => setTimeout(r, 300));`);
  const path = await app(`return st().filePath;`);
  check(!!path, 'el proyecto quedó con archivo', path);
  // Edición 1 y guardado: conserva lo que había en el archivo.
  await app(`const ids = st().doc.objects.map((o) => o.id); st().deleteObjects([ids[0]]); await commands.run('file.save'); await new Promise((r) => setTimeout(r, 300));`);
  // Edición 2 y versión con nombre desde el menú.
  await app(`const ids = st().doc.objects.map((o) => o.id); st().deleteObjects([ids[0]]);`);
  await click('.canvas-area');
  await clickText('.menubar__trigger', 'Archivo');
  await wait(300);
  const menuHas = await evaluate(`[...document.querySelectorAll('[role="menu"] [role="menuitem"]')].map((e) => e.textContent.trim().replace(/\\s+/g, ' '))`);
  check(menuHas.some((t) => t.startsWith('Historial de versiones…')), 'Archivo > «Historial de versiones…»');
  check(menuHas.some((t) => t.startsWith('Guardar versión…')), 'Archivo > «Guardar versión…»');
  await shot('01-menu-archivo.png');
  await evaluate(`[...document.querySelectorAll('[role="menu"] [role="menuitem"]')].find((e) => e.textContent.trim().startsWith('Guardar versión…')).click()`);
  await wait(400);
  check(!!(await evaluate(`!!document.querySelector('.ver-name')`)), 'se abre «Guardar versión…»');
  await typeInto('.ver-name input', 'Antes de quitar los taladros');
  await shot('02-guardar-version.png');
  await evaluate(`document.querySelector('.ver-name button[type="submit"]').click()`);
  await wait(500);
  check(await evaluate(`!document.querySelector('.ver-name')`), 'el diálogo se cierra al guardar');
  // Edición 3 y una instantánea automática (lo que haría el temporizador).
  await app(`const ids = st().doc.objects.map((o) => o.id); st().deleteObjects([ids[0]]); await api.versionsSnapshot({ path: st().filePath, doc: st().doc, kind: 'auto' });`);

  console.log('2. el diálogo');
  await app(`await commands.run('versions.open');`);
  await wait(700);
  const rows = await evaluate(`[...document.querySelectorAll('.ver-row')].map((r) => r.textContent.replace(/\\s+/g, ' ').trim())`);
  check(rows.length === 3, 'tres versiones: automática, con nombre y guardado', JSON.stringify(rows));
  check(rows[0].includes('Automática') && rows[1].includes('Antes de quitar los taladros') && rows[2].includes('Guardado'), 'de la más nueva a la más vieja con su tipo');
  check((await evaluate(`document.querySelectorAll('.ver-row img.ver-thumb').length`)) === 3, 'las tres tienen miniatura');
  await shot('03-historial-lista.png');

  await evaluate(`document.querySelectorAll('.ver-row')[2].click()`);
  await wait(500);
  const compare = await evaluate(`document.querySelector('.ver-compare').textContent`);
  check(/se perderían \d+ objetos? agregados? después|Se perder/.test(compare) || compare.includes('Volver'), 'la comparación dice qué cambiaría', compare.slice(0, 120));
  check(await evaluate(`!!document.querySelector('.ver-detail img.ver-thumb--large')`), 'vista previa grande');
  await shot('04-version-elegida.png');

  console.log('3. restaurar como pestaña nueva');
  const objsBefore = await app(`return st().doc.objects.length;`);
  await clickText('.ver-detail__actions .btn', 'Restaurar como pestaña nueva');
  await wait(800);
  const afterNew = await app(`return { tabs: st().tabs.length, objs: st().doc.objects.length, path: st().filePath, dirty: st().dirty };`);
  check(afterNew.tabs === 2 && afterNew.path === null && afterNew.dirty, 'se abrió una pestaña nueva sin archivo y con cambios sin guardar', JSON.stringify(afterNew));
  check(afterNew.objs > objsBefore, 'trae los objetos que ya no estaban', `${afterNew.objs} > ${objsBefore}`);
  check(await evaluate(`!document.querySelector('.ver-card')`), 'el diálogo se cerró');
  await shot('05-pestana-nueva.png');
  await app(`const t = st().tabs.find((e) => e.id !== st().activeTabId); st().activateTab(t.id);`);
  const orig = await app(`return { objs: st().doc.objects.length, path: st().filePath };`);
  check(orig.objs === objsBefore && orig.path === path, 'el documento abierto no se tocó', JSON.stringify(orig));

  console.log('4. reemplazar el documento (con confirmación) y deshacer');
  // Un cambio nuevo sin guardar: es lo que debe quedar como «Antes de restaurar» (si no cambió, la última versión ya lo guarda).
  await app(`const ids = st().doc.objects.map((o) => o.id); st().deleteObjects([ids[0]]);`);
  const objsEdited = await app(`return st().doc.objects.length;`);
  await app(`await commands.run('versions.open');`);
  await wait(600);
  await evaluate(`document.querySelectorAll('.ver-row')[1].click()`);
  await wait(500);
  await clickText('.ver-detail__actions .btn', 'Reemplazar el documento abierto');
  await wait(500);
  const prompt = await evaluate(`document.querySelector('.modal__title')?.textContent`);
  check(prompt === 'Reemplazar el documento' || (await evaluate(`[...document.querySelectorAll('.modal__title')].map((e) => e.textContent).includes('Reemplazar el documento')`)), 'pide confirmar encima del historial');
  await shot('06-confirmar-reemplazo.png');
  await clickText('.modal__foot .btn', 'Restaurar');
  await wait(900);
  const replaced = await app(`return { objs: st().doc.objects.length, dirty: st().dirty, past: st().past.length, versions: (await api.versionsList({ path: st().filePath })).versions.map((v) => v.kind) };`);
  check(replaced.objs > objsEdited && replaced.dirty, 'el documento se reemplazó', JSON.stringify(replaced));
  check(replaced.versions.includes('restore'), 'el estado anterior quedó como versión «Antes de restaurar»');
  await shot('07-documento-reemplazado.png');
  await app(`st().undo();`);
  check((await app(`return st().doc.objects.length;`)) === objsEdited, 'Deshacer devuelve el estado anterior en un solo paso');

  console.log('5. ajustes, otros proyectos, inglés y tema claro');
  await app(`await api.versionsSnapshot({ path: '/proyectos/ajeno.tlaku', doc: st().doc, kind: 'manual', label: 'Entrega al cliente' }); await commands.run('versions.open');`);
  await wait(600);
  await evaluate(`document.querySelector('.ver-settings').open = true`);
  await wait(300);
  check(await evaluate(`!!document.querySelector('.ver-toolbar__projects select option[value="/proyectos/ajeno.tlaku"]')`), 'ofrece el historial de otro proyecto');
  await shot('08-ajustes.png');
  await app(`st().setPref('language', 'en'); st().setPref('theme', 'light');`);
  await wait(500);
  await evaluate(`document.querySelectorAll('.ver-row')[0].click()`);
  await wait(500);
  check((await evaluate(`document.querySelector('.ver-card .modal__title').textContent`)) === 'Version history', 'título en inglés');
  await shot('09-ingles-tema-claro.png');
  await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape' });
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape' });
  await wait(300);
  check(await evaluate(`!document.querySelector('.ver-card')`), 'Esc cierra el historial');
} catch (e) {
  console.error('ERROR', e);
  failures.push(String(e));
}
console.log(failures.length === 0 ? '\nTodo bien.' : `\nFallos: ${failures.length}\n- ${failures.join('\n- ')}`);
finish(failures.length === 0 ? 0 : 1);
