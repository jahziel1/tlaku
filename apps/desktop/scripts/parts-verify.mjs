// Verificación visual de «Generadores de piezas» con Chrome headless y el modo mock (la geometría la
// simula `src/api/mock/partsEngine.ts`; la real es la de `crates/tlaku-parts`):
//
//   node scripts/parts-verify.mjs                  # solo verifica
//   node scripts/parts-verify.mjs --shots DIR      # además guarda capturas
//
// Abre el diálogo, recorre los tres generadores, inserta una caja con rótulos, una bisagra y la pieza de
// prueba, regenera la caja en el mismo lugar y comprueba el resultado en el documento. Código de salida 1
// si algo falla. Variables: CHROME, APP_URL.
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
  vite = spawn('npx', ['vite', '--config', 'scripts/vite.stress.config.mjs', '--port', '1436', '--strictPort'], { cwd: appDir, stdio: 'ignore' });
  url = 'http://localhost:1436/';
  let up = false;
  for (let i = 0; i < 100 && !up; i++) { await wait(300); try { up = (await fetch(url)).ok; } catch { /* arrancando */ } }
  if (!up) { vite.kill(); throw new Error('Vite no arrancó'); }
}
const PORT = 9352;
const proc = spawn(bin, ['--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), 'ls-parts-'))}`, '--disable-gpu', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--hide-scrollbars', '--window-size=1440,900', '--force-device-scale-factor=1', url], { stdio: 'ignore' });
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

const click = async (sel) => {
  const r = await evaluate(`(() => { const e = document.querySelector(${JSON.stringify(sel)}); if (!e) return null; const b = e.getBoundingClientRect(); return [b.left + b.width / 2, b.top + b.height / 2]; })()`);
  if (!r) throw new Error(`No hay ${sel}`);
  await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: r[0], y: r[1], button: 'none' });
  await send('Input.dispatchMouseEvent', { type: 'mousePressed', x: r[0], y: r[1], button: 'left', buttons: 1, clickCount: 1 });
  await send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: r[0], y: r[1], button: 'left', buttons: 0, clickCount: 1 });
  await wait(150);
};
/** Clic en el botón con ese texto (dentro del diálogo si lo hay). */
const clickText = async (text) => {
  const marked = await evaluate(`(() => { const root = document.querySelector('[role="dialog"]') || document.body; const b = [...root.querySelectorAll('button')].find((x) => x.textContent.trim().includes(${JSON.stringify(text)})); if (!b) return false; b.setAttribute('data-vclick', '1'); return true; })()`);
  if (!marked) throw new Error(`No hay un botón «${text}»`);
  await click('[data-vclick="1"]');
  await evaluate(`document.querySelectorAll('[data-vclick]').forEach((b) => b.removeAttribute('data-vclick'))`);
};
const setField = async (label, value) => {
  await evaluate(`(() => {
    const l = [...document.querySelectorAll('[role="dialog"] label')].find((x) => x.textContent === ${JSON.stringify(label)});
    const i = document.getElementById(l.htmlFor);
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, ${JSON.stringify(String(value))});
    i.dispatchEvent(new Event('input', { bubbles: true }));
    i.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  })()`);
  await wait(450);
};
const setSelect = async (label, value) => {
  await evaluate(`(() => {
    const l = [...document.querySelectorAll('[role="dialog"] label')].find((x) => x.textContent === ${JSON.stringify(label)});
    const i = document.getElementById(l.htmlFor);
    Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(i, ${JSON.stringify(value)});
    i.dispatchEvent(new Event('change', { bubbles: true }));
  })()`);
  await wait(450);
};
const waitFor = async (expr, what) => { for (let i = 0; i < 60; i++) { if (await evaluate(expr)) return; await wait(100); } throw new Error(`Se agotó la espera: ${what}`); };

