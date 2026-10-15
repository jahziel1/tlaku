// Verificación del navegador de ajustes de la comunidad en el ESCRITORIO con Chrome headless y la
// máquina simulada (mock), con ratón y pantalla de verdad:
//
//   node scripts/community-verify.mjs                      # solo verifica
//   node scripts/community-verify.mjs --shots DIR          # además guarda capturas
//
// Comprueba que:
//   (a) arrancar y abrir el panel Materiales NO descarga nada; el catálogo se pide solo al abrir «Explorar la
//       comunidad…» (primero la copia guardada, sin red, y luego el sitio);
//   (b) por omisión se ve «Mi máquina» (modelo y potencia) con el aviso de «solo orientativo»; con «Todas las
//       máquinas» el ajuste de otra potencia o de otro láser lo advierte en su detalle;
//   (c) filtros por material, grosor, operación, verificados y texto;
//   (d) «Usar» copia a Mis materiales con su origen y no cambia la capa; «Aplicar a la capa» pregunta antes, se
//       puede cancelar y se puede deshacer; ninguna de las dos envía nada a la máquina;
//   (e) «Compartir» abre el diálogo y NO abre el navegador hasta pulsar «Abrir el formulario»; la dirección es
//       la del formulario del repositorio, prellenada y sin datos personales;
//   (f) sin conexión se ve la última copia con su fecha; con el catálogo vacío o inválido, un mensaje claro;
//   (g) la interfaz no hace ningún fetch por su cuenta (la red es de Rust).
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
let base = process.env.APP_URL;
if (!base) {
  vite = spawn('npx', ['vite', '--config', 'scripts/vite.stress.config.mjs', '--port', '1438', '--strictPort'], { cwd: appDir, stdio: 'ignore' });
  base = 'http://localhost:1438/';
  let up = false;
  for (let i = 0; i < 100 && !up; i++) { await wait(300); try { up = (await fetch(base)).ok; } catch { /* arrancando */ } }
  if (!up) { vite.kill(); throw new Error('Vite no arrancó'); }
}
const PORT = 9354;
const userData = mkdtempSync(join(tmpdir(), 'ls-community-'));
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

const clickText = (text, root = 'document') => evaluate(`(() => {
  const btns = [...${root}.querySelectorAll('button')].filter((b) => b.offsetParent !== null && b.textContent.includes(${JSON.stringify(text)}));
  if (!btns.length) return false;
  btns[0].click();
  return true;
})()`);
const setSelect = (label, value) => evaluate(`(() => {
  const el = [...document.querySelectorAll('.com__filters label')].find((l) => l.textContent.trim() === ${JSON.stringify(label)});
  const sel = el && document.getElementById(el.htmlFor);
  if (!sel) return false;
  const d = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value');
  d.set.call(sel, ${JSON.stringify(value)});
  sel.dispatchEvent(new Event('change', { bubbles: true }));
  return true;
})()`);
const rows = () => evaluate(`[...document.querySelectorAll('.com-row')].map((r) => r.querySelector('.com-row__title').textContent)`);

// La app con la API simulada, anotando cada llamada de red, de máquina y del catálogo.
const SETUP = (name) => `(async () => {
  const { useStore, bindMachineEvents } = await import('/src/store/index.ts');
  const { setApi } = await import('/src/api/index.ts');
  const { createMockApi } = await import('/src/api/mock/index.ts');
  const { commands } = await import('/src/commands/index.ts');
  const { useMaterials } = await import('/src/features/materials/state.ts');
  const { useCommunity } = await import('/src/features/materials/communityStore.ts');
  const { usePrompts } = await import('/src/features/history/prompt.ts');
  const { useHelpUi } = await import('/src/features/help/helpStore.ts');
  const { openEnv } = await import('/src/features/help/openExternal.ts');
  const { setLanguage } = await import('/src/i18n/index.ts');
  const V = (window.__c = { useStore, commands, useMaterials, useCommunity, usePrompts, useHelpUi, setLanguage, calls: [], opened: [], fetches: [] });
  const realFetch = window.fetch.bind(window);
  window.fetch = (input, init) => { V.fetches.push(String(input?.url ?? input)); return realFetch(input, init); };
  openEnv.open = async (u) => { V.opened.push(u); };
  const api = createMockApi({ latencyMs: 30, statusIntervalMs: 200 });
  for (const key of Object.keys(api)) {
    const fn = api[key];
    if (typeof fn === 'function') api[key] = (...a) => { V.calls.push(key); return fn(...a); };
  }
  setApi(api);
  await bindMachineEvents(api, useStore);
  const s = () => useStore.getState();
  s().setPref('uiMode', 'full');
  document.querySelectorAll('.toast button').forEach((b) => b.click());
  await s().loadDevices();
  // El perfil activo se llama como el asistente de máquinas nombra al modelo del catálogo.
  const dev = s().devices.find((d) => d.id === s().activeDeviceId) ?? s().devices[0];
  V.deviceName = dev.name;
  return { device: dev.name, kind: dev.kind, ${JSON.stringify(name)}: true };
})()`;

