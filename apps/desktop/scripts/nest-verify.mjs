// Verificación visual del acomodo con varias hojas, retazos y veta, con Chrome headless y el modo mock
// (el acomodo lo simula `src/api/mock/nest.ts`; el real es `crates/tlaku-nest`):
//
//   node scripts/nest-verify.mjs                  # solo verifica
//   node scripts/nest-verify.mjs --shots DIR      # además guarda capturas
//
// Recorre: piezas con copias de sobra → hojas nuevas → veta/sin giro por pieza → guardar el sobrante como
// retazo → ver el inventario → usarlo antes que una hoja nueva → aplicar (contornos de hoja en la capa de
// guías) → borrar el retazo → inglés. Código de salida 1 si algo falla. Variables: CHROME, APP_URL.
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
const proc = spawn(bin, ['--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), 'ls-nest-'))}`, '--disable-gpu', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--hide-scrollbars', '--window-size=1440,900', '--force-device-scale-factor=1', url], { stdio: 'ignore' });
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

/** Diálogo de más arriba (el del inventario o una pregunta se abre encima del de acomodo). */
const TOP = `(() => { const all = [...document.querySelectorAll('[role="dialog"], [role="alertdialog"]')]; return all[all.length - 1]; })()`;
const click = async (sel) => {
  const r = await evaluate(`(() => { const e = document.querySelector(${JSON.stringify(sel)}); if (!e) return null; e.scrollIntoView({ block: 'center' }); const b = e.getBoundingClientRect(); return [b.left + b.width / 2, b.top + b.height / 2]; })()`);
  if (!r) throw new Error(`No hay ${sel}`);
  await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: r[0], y: r[1], button: 'none' });
  await send('Input.dispatchMouseEvent', { type: 'mousePressed', x: r[0], y: r[1], button: 'left', buttons: 1, clickCount: 1 });
  await send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: r[0], y: r[1], button: 'left', buttons: 0, clickCount: 1 });
  await wait(200);
};
/** Clic en el botón con ese texto (dentro del diálogo de más arriba). */
const clickText = async (text) => {
  const marked = await evaluate(`(() => { const root = ${TOP} || document.body; const b = [...root.querySelectorAll('button')].find((x) => x.textContent.trim().includes(${JSON.stringify(text)})); if (!b) return false; b.setAttribute('data-vclick', '1'); return true; })()`);
  if (!marked) throw new Error(`No hay un botón «${text}»`);
  await click('[data-vclick="1"]');
  await evaluate(`document.querySelectorAll('[data-vclick]').forEach((b) => b.removeAttribute('data-vclick'))`);
};
const setField = async (label, value) => {
  await evaluate(`(() => {
    const l = [...(${TOP}).querySelectorAll('label')].find((x) => x.textContent === ${JSON.stringify(label)});
    const i = document.getElementById(l.htmlFor);
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, ${JSON.stringify(String(value))});
    i.dispatchEvent(new Event('input', { bubbles: true }));
    i.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    i.dispatchEvent(new Event('focusout', { bubbles: true }));
    i.blur();
  })()`);
  await wait(350);
};
const setSelect = async (label, value) => {
  await evaluate(`(() => {
    const l = [...(${TOP}).querySelectorAll('label')].find((x) => x.textContent === ${JSON.stringify(label)});
    const i = document.getElementById(l.htmlFor);
    Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(i, ${JSON.stringify(value)});
    i.dispatchEvent(new Event('change', { bubbles: true }));
  })()`);
  await wait(350);
};
const setCheck = async (labelText, on) => {
  const ok = await evaluate(`(() => { const l = [...(${TOP}).querySelectorAll('label.ar-check')].find((x) => x.textContent.includes(${JSON.stringify(labelText)})); if (!l) return false; const i = l.querySelector('input'); if (i.checked !== ${on}) i.click(); return true; })()`);
  if (!ok) throw new Error(`No hay la casilla «${labelText}»`);
  await wait(300);
};
const waitFor = async (expr, what) => { for (let i = 0; i < 80; i++) { if (await evaluate(expr)) return; await wait(100); } throw new Error(`Se agotó la espera: ${what}`); };
const text = (sel) => evaluate(`document.querySelector(${JSON.stringify(sel)})?.textContent ?? ''`);
const runNest = async () => {
  await clickText('Empezar');
  await waitFor(`(() => { const d = ${TOP}; return !!d && [...d.querySelectorAll('button')].some((b) => b.textContent.trim() === 'Empezar'); })()`, 'que termine el acomodo');
  await wait(300);
};

