// Capturas de la interfaz en modo mock, sin dependencias: usa Chrome/Chromium ya instalado y su
// protocolo de depuración (WebSocket nativo de Node 22).
//
//   npm run dev            # en otra terminal (puerto 1420); este script NO lo arranca
//   npm run screenshots    # guarda PNG en docs/screenshots/
//
// Chrome corre con una cámara falsa (--use-fake-device-for-media-stream) para el asistente de
// calibración. Las capturas de la fase 3 preparan el estado con los módulos de Vite (`app()`),
// que son los mismos que usa la página: solo funciona contra el servidor de desarrollo.
//
// Variables: CHROME (ruta del binario), APP_URL (por defecto http://localhost:1420/).
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, '..', 'docs', 'screenshots');
const url = process.env.APP_URL ?? 'http://localhost:1420/';
const port = 9333;
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

const bin = process.env.CHROME ?? ['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', '/usr/bin/google-chrome', '/usr/bin/chromium'].find(existsSync);
if (!bin) throw new Error('No se encontró Chrome/Chromium. Define CHROME=/ruta/al/binario');

mkdirSync(out, { recursive: true });
const proc = spawn(bin, [
  '--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), 'ls-shot-'))}`,
  '--disable-gpu', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--hide-scrollbars', '--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream', '--window-size=1440,900', '--force-device-scale-factor=1', 'about:blank',
], { stdio: 'ignore' });

let target;
for (let i = 0; i < 50 && !target; i++) {
  await wait(200);
  try {
    const list = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
    target = list.find((t) => t.type === 'page');
  } catch { /* aún arrancando */ }
}
if (!target) throw new Error('Chrome no respondió');

const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map();
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
};
const send = (method, params = {}) => new Promise((resolve, reject) => {
  const mid = ++id;
  pending.set(mid, (msg) => (msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result)));
  ws.send(JSON.stringify({ id: mid, method, params }));
});
// Una excepción en la página no rechaza la promesa de CDP: se convierte en error para no capturar a ciegas.
const evaluate = async (expression) => {
  const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
  return r;
};
const click = (selector) => evaluate(`(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) throw new Error('sin ${selector}'); el.click(); })()`);
const clickText = (selector, text) => evaluate(`(() => { const el = [...document.querySelectorAll(${JSON.stringify(selector)})].find(e => e.textContent.trim().startsWith(${JSON.stringify(text)})); if (!el) throw new Error('sin ${text}'); el.click(); })()`);
/** Ejecuta código en la página con `useStore` y `commands` (los módulos de la propia app) a mano. */
const app = (code) => evaluate(`(async () => {
  const { useStore } = await import('/src/store/index.ts');
  const { commands } = await import('/src/commands/index.ts');
  const st = () => useStore.getState();
  ${code}
})()`);
const type = (text) => send('Input.insertText', { text });
const key = async (k, code, modifiers = 0) => {
  await send('Input.dispatchKeyEvent', { type: 'keyDown', key: k, code, modifiers, text: modifiers || k.length !== 1 ? undefined : k });
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key: k, code, modifiers });
};
const shot = async (name) => {
  const { data } = await send('Page.captureScreenshot', { format: 'png' });
  writeFileSync(join(out, name), Buffer.from(data, 'base64'));
  console.log('guardada', name);
};

await send('Page.enable');
await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url });
await wait(800);
await evaluate("localStorage.setItem('tlaku.theme', 'dark')"); // siempre se parte del tema oscuro
await send('Page.navigate', { url });
await wait(2500);
await evaluate(`document.querySelectorAll('.toast button').forEach(b => b.click())`);
await wait(300);

// Atajos: Cmd en macOS y Ctrl en el resto (el registro usa "Mod").
const MOD = (await evaluate(`/Mac/.test(navigator.platform)`)).result.value ? 4 : 2;
const SHIFT = 8;
const closeDialog = async () => {
  await key('Escape', 'Escape');
  await wait(300);
};