async function load(query = '') {
  await send('Page.navigate', { url: `${base}${query}` });
  await wait(2500);
  const info = await evaluate(SETUP('ok'));
  return info;
}

try {
  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });

  console.log('1. arrancar y abrir el panel Materiales no descarga nada');
  const info = await load();
  console.log('   máquina activa:', JSON.stringify(info));
  await evaluate(`(async () => {
    const V = window.__c;
    const s = V.useStore.getState();
    // La máquina activa como el asistente la nombra al elegir un modelo del catálogo: marca y modelo.
    const dev = s.devices.find((d) => d.id === s.activeDeviceId) ?? s.devices[0];
    await s.saveDevice({ ...dev, name: 'Creality Falcon2 22 W' });
  })()`);
  await evaluate(`window.__c.commands.run('panel.materials')`);
  await wait(800);
  check(await evaluate(`window.__c.calls.filter((c) => c.startsWith('communityCatalog')).length === 0`), 'ninguna llamada al catálogo al arrancar ni al abrir el panel');
  check(await evaluate(`[...document.querySelectorAll('button')].some((b) => b.textContent.includes('Explorar la comunidad'))`), 'el panel tiene el botón «Explorar la comunidad…»');
  await shot('01-panel-materiales.png');

  console.log('2. «Explorar la comunidad…»: primero la copia (sin red) y luego el sitio');
  check(await clickText('Explorar la comunidad'), 'clic en «Explorar la comunidad…»');
  await until(`!!document.querySelector('.com') && document.querySelectorAll('.com-row').length > 0`, 'aparece la lista');
  const order = await evaluate(`window.__c.calls.filter((c) => c.startsWith('communityCatalog'))`);
  check(JSON.stringify(order) === JSON.stringify(['communityCatalogCached', 'communityCatalogRefresh']), 'se pidió primero la copia y luego el sitio', order.join(' → '));
  check(await evaluate(`window.__c.fetches.every((u) => !/github|http/.test(u) || u.startsWith(location.origin))`), 'la interfaz no hizo ningún fetch fuera de la app');
  const r1 = await rows();
  console.log('   filas:', JSON.stringify(r1));
  check(r1.length === 3, 'por omisión: mi modelo y mi clase (3 ajustes)', String(r1.length));
  check(await evaluate(`document.querySelector('.com__notes').textContent.includes('solo orientativos')`), 'se ve el aviso de «solo orientativos»');
  check(await evaluate(`document.querySelector('.com__detail').textContent.includes('Verificado por 2 personas')`), 'el detalle muestra «Verificado por 2 personas»');
  check(await evaluate(`document.querySelector('.com__detail').textContent.includes('nadie la comprueba')`), 'dice que «misma máquina» es declarado');
  await shot('02-comunidad-mi-maquina.png');

  console.log('3. todas las máquinas: el de otra potencia y el de otro láser advierten');
  check(await setSelect('Máquina', 'all'), 'filtro «Todas las máquinas»');
  await wait(200);
  check((await rows()).length === 6, 'se ven los 6 ajustes');
  check(await evaluate(`(() => { const r = [...document.querySelectorAll('.com-row')].find((x) => x.textContent.includes('MDF')); r.click(); return true; })()`), 'clic en el de MDF (diodo de 10 W)');
  await wait(200);
  check(await evaluate(`document.querySelector('.com__detail').textContent.includes('Solo orientativo')`), 'el detalle de otra potencia dice «Solo orientativo»');
  await shot('03-comunidad-otra-potencia.png');
  await evaluate(`(() => { const r = [...document.querySelectorAll('.com-row')].find((x) => x.textContent.includes('Imagen')); r.click(); })()`);
  await wait(200);
  check(await evaluate(`document.querySelector('.com__detail').textContent.includes('Otro tipo de láser')`), 'el de fibra dice «Otro tipo de láser»');

  console.log('4. filtros');
  check(await setSelect('Material', 'basswood'), 'material: tilo');
  await wait(200);
  check((await rows()).length === 2, 'quedan 2 de tilo');
  check(await setSelect('Operación', 'fill'), 'operación: relleno');
  await wait(200);
  check((await rows()).length === 1, 'queda 1 (relleno)');
  await shot('04-comunidad-filtros.png');
  await setSelect('Operación', '');
  await setSelect('Material', '');
  check(await setSelect('Grosor', 'none'), 'grosor: sin grosor');
  await wait(200);
  check((await rows()).length === 2, 'sin grosor: 2 ajustes');
  await setSelect('Grosor', '');
  await evaluate(`(() => { const c = document.querySelector('.com__filters input[type=checkbox]'); c.click(); })()`);
  await wait(200);
  check((await rows()).length === 2, 'solo verificados: 2 ajustes');
  await evaluate(`(() => { const c = document.querySelector('.com__filters input[type=checkbox]'); c.click(); })()`);
  await wait(100);
  check(await setSelect('Máquina', 'mine'), 'vuelve a «Mi máquina»');
  await wait(200);

  console.log('5. «Usar»: copia a Mis materiales con su origen y no cambia la capa');
  const layerBefore = await evaluate(`JSON.stringify(window.__c.useStore.getState().doc.layers[0])`);
  const libBefore = await evaluate(`window.__c.useMaterials.getState().library.presets.length`);
  check(await clickText('Usar (copiar'), 'clic en «Usar (copiar a mi biblioteca)»');
  await wait(500);
  const lib = await evaluate(`window.__c.useMaterials.getState().library.presets`);
  check(lib.length === libBefore + 1, 'la biblioteca tiene un ajuste más');
  const added = lib[lib.length - 1];
  check(added.source === 'https://github.com/jahziel1/tlaku/issues/101' && added.confidence === 'forum', 'trae su origen (issue) y la confianza «foro»', added.source);
  check(added.notes.includes('@ejemplo-uno') && added.notes.includes('Verificado por 2 personas'), 'las notas dicen quién lo aportó y cuántos lo verificaron', added.notes);
  check((await evaluate(`JSON.stringify(window.__c.useStore.getState().doc.layers[0])`)) === layerBefore, 'la capa no cambió');
  await clickText('Usar (copiar');
  await wait(400);
  check((await evaluate(`window.__c.useMaterials.getState().library.presets.length`)) === libBefore + 1, 'usarlo otra vez no lo duplica');
  await shot('05-comunidad-usar.png');

  console.log('6. «Aplicar a la capa seleccionada…»: pregunta, se puede cancelar y se puede deshacer');
  const startsBefore = await evaluate(`window.__c.calls.filter((c) => c === 'startJob' || c === 'sendJob').length`);
  await evaluate(`(() => { const s = window.__c.useStore.getState(); s.setActiveLayer(s.doc.layers[0].color); })()`);
  await wait(200);
  check(await clickText('Aplicar a la capa seleccionada'), 'clic en «Aplicar a la capa seleccionada…»');
  await until(`!!window.__c.usePrompts.getState().current`, 'aparece la confirmación');
  const prompt = await evaluate(`window.__c.usePrompts.getState().current`);
  check(prompt.message.includes('500 mm/min') && prompt.message.includes('No se envía nada a la máquina'), 'la pregunta muestra los valores y dice que no se envía nada');
  await wait(300);
  await shot('06-comunidad-aplicar-confirmacion.png');
  await evaluate(`window.__c.usePrompts.getState().answer('cancel')`);
  await wait(300);
  check((await evaluate(`JSON.stringify(window.__c.useStore.getState().doc.layers[0])`)) === layerBefore, 'cancelar no cambió la capa');
  await clickText('Aplicar a la capa seleccionada');
  await until(`!!window.__c.usePrompts.getState().current`, 'otra vez la confirmación');
  await evaluate(`window.__c.usePrompts.getState().answer('apply')`);
  await wait(400);
  const layerAfter = await evaluate(`window.__c.useStore.getState().doc.layers[0]`);
  check(layerAfter.speed === 500 && layerAfter.power === 100 && layerAfter.passes === 2, 'confirmar copió velocidad, potencia y pasadas a la capa', `${layerAfter.speed} ${layerAfter.power} ${layerAfter.passes}`);
  await evaluate(`window.__c.useStore.getState().undo()`);
  await wait(200);
  check((await evaluate(`JSON.stringify(window.__c.useStore.getState().doc.layers[0])`)) === layerBefore, 'deshacer devuelve la capa');
  check((await evaluate(`window.__c.calls.filter((c) => c === 'startJob' || c === 'sendJob').length`)) === startsBefore, 'nada se envió a la máquina');

  console.log('7. compartir: abre el diálogo y NO el navegador hasta pulsar «Abrir el formulario»');
  check(await clickText('Compartir un ajuste mío'), 'clic en «Compartir un ajuste mío…»');
  await until(`!!document.querySelector('#help-share-material-title')`, 'se abre el diálogo de compartir');
  await wait(500);
  check((await evaluate(`window.__c.opened.length`)) === 0, 'el navegador no se abrió solo');
  check(await evaluate(`document.body.textContent.includes('Ver el aporte como JSON')`), 'se ofrece ver el aporte como JSON');
  await evaluate(`document.querySelector('details summary').click()`);
  await wait(200);
  await shot('07-compartir-ajuste.png');
  const json = await evaluate(`document.querySelector('details code').textContent`);
  console.log('   JSON:', json.replace(/\s+/g, ' '));
  check(!/mat-|\/Users|notes/.test(json), 'el JSON no lleva identificadores, rutas ni notas');
  check(await clickText('Abrir el formulario en GitHub'), 'clic en «Abrir el formulario en GitHub»');
  await wait(300);
  const opened = await evaluate(`window.__c.opened`);
  check(opened.length === 1 && opened[0].startsWith('https://github.com/jahziel1/tlaku/issues/new?template=material_settings.yml'), 'se abrió solo la dirección del formulario del repositorio', opened[0]);
  check(/speed=/.test(opened[0] ?? '') && /laser_type=/.test(opened[0] ?? ''), 'la dirección trae los valores prellenados');
  await evaluate(`window.__c.useHelpUi.getState().close()`);

  console.log('8. inglés');
  await evaluate(`window.__c.setLanguage('en'); window.__c.useStore.getState().setPref('language', 'en')`);
  await evaluate(`(async () => { await window.__c.commands.run('panel.materials'); })()`);
  await wait(300);
  check(await clickText('Browse the community'), 'clic en «Browse the community…»');
  await until(`document.querySelectorAll('.com-row').length > 0`, 'lista en inglés');
  check(await evaluate(`document.querySelector('.com__notes').textContent.includes('only a rough guide')`), 'el aviso está en inglés');
  await shot('08-comunidad-ingles.png');
  await evaluate(`window.__c.setLanguage('es'); window.__c.useStore.getState().setPref('language', 'es')`);

  console.log('9. sin conexión: la última copia con su fecha');
  await load('?community=offline');
  await evaluate(`window.__c.useStore.getState().setPref('uiMode', 'full')`);
  await evaluate(`window.__c.commands.run('panel.materials')`);
  await wait(500);
  await clickText('Explorar la comunidad');
  await until(`!!document.querySelector('.com__status[data-tone=warn]')`, 'aparece el aviso de sin conexión');
  const status = await evaluate(`document.querySelector('.com__head').textContent`);
  check(status.includes('No se pudo actualizar; se muestra la copia guardada el'), 'dice que muestra la copia guardada', status.slice(0, 160));
  check((await evaluate(`document.querySelectorAll('.com-row, .com__empty').length`)) > 0, 'sigue mostrando los ajustes de la copia');
  await shot('09-comunidad-sin-conexion.png');

  console.log('10. catálogo vacío e inválido');
  await load('?community=empty');
  await evaluate(`window.__c.commands.run('panel.materials')`);
  await wait(500);
  await clickText('Explorar la comunidad');
  await until(`!!document.querySelector('.com__empty')`, 'estado vacío');
  check(await evaluate(`document.body.textContent.includes('Todavía no hay ajustes de la comunidad')`), 'el estado vacío es amable e invita a compartir');
  await shot('10-comunidad-vacio.png');
  await load('?community=invalid');
  await evaluate(`window.__c.commands.run('panel.materials')`);
  await wait(500);
  await clickText('Explorar la comunidad');
  await until(`!!document.querySelector('.com__status[data-tone=warn]')`, 'aviso de catálogo inválido');
  check(await evaluate(`document.querySelector('.com__head').textContent.includes('no es válido y se descartó')`), 'dice que el catálogo no es válido y se descartó');
  await shot('11-comunidad-invalido.png');
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