try {
  await waitFor(`!!document.querySelector('#canvas-main')`, 'el lienzo');
  await wait(2500); // la app carga su diseño de ejemplo al arrancar
  await evaluate(`(async () => {
    const { useStore } = await import('/src/store/index.ts');
    const { commands } = await import('/src/commands/index.ts');
    const { polylineBez } = await import('/src/lib/bezpath.ts');
    window.__v = { useStore, commands, polylineBez };
    const s = useStore.getState();
    s.setPref('uiMode', 'full');
    s.setPref('language', 'es');
    // Piezas: tres tamaños de rectángulo y una pieza en L; la cama del mock es de 600 x 400 mm.
    const layer = s.doc.layers[0].color;
    const mk = (id, pts, x) => ({ id, layer, transform: [1, 0, 0, 1, x, 0], kind: { type: 'vector', paths: [polylineBez(pts, true)] }, locked: false, hidden: false, group: null });
    const rect = (w, h) => [[0, 0], [w, 0], [w, h], [0, h]];
    const ele = [[0, 0], [90, 0], [90, 40], [40, 40], [40, 90], [0, 90]];
    s.applyDocChange('history.delete', (d) => ({ ...d, objects: [mk('panel', rect(150, 100), 0), mk('tapa', rect(110, 80), 200), mk('ele', ele, 360)] }));
    s.select(['panel', 'tapa', 'ele']);
    document.querySelectorAll('.toast button').forEach((b) => b.click());
  })()`);
  await wait(500);
  check(await evaluate(`window.__v.useStore.getState().doc.objects.length`) === 3, 'tres piezas en la cama');

  console.log('Diálogo: piezas, giros por pieza, hojas y retazos');
  await evaluate(`window.__v.commands.run('arrange.nest')`);
  await waitFor(`!!document.querySelector('[role="dialog"] .ne-cols')`, 'el diálogo de acomodo');
  check((await text('[role="dialog"]')).includes('Hojas y retazos'), 'el diálogo tiene la sección «Hojas y retazos»');
  check(await evaluate(`document.querySelectorAll('[role="dialog"] select.ne-rot').length`) === 3, 'cada pieza tiene su selector de giros');
  check(await evaluate(`[...document.querySelectorAll('[role="dialog"] select.ne-rot')[0].options].map((o) => o.textContent).join('|')`) === 'Según las opciones|Sin giro|Veta (0° y 180°)', 'los giros por pieza: opciones, sin giro y veta');
  await shot('01-dialogo.png');

  console.log('Una sola hoja: lo que no cabe se reporta');
  await setField('Copias de todas', 8);
  await runNest();
  let st = await evaluate(`(() => document.querySelectorAll('[role="dialog"] .ne-sheetrow').length)()`);
  check(st === 1, 'con una sola hoja, un resumen de hoja', String(st));
  check((await text('[role="dialog"] .warn-line')).includes('no cupieron'), 'avisa de las piezas que no cupieron');
  await shot('02-una-hoja.png');

  console.log('Seguir en hojas nuevas');
  await setCheck('Seguir en hojas nuevas', true);
  await setField('Máximo de hojas nuevas', 5);
  await runNest();
  const sheetsText = await text('[role="dialog"] .ne-sheets');
  const nSheets = await evaluate(`document.querySelectorAll('[role="dialog"] .ne-sheetrow').length`);
  check(nSheets >= 2, 'las piezas se reparten en dos hojas o más', `${nSheets} hojas`);
  check(!(await text('[role="dialog"] .ne-side')).includes('no cupieron'), 'ya no queda nada sin colocar');
  check(sheetsText.includes('Hoja 1') && sheetsText.includes('Hoja 2') && sheetsText.includes('%'), 'cada hoja muestra su aprovechamiento', sheetsText.replace(/\s+/g, ' ').slice(0, 120));
  check(await evaluate(`document.querySelectorAll('[role="dialog"] .ne-preview .ne-sheet').length`) === nSheets, 'la vista previa dibuja cada hoja con su contorno');
  check(await evaluate(`document.querySelectorAll('[role="dialog"] .ne-sheet-label').length`) === nSheets, 'y su número');
  await shot('03-varias-hojas.png');

  console.log('Veta y sin giro');
  await setSelect('Giros de todas las piezas', 'grain');
  check(await evaluate(`[...document.querySelectorAll('[role="dialog"] select.ne-rot')].every((s) => s.value === 'grain')`), 'todas las piezas con veta');
  await runNest();
  check(await evaluate(`document.querySelectorAll('[role="dialog"] .ne-sheetrow').length`) >= 2, 'con veta también se reparte en hojas');
  await shot('04-veta.png');
  await setSelect('Giros de todas las piezas', 'none');
  await runNest();
  await setSelect('Giros de todas las piezas', 'inherit');

  console.log('Guardar el sobrante como retazo');
  await setField('Copias de todas', 2);
  await setField('Material', 'Triplay');
  await setField('Grosor', 3);
  await runNest();
  check((await text('[role="dialog"] .ne-sheets')).includes('Sobra'), 'se ofrece el sobrante de la hoja');
  await shot('05-sobrante.png');
  await clickText('Guardar el sobrante como retazo');
  await wait(400);
  const remnants = await evaluate(`(async () => { const { useRemnants } = await import('/src/features/nest/remnantStore.ts'); return useRemnants.getState().library.remnants.map((r) => [r.material, r.thickness_mm, r.width_mm, r.height_mm, r.rect]); })()`);
  check(remnants.length === 1 && remnants[0][0] === 'Triplay' && remnants[0][1] === 3, 'el retazo se guardó con material y grosor', JSON.stringify(remnants));
  check((await text('[role="dialog"] .ne-sheets')).includes('Guardado en retazos'), 'el botón queda como «Guardado en retazos»');

  console.log('Inventario de retazos');
  await clickText('Retazos guardados…');
  await waitFor(`document.querySelectorAll('[role="dialog"]').length === 2`, 'el inventario');
  await wait(300);
  check(await evaluate(`document.querySelectorAll('.ne-rem').length`) === 1, 'el inventario lista el retazo');
  check((await text('.ne-rem')).includes('mm'), 'con sus medidas');
  await shot('06-inventario.png');
  await clickText('Cerrar');
  await wait(300);

  console.log('Usar el retazo antes que una hoja nueva');
  await setCheck('Usar antes los retazos guardados', true);
  check((await text('[role="dialog"]')).includes('Coincide 1 retazo guardado'), 'dice que coincide 1 retazo');
  await setField('Copias de todas', 1);
  await runNest();
  const rs = await evaluate(`[...document.querySelectorAll('[role="dialog"] .ne-sheetrow__head')].map((p) => p.textContent.replace(/\\s+/g, ' ')).join(' | ')`);
  check(rs.includes('retazo «'), 'la primera hoja es el retazo guardado', rs);
  await shot('07-con-retazo.png');

  console.log('Aplicar: hojas en fila con su contorno de referencia');
  await clickText('Aplicar');
  await wait(600);
  const doc = await evaluate(`(() => { const s = window.__v.useStore.getState(); return { n: s.doc.objects.length, refs: s.doc.objects.filter((o) => o.id.startsWith('hoja-')).map((o) => [o.id, o.layer]), guide: s.doc.layers.find((l) => l.color === '#7e8aa0')?.output, sel: s.selectedIds.length }; })()`);
  check(doc.refs.length >= 1 && doc.refs.every((r) => r[1] === '#7e8aa0') && doc.guide === false, 'el contorno de la hoja va en la capa de guías (no se corta)', JSON.stringify(doc));
  const remLeft = await evaluate(`(async () => { const { useRemnants } = await import('/src/features/nest/remnantStore.ts'); return useRemnants.getState().library.remnants.length; })()`);
  check(remLeft === 0, 'el retazo usado salió del inventario', String(remLeft));
  await evaluate(`window.__v.useStore.getState().select([]); window.__v.useStore.getState().fitContent()`);
  await wait(600);
  await shot('08-aplicado.png');
  await evaluate(`window.__v.useStore.getState().undo()`);
  await wait(300);

  console.log('Inglés');
  await evaluate(`window.__v.useStore.getState().setPref('language', 'en')`);
  await wait(400);
  await evaluate(`window.__v.useStore.getState().select(['panel', 'tapa', 'ele']); window.__v.commands.run('arrange.nest')`);
  await waitFor(`!!document.querySelector('[role="dialog"] .ne-cols')`, 'el diálogo en inglés');
  check((await text('[role="dialog"]')).includes('Sheets and offcuts'), 'el diálogo está en inglés');
  await shot('09-english.png');
  await clickText('Cancel');
} catch (e) {
  console.error(e);
  failures.push(String(e));
}
console.log(failures.length === 0 ? 'Todo bien.' : `Fallas: ${failures.length}`);
finish(failures.length === 0 ? 0 : 1);
