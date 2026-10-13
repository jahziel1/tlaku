// Capturas de la interfaz en modo mock, sin dependencias: usa Chrome/Chromium ya instalado y su
// protocolo de depuración (WebSocket nativo de Node 22).
//
//   npm run dev            # en otra terminal (puerto 1420)
//   npm run screenshots    # guarda PNG en docs/screenshots/
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
  '--disable-gpu', '--hide-scrollbars', '--window-size=1440,900', '--force-device-scale-factor=1', 'about:blank',
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
const evaluate = (expression) => send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
const click = (selector) => evaluate(`(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) throw new Error('sin ${selector}'); el.click(); })()`);
const clickText = (selector, text) => evaluate(`(() => { const el = [...document.querySelectorAll(${JSON.stringify(selector)})].find(e => e.textContent.trim().startsWith(${JSON.stringify(text)})); if (!el) throw new Error('sin ${text}'); el.click(); })()`);
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
await evaluate("localStorage.setItem('lasersoft.theme', 'dark')"); // siempre se parte del tema oscuro
await send('Page.navigate', { url });
await wait(2500);
await evaluate(`document.querySelectorAll('.toast button').forEach(b => b.click())`);
await wait(300);

// 1. Vista principal, tema oscuro
await shot('01-principal-oscuro.png');

// 2. Menú abierto
await clickText('.menubar__trigger', 'Archivo');
await wait(300);
await shot('02-menu-archivo.png');
await key('Escape', 'Escape');

// 3. Vista previa del trabajo (atajo P)
await click('.canvas-area');
await key('p', 'KeyP');
await wait(1200);
await shot('03-vista-previa.png');
await key('p', 'KeyP');

// 4. Máquina conectada con un trabajo en curso
await clickText('.tab', 'Máquina');
await wait(300);
await clickText('.btn', 'Conectar');
await wait(1200);
await clickText('.btn', 'Iniciar');
await wait(2500);
await shot('04-maquina-trabajo.png');
await clickText('.btn', 'Detener');
await wait(400);

// 5. Consola
await clickText('.tab', 'Consola');
await wait(300);
await shot('05-consola.png');

// 6. Tema claro
await clickText('.tab', 'Capas');
await click('button[aria-label="Cambiar a tema claro"]');
await wait(500);
await shot('06-principal-claro.png');

await send('Browser.close').catch(() => {});
ws.close();
proc.kill();