try {
  await waitFor(`!!document.querySelector('#canvas-main')`, 'el lienzo');
  await wait(2500); // la app carga su diseño de ejemplo al arrancar
  await evaluate(`(async () => {
    const { useStore } = await import('/src/store/index.ts');
    const { commands } = await import('/src/commands/index.ts');
    window.__v = { useStore, commands };
    const s = useStore.getState();
    s.setPref('uiMode', 'full');
    // La app arranca con un diseño de ejemplo: se parte de la cama vacía.
    s.applyDocChange('history.delete', (d) => ({ ...d, objects: [] }));
    document.querySelectorAll('.toast button').forEach((b) => b.click());
  })()`);
  await wait(500);
  check(await evaluate(`window.__v.useStore.getState().doc.objects.length`) === 0, 'cama vacía al empezar');

  console.log('Diálogo');
  await evaluate(`window.__v.commands.run('parts.open')`);
  await waitFor(`!!document.querySelector('[data-testid="parts-preview"] [data-part]')`, 'la vista previa de la caja');
  await wait(300);
  check(await evaluate(`document.querySelectorAll('[data-testid="parts-preview"] [data-part]').length`) === 6, 'la caja cerrada se previsualiza con 6 piezas');
  check(await evaluate(`[...document.querySelectorAll('.parts__gen-name')].map((n) => n.textContent).join('|')`) === 'Caja con dientes|Bandeja y cajón|Bisagra flexible|Prueba de encaje|Engranajes|Uniones (cola de milano, pestaña-ranura)|Caja redondeada', 'lista de generadores (7)');
  check(await evaluate(`[...document.querySelectorAll('.parts__gen')].every((b) => !!b.querySelector('svg'))`) && await evaluate(`document.querySelectorAll('.parts__gen').length`) === 7, 'los 7 generadores llevan icono');
  await shot('01-dialogo-caja.png');

  await setField('Grosor del material', 6);
  await setSelect('Tipo', 'open');
  check(await evaluate(`document.querySelectorAll('[data-testid="parts-preview"] [data-part]').length`) === 5, 'caja abierta: 5 piezas');
  await setField('Grosor del material', 3);

  await clickText('Bisagra flexible');
  await wait(450);
  await setSelect('Forma de los cortes', 'wave');
  check(await evaluate(`document.querySelectorAll('[data-testid="parts-preview"] path').length`) > 20, 'bisagra ondulada con muchos cortes');
  await shot('02-dialogo-bisagra.png');

  await clickText('Prueba de encaje');
  await wait(450);
  check(await evaluate(`document.querySelectorAll('[data-testid="parts-preview"] [data-part]').length`) === 2, 'prueba de encaje: placa y macho');
  await shot('03-dialogo-prueba-encaje.png');

  for (const [name, file] of [['Bandeja y cajón', '03b-dialogo-bandeja.png'], ['Engranajes', '03c-dialogo-engranajes.png'], ['Uniones', '03d-dialogo-uniones.png'], ['Caja redondeada', '03e-dialogo-caja-redondeada.png']]) {
    await clickText(name);
    await wait(500);
    check(await evaluate(`document.querySelectorAll('[data-testid="parts-preview"] [data-part]').length`) > 0, `${name}: la vista previa muestra piezas`);
    await shot(file);
  }

  console.log('Insertar una caja con rótulos');
  await clickText('Caja con dientes');
  await wait(450);
  await setSelect('Tipo', 'closed');
  await evaluate(`[...document.querySelectorAll('label.parts__check')].find((l) => l.textContent.includes('Grabar el nombre')).querySelector('input').click()`);
  await wait(450);
  await shot('04-dialogo-caja-rotulos.png');
  await clickText('Insertar');
  await wait(400);
  const st = await evaluate(`(() => { const s = window.__v.useStore.getState(); return { n: s.doc.objects.length, groups: [...new Set(s.doc.objects.map((o) => o.group))], layers: s.doc.layers.map((l) => l.name), rec: s.doc.generators, sel: s.selectedIds.length }; })()`);
  check(st.n === 12 && st.groups.length === 1, 'se insertaron 12 objetos (6 cortes + 6 rótulos) en un grupo', JSON.stringify(st.layers));
  check(st.rec?.length === 1 && st.rec[0].generator === 'box', 'el grupo guarda sus parámetros para regenerar');
  await evaluate(`window.__v.useStore.getState().fitContent()`);
  await wait(500);
  check(await evaluate(`!!document.querySelector('[data-testid="parts-bar"]')`), 'al seleccionar el grupo aparece «Regenerar…»');
  await shot('05-caja-insertada.png');

  console.log('Regenerar en el mismo lugar');
  await click('[data-testid="parts-bar"] button');
  await waitFor(`!!document.querySelector('[data-testid="parts-preview"] [data-part]')`, 'la vista previa al regenerar');
  await wait(300);
  check(await evaluate(`document.querySelector('[role="dialog"] h2').textContent`) === 'Regenerar piezas', 'el diálogo es de regenerar');
  check(await evaluate(`(() => { const l = [...document.querySelectorAll('[role="dialog"] label')].find((x) => x.textContent === 'Ancho (X)'); return document.getElementById(l.htmlFor).value; })()`) === '100', 'trae los parámetros guardados');
  await setField('Ancho (X)', 70);
  await shot('06-dialogo-regenerar.png');
  await clickText('Regenerar');
  await wait(400);
  const after = await evaluate(`(() => { const s = window.__v.useStore.getState(); return { n: s.doc.objects.length, groups: [...new Set(s.doc.objects.map((o) => o.group))].length, w: s.doc.generators[0].params.width_mm }; })()`);
  check(after.n === 12 && after.groups === 1 && after.w === 70, 'regenerar reemplazó las piezas del grupo', JSON.stringify(after));
  await evaluate(`window.__v.useStore.getState().fitContent()`);
  await wait(400);
  await shot('07-caja-regenerada.png');

  console.log('Otras piezas junto a la caja');
  await evaluate(`window.__v.commands.run('parts.open')`);
  await waitFor(`!!document.querySelector('[data-testid="parts-preview"]')`, 'diálogo');
  await clickText('Bisagra flexible');
  await wait(400);
  await clickText('Insertar');
  await wait(400);
  await evaluate(`window.__v.commands.run('parts.open')`);
  await waitFor(`!!document.querySelector('[data-testid="parts-preview"]')`, 'diálogo');
  await clickText('Prueba de encaje');
  await wait(400);
  await clickText('Insertar');
  await wait(400);
  const all = await evaluate(`(() => { const s = window.__v.useStore.getState(); return { groups: [...new Set(s.doc.objects.map((o) => o.group))].length, recs: s.doc.generators.map((r) => r.generator) }; })()`);
  check(all.groups === 3 && all.recs.join() === 'box,hinge,fit_test', 'tres grupos, uno por generador', JSON.stringify(all));
  await evaluate(`window.__v.useStore.getState().select([]); window.__v.useStore.getState().fitContent()`);
  await wait(500);
  await shot('08-tres-grupos.png');

  console.log('Inglés');
  await evaluate(`window.__v.useStore.getState().setPref('language', 'en')`);
  await wait(400);
  await evaluate(`window.__v.commands.run('parts.open')`);
  await waitFor(`!!document.querySelector('[data-testid="parts-preview"]')`, 'diálogo en inglés');
  await clickText('Fit test');
  await wait(450);
  check(await evaluate(`document.querySelector('[role="dialog"] h2').textContent`) === 'Part generators', 'el diálogo está en inglés');
  await shot('09-dialog-english.png');
  await clickText('Cancel');
} catch (e) {
  console.error(e);
  failures.push(String(e));
}
console.log(failures.length === 0 ? 'Todo bien.' : `Fallas: ${failures.length}`);
finish(failures.length === 0 ? 0 : 1);
