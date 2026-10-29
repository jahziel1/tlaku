// Verificación de los componentes guardados (issue #22) en el ESCRITORIO con Chrome headless y la máquina
// simulada (mock), con ratón y pantalla de verdad:
//
//   node scripts/components-verify.mjs                          # español, tema oscuro (verifica)
//   node scripts/components-verify.mjs --shots DIR              # además guarda capturas
//   node scripts/components-verify.mjs --lang en --theme light  # otro idioma / tema
//   node scripts/components-verify.mjs --matrix --shots DIR     # los cuatro: es/en × claro/oscuro
//
// Comprueba que:
//   (a) dos objetos en capas distintas, seleccionados, se guardan con «Guardar como componente…» (menú contextual)
//       y el nombre elegido;
//   (b) la Biblioteca de arte › «Mis componentes» muestra miniatura, nombre y tamaño;
//   (c) insertarlo en una pestaña nueva lo deja en su posición original, con sus objetos y capas (con los ajustes);
//   (d) Deshacer lo quita en UN solo paso;
//   (e) renombrar y borrar (el borrado pide confirmación y «Conservar» no borra) funcionan;
//   (f) en cada paso no hay botones sin texto ni controles que se salgan de su caja.
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
  vite = spawn('npx', ['vite', '--config', 'scripts/vite.stress.config.mjs', '--port', '1438', '--strictPort'], { cwd: appDir, stdio: 'ignore' });
  url = 'http://localhost:1438/';
  let up = false;
  for (let i = 0; i < 100 && !up; i++) { await wait(300); try { up = (await fetch(url)).ok; } catch { /* arrancando */ } }
  if (!up) { vite.kill(); throw new Error('Vite no arrancó'); }
}
const PORT = 9354;
const userData = mkdtempSync(join(tmpdir(), 'ls-components-'));
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

const SETUP = (lang, theme) => `(async () => {
  const { useStore, bindMachineEvents } = await import('/src/store/index.ts');
  const { setApi } = await import('/src/api/index.ts');
  const { createMockApi } = await import('/src/api/mock/index.ts');
  const { commands } = await import('/src/commands/index.ts');
  const { t } = await import('/src/i18n/index.ts');
  const { useArtPanel } = await import('/src/features/art/artStore.ts');
  const { usePrompts } = await import('/src/features/history/prompt.ts');
  const { makeLayer } = await import('/src/lib/layers.ts');
  const { polylineBez } = await import('/src/lib/bezpath.ts');
  const { objectBoundsWithCopies } = await import('/src/lib/virtualArray.ts');
  const { unionBounds } = await import('/src/lib/geometry.ts');
  const { worldToScreen } = await import('/src/lib/viewport.ts');
  const V = (window.__v = { useStore, commands, t, useArtPanel, usePrompts, makeLayer, polylineBez, objectBoundsWithCopies, unionBounds, worldToScreen });
  const s = () => useStore.getState();
  const api = createMockApi({ latencyMs: 0, statusIntervalMs: 200 });
  V.api = api;
  setApi(api);
  await bindMachineEvents(api, useStore);
  s().setPref('uiMode', 'full');
  s().setPref('language', ${JSON.stringify(lang)});
  s().setPref('theme', ${JSON.stringify(theme)});
  document.querySelectorAll('.toast button').forEach((b) => b.click());
  await s().loadDevices();
  return { lang: document.documentElement.lang, theme: document.documentElement.dataset.theme ?? null, tabs: s().tabs.length };
})()`;