// 1. Vista principal, tema oscuro, con el diseño de ejemplo seleccionado
await click('.canvas-area');
await key('a', 'KeyA', MOD);
await wait(500);
await shot('01-principal-oscuro.png');

// 2. Menú abierto
await clickText('.menubar__trigger', 'Archivo');
await wait(300);
await shot('02-menu-archivo.png');
await key('Escape', 'Escape');

// 3. Editor de ajustes de capa (doble clic en la fila de Corte)
await evaluate(`(() => {
  const row = [...document.querySelectorAll('.lyr-main')].find((e) => e.textContent.includes('Corte'));
  if (!row) throw new Error('sin la capa Corte');
  row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
})()`);
await wait(500);
await shot('03-editor-de-capa.png');
await closeDialog();

// 4. Vista previa del trabajo con el panel de resultados (atajo P)
await click('.canvas-area');
await key('p', 'KeyP');
await wait(1500);
await shot('04-vista-previa.png');
await key('p', 'KeyP');
await wait(300);

// 5. Máquina conectada con el diálogo de iniciar
await click('#tab-machine');
await wait(300);
await clickText('.btn', 'Conectar');
await wait(1500);
await clickText('.btn', 'Iniciar');
await wait(1200);
await shot('05-maquina-iniciar.png');

// 6. Trabajo en curso
await clickText('.mc-modal .btn', 'Iniciar');
await wait(2500);
await shot('06-maquina-trabajo.png');
await clickText('.btn', 'Detener');
await wait(400);

// 7. Consola
await click('#tab-console');
await wait(300);
await shot('07-consola.png');

// 8. Historial: unos pasos de edición para que haya qué ver
await click('.canvas-area');
await key('ArrowRight', 'ArrowRight', SHIFT);
await key('ArrowUp', 'ArrowUp', SHIFT);
await key('d', 'KeyD', MOD);
await key('ArrowLeft', 'ArrowLeft', SHIFT);
await key('z', 'KeyZ', MOD);
await wait(300);
await click('#tab-history');
await wait(500);
await shot('08-historial.png');

// 9. Diálogo de atajos (F1)
await key('F1', 'F1');
await wait(500);
await shot('09-atajos.png');
await closeDialog();

// 10. Tema claro
await click('#tab-layers');
await click('button[aria-label="Cambiar a tema claro"]');
await wait(500);
await shot('10-principal-claro.png');


// ---- Fase 3 (siempre desde un documento limpio y el tema oscuro) ----------------------------------------
const fresh = async () => {
  await evaluate("localStorage.setItem('tlaku.theme', 'dark')");
  await send('Page.navigate', { url });
  await wait(2500);
  await evaluate(`document.querySelectorAll('.toast button').forEach(b => b.click())`);
  await wait(300);
};

// 11. Editor de nodos: la placa de esquinas curvas con nodos y manejadores (tres nodos elegidos)
await fresh();
await app(`
  st().select(['obj-placa']);
  st().setTool('nodes');
  st().fitContent();
  st().setNodeSel({ 'obj-placa': [1, 2, 5] });
`);
await wait(500);
await shot('11-editor-de-nodos.png');

// 12. Panel de texto: se crea un texto con la herramienta y se escribe en el panel
await fresh();
await app(`
  const tt = await import('/src/features/canvas/textTool.ts');
  tt.createTextAt(st(), [215, 262]);
  st().setTool('select');
`);
await wait(1200);
await evaluate(`(() => { const a = document.querySelector('.text-panel__area'); a.focus(); a.select(); })()`);
await type('Tlaku láser');
await wait(1200);
await shot('12-panel-de-texto.png');

// 13. Diferencia: la placa (A) menos el engranaje (B); la forma principal se resalta en el lienzo
await fresh();
await app(`
  st().select(['obj-placa', 'obj-engranaje']);
  st().fitContent();
  await commands.run('arrange.difference');
`);
await wait(700);
await shot('13-diferencia.png');
await closeDialog();

// 14. Arreglo circular de la estrella de marcas
await fresh();
await app(`
  st().select(['obj-estrella']);
  await commands.run('arrange.circular');
`);
await wait(700);
await shot('14-arreglo-circular.png');
await closeDialog();

