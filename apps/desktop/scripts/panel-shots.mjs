// Capturas del panel Máquina (antes/después de las secciones plegables) en Chrome headless y con la
// app en modo mock. Dos estados (desconectada; conectada a la máquina virtual con un trabajo en curso),
// es/en, tema oscuro/claro y dos tamaños de ventana, más el panel entero (ventana alta).
//
//   node scripts/panel-shots.mjs --out DIR --prefix antes
//   node scripts/panel-shots.mjs --out DIR --prefix despues
//
// Arranca Vite con `scripts/vite.stress.config.mjs` (sin recarga en caliente) salvo que se pase APP_URL.
// Variables: CHROME, APP_URL.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const appDir = join(here, '..');
const args = process.argv.slice(2);
const opt = (name, def) => (args.includes(name) ? args[args.indexOf(name) + 1] : def);
const outDir = opt('--out', join(appDir, 'panel-shots'));
const prefix = opt('--prefix', 'despues');
mkdirSync(outDir, { recursive: true });
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
const PORT = 9350;
const proc = spawn(bin, ['--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), 'ls-panel-'))}`, '--disable-gpu', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--hide-scrollbars', '--window-size=1440,900', '--force-device-scale-factor=1', 'about:blank'], { stdio: 'ignore' });
const cleanup = () => { proc.kill(); vite?.kill(); };
let first;
for (let i = 0; i < 50 && !first; i++) { await wait(200); try { first = (await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()).find((t) => t.type === 'page'); } catch { /* arrancando */ } }
if (!first) { cleanup(); throw new Error('Chrome no respondió'); }
const ws = new WebSocket(first.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0; const pending = new Map();
ws.onmessage = (m) => { const msg = JSON.parse(m.data); if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); } };
const send = (method, params = {}) => new Promise((resolve, reject) => { const mid = ++id; pending.set(mid, (msg) => (msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result))); ws.send(JSON.stringify({ id: mid, method, params })); });
const evaluate = async (expression) => { const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true }); if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text); return r.result.value; };
const shot = async (name, clip) => {
  const r = await send('Page.captureScreenshot', { format: 'png', ...(clip ? { clip: { ...clip, scale: 1 }, captureBeyondViewport: true } : {}) });
  writeFileSync(join(outDir, name), Buffer.from(r.data, 'base64'));
  console.log('  captura', name);
};

const SETUP = (connected) => `(async () => {
  const { useStore, bindMachineEvents } = await import('/src/store/index.ts');
  const { setApi } = await import('/src/api/index.ts');
  const { createMockApi } = await import('/src/api/mock/index.ts');
  const s = () => useStore.getState();
  const api = createMockApi({ latencyMs: 0, statusIntervalMs: 300, minJobMs: 600000, jobSpeedup: 1 });
  setApi(api);
  await bindMachineEvents(api, useStore);
  s().setPref('uiMode', 'full');
  document.querySelectorAll('.toast button').forEach((b) => b.click());
  await s().loadDevices(); await s().refreshPorts();
  s().setDockTab('machine');
  ${connected ? `
  s().setSelectedPort('virtual:falcon2'); await s().connect();
  await new Promise((r) => setTimeout(r, 600));
  await s().computePreview();
  await s().startJob();
  await new Promise((r) => setTimeout(r, 7000));` : `await new Promise((r) => setTimeout(r, 600));`}
  window.__s = useStore;
})()`;

const SIZES = [[1440, 900], [1280, 800]];
const STATES = ['desconectada', 'conectada'];
const LANGS = ['es', 'en'];
const THEMES = ['dark', 'light'];

async function apply(lang, theme) {
  await evaluate(`(async () => { const s = window.__s.getState(); s.setPref('language', '${lang}'); s.setTheme('${theme}'); document.querySelectorAll('.toast button').forEach((b) => b.click()); await new Promise((r) => setTimeout(r, 400)); })()`);
}

try {
  for (const [w, h] of SIZES) {
    await send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false });
    for (const state of STATES) {
      await send('Page.navigate', { url });
      await wait(3000);
      await evaluate(SETUP(state === 'conectada'));
      for (const lang of LANGS) {
        for (const theme of THEMES) {
          await apply(lang, theme);
          await shot(`${prefix}-${state}-${lang}-${theme}-${w}x${h}.png`);
        }
      }
      if (w === 1440) {
        // Panel entero: ventana alta para que el panel no necesite desplazarse.
        await send('Emulation.setDeviceMetricsOverride', { width: w, height: 2600, deviceScaleFactor: 1, mobile: false });
        await wait(500);
        for (const lang of LANGS) {
          for (const theme of THEMES) {
            await apply(lang, theme);
            await shot(`${prefix}-panel-completo-${state}-${lang}-${theme}.png`);
          }
        }
        await send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false });
      }
    }
  }
  // Solo con las secciones plegables (`despues`): pedir una sección desde fuera la abre, la deja a la
  // vista dentro del panel (sin mover la ventana: base.css usa overflow: clip) y le da el foco.
  if (prefix === 'despues') {
    const r = await evaluate(`(async () => {
      const { revealPanelSection } = await import('/src/ui/PanelSection.tsx');
      window.__s.getState().setDockTab('layers');
      revealPanelSection('machine.settings');
      await new Promise((r) => setTimeout(r, 600));
      const sec = document.querySelector('[data-section="machine.settings"]');
      const box = sec.closest('.panel-scroll').getBoundingClientRect();
      const b = sec.getBoundingClientRect();
      return { open: sec.dataset.open, tab: window.__s.getState().dockTab, visible: b.top >= box.top - 1 && b.bottom <= box.bottom + 1, focus: document.activeElement === sec.querySelector('[data-psec-toggle]'), winScroll: [window.scrollX, window.scrollY, document.scrollingElement.scrollTop, document.body.scrollTop] };
    })()`);
    console.log('  revelar sección:', JSON.stringify(r));
    await shot(`${prefix}-revelar-ajustes-maquina.png`);
    const ok = r.open === 'true' && r.tab === 'machine' && r.visible && r.focus && r.winScroll.every((v) => v === 0);
    console.log(ok ? '  OK  la sección se abre, queda visible y no se mueve la ventana' : '  FALLA revelar sección');
    if (!ok) process.exitCode = 1;
  }
} finally {
  cleanup();
}
process.exit(process.exitCode ?? 0);