async function scenario(lang, theme) {
  prefix = `${lang}-${theme}-`;
  console.log(`\n=== ${lang} · ${theme} ===`);
  await send('Page.navigate', { url });
  await wait(3000);
  const info = await evaluate(SETUP(lang, theme));
  console.log('preparando…', JSON.stringify(info));
  await wait(400);

  console.log('1. dos objetos en capas distintas, seleccionados');
  await evaluate(`(() => {
    const V = window.__v;
    const sq = (id, layer, x, y, w, h) => ({ id, layer, transform: [1, 0, 0, 1, x, y], kind: { type: 'vector', paths: [V.polylineBez([[0, 0], [w, 0], [w, h], [0, h]], true)] }, locked: false, hidden: false, group: null });
    const s = V.useStore.getState();
    s.newDocument();
    s.applyImport({
      objects: [sq('a', '#ff0000', 60, 60, 30, 30), sq('b', '#0000ff', 100, 70, 30, 20)],
      new_layers: [V.makeLayer('#ff0000', 'Corte', { speed: 777, power: 90 }), V.makeLayer('#0000ff', 'Grabado', { mode: 'fill', speed: 2500, power: 30, interval: 0.2 })],
      warnings: [],
    });
    s.fitContent();
    s.select(['a', 'b']);
  })()`);
  await wait(500);
  check((await evaluate('window.__v.useStore.getState().selectedIds.length')) === 2, 'quedaron seleccionados los dos objetos');
  check((await evaluate('window.__v.useStore.getState().doc.layers.length')) === 2, 'hay dos capas distintas');
  await shot('01-seleccion.png');

  console.log('2. «Guardar como componente…» desde el menú contextual del lienzo');
  await evaluate(`(() => {
    const V = window.__v;
    const s = V.useStore.getState();
    const c = document.querySelector('#canvas-main');
    const rc = c.getBoundingClientRect();
    const [sx, sy] = V.worldToScreen({ zoom: s.zoom, panX: s.panX, panY: s.panY }, [75, 75]);
    c.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: rc.left + sx, clientY: rc.top + sy, button: 2 }));
  })()`);
  await wait(400);
  const saveLabel = await T('art.component.cmd.save');
  check(await evaluate(`[...document.querySelectorAll('.ctxmenu [role=menuitem]')].some((b) => b.textContent.includes(${JSON.stringify(saveLabel)}))`), 'el menú contextual ofrece «Guardar como componente»', saveLabel);
  await shot('02-menu-contextual.png');
  check(await clickText(saveLabel, 'document'), 'se pulsa «Guardar como componente»');
  await wait(500);
  const dlgTitle = await T('art.component.dialog.title');
  check(await evaluate(`document.querySelector('.modal__title')?.textContent === ${JSON.stringify(dlgTitle)}`), 'se abre el diálogo para nombrarlo', dlgTitle);
  const NAME = lang === 'es' ? 'Soporte con tapa' : 'Stand with lid';
  await evaluate(`(() => {
    const i = document.querySelector('.modal input');
    const d = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
    d.set.call(i, ${JSON.stringify(NAME)});
    i.dispatchEvent(new Event('input', { bubbles: true }));
  })()`);
  await wait(200);
  check(((await layoutProblems('.modal')) ?? []).length === 0, 'el diálogo no tiene botones sin texto ni desbordes', JSON.stringify(await layoutProblems('.modal')));
  await shot('03-dialogo-nombre.png');
  check(await clickText(await T('art.component.dialog.save'), `document.querySelector('.modal')`), 'se pulsa «Guardar»');
  await until(`!document.querySelector('.modal') && window.__v.useArtPanel.getState().components.length === 1`, 'el componente quedó guardado');
  check((await evaluate('window.__v.useArtPanel.getState().components[0]?.name')) === NAME, 'se guardó con el nombre elegido', NAME);
  const data = await evaluate('JSON.stringify(window.__v.useArtPanel.getState().components[0].data)');
  const comp = JSON.parse(data);
  check(comp.objects.length === 2 && comp.layers.length === 2, 'el componente trae dos objetos y dos capas con sus ajustes');
  check(comp.layers.some((l) => l.speed === 777) && comp.layers.some((l) => l.mode === 'fill'), 'las capas conservan sus ajustes');
  await wait(300);
  await shot('04-guardado.png');

  console.log('3. Biblioteca de arte › Mis componentes: miniatura, nombre y tamaño');
  await evaluate(`window.__v.commands.run('tools.artLibrary')`);
  await wait(500);
  check(await evaluate(`!!document.querySelector('.art-panel')`), 'se abre la Biblioteca de arte');
  const catLabel = await T('art.cat.componentes');
  check(await clickText(catLabel, `document.querySelector('.art-panel')`), 'categoría «Mis componentes»', catLabel);
  await wait(500);
  const row = await evaluate(`(() => {
    const li = document.querySelector('.art-comp');
    if (!li) return null;
    const img = li.querySelector('img.art-comp__img');
    return { name: li.querySelector('.art-comp__name')?.textContent ?? '', meta: li.querySelector('.art-comp__meta')?.textContent ?? '', thumb: !!img, w: img ? img.getBoundingClientRect().width : 0, loaded: img ? img.complete && img.naturalWidth > 0 : false };
  })()`);
  check(!!row && row.name === NAME, 'sale el nombre', row?.name);
  check(!!row && row.meta.includes('70 × 30 mm'), 'sale el tamaño (70 × 30 mm)', row?.meta);
  check(!!row && row.thumb && row.loaded && row.w > 8, 'sale la miniatura (imagen cargada)', JSON.stringify({ w: row?.w, loaded: row?.loaded }));
  const lp = await layoutProblems('.art-panel');
  check(lp.length === 0, 'el panel no tiene botones sin texto ni desbordes', JSON.stringify(lp));
  await shot('05-biblioteca-mis-componentes.png');

  console.log('4. insertarlo en una pestaña nueva, en su posición original');
  await evaluate(`window.__v.useStore.getState().newTab()`);
  await wait(500);
  check((await evaluate('window.__v.useStore.getState().doc.objects.length')) === 0, 'la pestaña nueva está vacía');
  const tabsBefore = await evaluate('window.__v.useStore.getState().tabs.length');
  check(tabsBefore >= 2, 'hay más de una pestaña', String(tabsBefore));
  check(await mouseClick('.art-comp__main'), 'clic de ratón en el componente de la biblioteca');
  await until(`window.__v.useStore.getState().doc.objects.length === 2`, 'aparecen los dos objetos');
  await wait(500);
  const ins = await evaluate(`(() => {
    const V = window.__v; const d = V.useStore.getState().doc;
    const b = V.unionBounds(d.objects.map(V.objectBoundsWithCopies));
    return { b, layers: d.layers.map((l) => [l.color, l.speed, l.mode]), objLayers: d.objects.map((o) => o.layer), sel: V.useStore.getState().selectedIds.length };
  })()`);
  check(ins.b && Math.abs(ins.b[0][0] - 60) < 0.01 && Math.abs(ins.b[0][1] - 60) < 0.01 && Math.abs(ins.b[1][0] - 130) < 0.01 && Math.abs(ins.b[1][1] - 90) < 0.01, 'quedó en su posición original (60,60)-(130,90)', JSON.stringify(ins.b));
  check(new Set(ins.objLayers).size === 2, 'los objetos están en dos capas distintas', ins.objLayers.join(','));
  check(ins.layers.length === 2 && ins.layers.some((l) => l[1] === 777) && ins.layers.some((l) => l[2] === 'fill'), 'las capas llegaron con sus ajustes', JSON.stringify(ins.layers));
  check(ins.sel === 2, 'lo insertado queda seleccionado');
  const origMsg = await T('art.component.insertedOriginal', { name: NAME });
  check(await evaluate(`document.body.textContent.includes(${JSON.stringify(origMsg)})`), 'el aviso dice «en su posición original»', origMsg);
  await shot('06-insertado-pestana-nueva.png');

  console.log('5. Deshacer lo quita en un solo paso');
  await evaluate(`window.__v.commands.run('edit.undo')`);
  await wait(400);
  check((await evaluate('window.__v.useStore.getState().doc.objects.length')) === 0, 'un solo Deshacer quita los dos objetos');
  await evaluate(`window.__v.commands.run('edit.redo')`);
  await wait(300);
  check((await evaluate('window.__v.useStore.getState().doc.objects.length')) === 2, 'Rehacer los devuelve');
  await shot('07-deshacer-rehacer.png');

  console.log('6. renombrar');
  const NEW = lang === 'es' ? 'Soporte final' : 'Final stand';
  check(await clickText(await T('art.component.rename.button', { name: NAME }), `document.querySelector('.art-panel')`), 'botón de lápiz');
  await wait(300);
  await shot('08-renombrando.png');
  await evaluate(`(() => {
    const i = document.querySelector('.art-comp__rename');
    const d = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
    d.set.call(i, ${JSON.stringify(NEW)});
    i.dispatchEvent(new Event('input', { bubbles: true }));
    i.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  })()`);
  await until(`window.__v.useArtPanel.getState().components.some((c) => c.name === ${JSON.stringify(NEW)})`, 'el componente cambió de nombre');
  await wait(300);
  check((await evaluate(`document.querySelector('.art-comp__name')?.textContent`)) === NEW, 'la lista muestra el nombre nuevo', NEW);

  console.log('7. borrar: pide confirmación; «Conservar» no borra y «Borrar» sí');
  check(await clickText(await T('art.component.delete.button', { name: NEW }), `document.querySelector('.art-panel')`), 'botón de papelera');
  await wait(400);
  const ask = await evaluate(`window.__v.usePrompts.getState().current?.title ?? ''`);
  check(ask === (await T('art.component.delete.title')), 'aparece la confirmación de borrado', ask);
  const pl = await layoutProblems('.modal');
  check(pl.length === 0, 'la confirmación no tiene botones sin texto ni desbordes', JSON.stringify(pl));
  await shot('09-confirmar-borrado.png');
  check(await clickText(await T('art.component.delete.cancel'), `document.querySelector('.modal')`), 'se pulsa «Conservar»');
  await wait(400);
  check((await evaluate('window.__v.useArtPanel.getState().components.length')) === 1, 'Conservar no borró nada');
  await clickText(await T('art.component.delete.button', { name: NEW }), `document.querySelector('.art-panel')`);
  await wait(400);
  check(await clickText(await T('art.component.delete.confirm'), `document.querySelector('.modal')`), 'se pulsa «Borrar»');
  await until(`window.__v.useArtPanel.getState().components.length === 0`, 'el componente se borró');
  await wait(300);
  check(await evaluate(`!!document.querySelector('.art-panel .art-empty')`), 'la lista vacía muestra su texto de ayuda');
  check((await evaluate('window.__v.useStore.getState().doc.objects.length')) === 2, 'borrar de la biblioteca no toca el proyecto donde ya se insertó');
  await shot('10-biblioteca-vacia.png');
}

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