// 15. Vectorizado de la foto con su vista previa
await fresh();
await app(`
  st().select(['obj-foto']);
  await commands.run('tools.traceImage');
`);
await wait(2500);
await shot('15-vectorizar.png');
await closeDialog();

// 16. Cámara (falsa de Chrome) con el asistente de calibración: vistas capturadas y resultado
await fresh();
await click('#tab-camera');
await wait(500);
await app(`await commands.run('camera.calibrate');`);
await wait(500);
await clickText('.cam-cal__empty .btn', 'Encender');
await wait(2500);
for (let i = 0; i < 5; i++) {
  await clickText('.cam-cal__bar .btn', 'Capturar');
  await wait(700);
}
await clickText('.cam-cal__side .btn', 'Calcular');
await wait(1200);
await shot('16-camara-calibracion.png');
await closeDialog();
await wait(300);
await shot('17-panel-camara.png');

// 18. Pestañas del panel: con poco ancho pasan a solo ícono (nombre en tooltip y aria-label)
await app(`st().setDockWidth(520); st().setDockTab('layers');`);
await wait(500);
await shot('18-pestanas-ancho.png');

// 19. Panel de materiales con la biblioteca de ejemplo
await fresh();
await click('#tab-materials');
await wait(800);
await shot('19-materiales.png');

// 20. Prueba de materiales: cuadrícula potencia x velocidad
await app(`await commands.run('tools.materialTest');`);
await wait(1500);
await shot('20-prueba-materiales.png');
await closeDialog();

// 21. Editor de la capa de corte con puentes activados
await fresh();
await app(`
  const corte = st().doc.layers.find((l) => l.name.includes('Corte'));
  st().updateLayer(corte.color, { tabs: { ...corte.tabs, enabled: true, count: 4, width: 1 } });
`);
await evaluate(`(() => {
  const row = [...document.querySelectorAll('.lyr-main')].find((e) => e.textContent.includes('Corte'));
  row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
})()`);
await wait(600);
await evaluate(`(() => {
  const el = [...document.querySelectorAll('h2, h3, h4, legend, .section-title, [class*=title], [class*=head]')].find((e) => e.textContent.trim().startsWith('Puentes'));
  if (!el) throw new Error('sin la sección Puentes');
  el.scrollIntoView({ block: 'start' });
})()`);
await wait(300);
await shot('21-capa-puentes.png');
await closeDialog();

// 22. Perfil de máquina: eje Z, rotativo y corrección de barrido
await app(`await commands.run('machine.devices');`);
await wait(800);
await evaluate(`(() => {
  const el = [...document.querySelectorAll('label, span, legend, div')].find((e) => e.childElementCount === 0 && e.textContent.trim() === 'Rotativo');
  if (!el) throw new Error('sin la sección Rotativo');
  const label = [...document.querySelectorAll('label')].find((e) => e.textContent.includes('Usar un rotativo'));
  const box = label?.querySelector('input[type=checkbox]');
  if (box && !box.checked) box.click();
  el.scrollIntoView({ block: 'start' });
})()`);
await wait(400);
await shot('22-maquina-rotativo.png');
await closeDialog();

// 23. Quitar duplicados con su tolerancia
await fresh();
await app(`
  st().select(['obj-placa', 'obj-engranaje']);
  await commands.run('arrange.dedupe');
`);
await wait(700);
await shot('23-quitar-duplicados.png');
await closeDialog();

// 24. Vista previa con puentes en la capa de corte
await fresh();
await app(`
  const corte = st().doc.layers.find((l) => l.name.includes('Corte'));
  st().updateLayer(corte.color, { tabs: { ...corte.tabs, enabled: true, count: 4, width: 2 } });
`);
await click('.canvas-area');
await key('p', 'KeyP');
await wait(1800);
await shot('24-vista-previa-puentes.png');

await send('Browser.close').catch(() => {});
ws.close();
proc.kill();
