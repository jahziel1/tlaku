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
// Variables: CHROME (ruta del binario), APP_URL (por defecto http://localhost:1420/), SCREENSHOTS_OUT (carpeta de salida),
// REMOTE_DEMO_URL (enlace de una demo ya abierta) y SCREENSHOTS_SKIP_PHONE=1 (no levanta la demo remota ni toma las capturas 63).
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const out = process.env.SCREENSHOTS_OUT ?? join(here, '..', 'docs', 'screenshots'); // SCREENSHOTS_OUT: carpeta de salida alternativa
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
// Los PNG se guardan al final: la ayuda de la app los importa con `import.meta.glob`, así que escribir
// uno a mitad de la ejecución hace que Vite recargue módulos y reinicie el estado de la página.
const shots = new Map();
const shot = async (name) => {
  const { data } = await send('Page.captureScreenshot', { format: 'png' });
  shots.set(name, Buffer.from(data, 'base64'));
  console.log('capturada', name);
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

/** Espera a que el diseño de ejemplo (que carga el mock de forma asíncrona) esté en el documento. */
const sampleLoaded = () => evaluate(`new Promise((ok, fail) => {
  const t0 = Date.now();
  const tick = async () => {
    const { useStore } = await import('/src/store/index.ts');
    if (useStore.getState().doc.objects.length > 0) return ok();
    if (Date.now() - t0 > 20000) return fail(new Error('el diseño de ejemplo no cargó'));
    setTimeout(tick, 200);
  };
  tick();
})`);

// Atajos: Cmd en macOS y Ctrl en el resto (el registro usa "Mod").
const MOD = (await evaluate(`/Mac/.test(navigator.platform)`)).result.value ? 4 : 2;
const SHIFT = 8;
const closeDialog = async () => {
  await key('Escape', 'Escape');
  await wait(300);
};

// 58. Primer arranque: "¿Es tu primera vez con un láser?" (aparece mientras `uiMode` es "ask"). Se contesta "No" para que
// el resto de las capturas salgan con la interfaz completa.
await shot('58-primer-arranque.png');
await click('.simple-first__buttons .btn:not(.btn--accent)');
await wait(300);

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
  await sampleLoaded();
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
// El panel espera a que carguen las fuentes: se sondea hasta 15 s en vez de suponer un tiempo fijo.
await evaluate(`new Promise((ok, fail) => {
  const t0 = Date.now();
  const tick = () => document.querySelector('.text-panel__area') ? ok() : Date.now() - t0 > 15000 ? fail(new Error('sin el panel de texto')) : setTimeout(tick, 200);
  tick();
})`);
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

// 25. Máquina virtual Falcon2 Pro conectada, con el distintivo y la sección de fallas simuladas
await fresh();
await app(`
  st().setDockTab('machine');
  st().setSelectedPort('virtual:falcon2');
  await st().connect();
`);
await wait(1500);
await shot('25-maquina-virtual.png');

// 26. Asistente "Agregar máquina": paso de detección
await fresh();
await app(`await commands.run('machine.addDevice');`);
await wait(800);
await shot('26-agregar-maquina.png');
await closeDialog();

// 27. Tipo de controladora y dialecto en el gestor de dispositivos
await fresh();
await app(`await commands.run('machine.devices');`);
await wait(800);
await evaluate(`(() => {
  const el = [...document.querySelectorAll('label, span, legend, div')].find((e) => e.childElementCount === 0 && /tipo de controladora/i.test(e.textContent.trim()));
  if (el) el.scrollIntoView({ block: 'start' });
})()`);
await wait(400);
await shot('27-dialecto.png');
await closeDialog();

// 28. Menú contextual sobre la selección
await fresh();
await app(`st().select(['obj-engranaje']);`);
await evaluate(`(() => {
  const c = document.querySelector('.canvas-area');
  const r = c.getBoundingClientRect();
  c.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: r.left + r.width * 0.35, clientY: r.top + r.height * 0.6 }));
})()`);
await wait(600);
await shot('28-menu-contextual.png');
await key('Escape', 'Escape');

// 29. Preferencias
await fresh();
await app(`void commands.run('prefs.open');`);
await wait(700);
await shot('29-preferencias.png');
await closeDialog();

// 30. Exportar SVG
await fresh();
await app(`void commands.run('fileio.exportSvg');`);
await wait(700);
await shot('30-exportar.png');
await closeDialog();

// 31. Insertar código QR
await fresh();
await app(`void commands.run('tools.insertCode');`);
await wait(900);
await shot('31-codigo-qr.png');
await closeDialog();

// 32. Manual dentro de la app
await fresh();
await app(`void commands.run('help.manual');`);
await wait(900);
await shot('32-manual.png');
await closeDialog();

// ---- Ola 2 (capturas 33 a 38) ---------------------------------------------------------------------------

/** Abre el editor de la capa Corte (doble clic en su fila). */
const openCutEditor = async () => {
  await evaluate(`(() => {
    const row = [...document.querySelectorAll('.lyr-main')].find((e) => e.textContent.includes('Corte'));
    if (!row) throw new Error('sin la capa Corte');
    row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
  })()`);
  await wait(600);
};
/** Abre la sección plegable `title` del editor de capa (si está cerrada) y la deja a la vista. */
const openFold = (title) => evaluate(`(() => {
  const d = [...document.querySelectorAll('details.lyr-fold')].find((e) => e.querySelector('summary')?.textContent.trim().startsWith(${JSON.stringify(title)}));
  if (!d) throw new Error('sin la sección ${title}');
  if (!d.open) d.querySelector('summary').click();
  d.scrollIntoView({ block: 'start' });
  // El encabezado fijo del editor tapa el borde superior: se sube un poco el contenido.
  let box = d.parentElement;
  while (box && !(box.scrollHeight > box.clientHeight && /auto|scroll/.test(getComputedStyle(box).overflowY))) box = box.parentElement;
  if (box) box.scrollTop -= 70;
})()`);

// 33. Editor de capa con "Corte avanzado": perforación y entrada en arco activas
await fresh();
await app(`
  const corte = st().doc.layers.find((l) => l.name.includes('Corte'));
  st().updateLayer(corte.color, {
    perforation: { enabled: true, cut_mm: 4, skip_mm: 2 },
    lead_in: { enabled: true, kind: 'arc', length: 3, angle: 90 },
  });
`);
await openCutEditor();
await openFold('Corte avanzado');
await wait(500);
await shot('33-corte-avanzado.png');
await closeDialog();

// 34. Operaciones adicionales (sub-capas) de la capa de corte
await fresh();
await app(`
  const corte = st().doc.layers.find((l) => l.name.includes('Corte'));
  const { addSubLayer } = await import('/src/features/layers/actions.ts');
  addSubLayer(st(), corte.color, 'line');
  addSubLayer(st(), corte.color, 'fill');
`);
await openCutEditor();
await evaluate(`(() => {
  const g = [...document.querySelectorAll('.lyr-group, fieldset, section, div')].find((e) => e.firstElementChild && /^Operaciones adicionales/.test(e.firstElementChild.textContent.trim()) && e.childElementCount < 8);
  const h = g ?? [...document.querySelectorAll('*')].find((e) => e.childElementCount === 0 && e.textContent.trim() === 'Operaciones adicionales');
  if (!h) throw new Error('sin Operaciones adicionales');
  h.scrollIntoView({ block: 'start' });
  let box = h.parentElement;
  while (box && !(box.scrollHeight > box.clientHeight && /auto|scroll/.test(getComputedStyle(box).overflowY))) box = box.parentElement;
  if (box) box.scrollTop -= 70;
})()`);
await wait(500);
await shot('34-operaciones-adicionales.png');
await closeDialog();

// 35. Ajustes de optimización del trabajo
await fresh();
await app(`void commands.run('preview.optimization');`);
await wait(800);
await shot('35-optimizacion.png');
await closeDialog();

// 36. Diálogo de inicio con el checklist (máquina virtual GRBL 1.1 conectada)
await fresh();
await app(`
  st().setDockTab('machine');
  st().setSelectedPort('virtual:grbl11');
  await st().connect();
`);
await wait(1500);
await app(`void commands.run('machine.start');`);
await wait(900);
await shot('36-checklist-inicio.png');
await closeDialog();

// 37. Panel Máquina con "Ajustes en vivo" y la máquina virtual conectada
await fresh();
await app(`
  st().setDockTab('machine');
  st().setSelectedPort('virtual:grbl11');
  await st().connect();
`);
await wait(1500);
await evaluate(`(() => {
  const el = [...document.querySelectorAll('h2, h3, h4, legend, span, div')].find((e) => e.childElementCount === 0 && e.textContent.trim() === 'Ajustes en vivo');
  if (!el) throw new Error('sin Ajustes en vivo');
  el.scrollIntoView({ block: 'center' });
})()`);
await wait(500);
await shot('37-ajustes-en-vivo.png');

// 38. La interfaz en inglés (preferencia de idioma)
await fresh();
await app(`st().setPref('language', 'en');`);
await wait(500);
// Se recarga con el idioma ya guardado: el cálculo del trabajo (mock) se rehace en inglés.
await fresh();
await click('.canvas-area');
await key('a', 'KeyA', MOD);
await wait(600);
await shot('38-english.png');
await app(`st().setPref('language', 'es');`);


// ---- Ola 3: diseño avanzado (capturas 39 a 47) ---------------------------------------------------------

// La captura 18 dejó el panel lateral en su ancho máximo (se guarda en las preferencias): se devuelve al normal.
await app(`st().setDockWidth(332);`);

// 39. Nesting: la placa, el engranaje y la estrella como piezas, con un resultado calculado
await fresh();
await app(`
  st().select(['obj-placa', 'obj-engranaje', 'obj-estrella']);
  await commands.run('arrange.nest');
`);
await wait(800);
await clickText('.btn', 'Empezar');
await wait(4500); // el cálculo del mock es asíncrono
await shot('39-nesting.png');
await closeDialog();

// 40. Asistente booleano con las miniaturas de cada operación
await fresh();
await app(`
  st().select(['obj-placa', 'obj-engranaje']);
  st().fitContent();
  await commands.run('arrange.boolAssistant');
`);
await wait(1200);
await shot('40-asistente-booleano.png');
await closeDialog();

// 41. Deformar con vista previa en el lienzo
await fresh();
await app(`
  st().select(['obj-estrella']);
  st().fitContent();
  await commands.run('arrange.warp');
`);
await wait(800);
await clickText('.chip', 'Abombar');
await wait(900);
await shot('41-deformar.png');
await closeDialog();

// 42. Arreglo virtual de la estrella: cuadrícula de 3 x 2 con las copias fantasma en el lienzo
await fresh();
await app(`
  st().select(['obj-estrella']);
  st().fitContent();
  await commands.run('arrange.virtualArray');
`);
await wait(800);
await app(`
  const ui = (await import('/src/features/arrange/designUi.ts')).useDesignUi;
  ui.getState().patchVirtual({ kind: 'grid', cols: 3, rows: 2, dx: 8, dy: 8 });
`);
await wait(600);
await clickText('.modal__foot .btn', 'Asignar');
await wait(700);
await app(`st().select(['obj-estrella']); st().fitContent();`);
await wait(900);
await shot('42-arreglo-virtual.png');

// 43. Texto variable: un texto con {serie} y {csv:Nombre}, y un CSV de ejemplo (lo simula el mock)
await fresh();
await app(`
  const tt = await import('/src/features/canvas/textTool.ts');
  tt.createTextAt(st(), [215, 262]);
  st().setTool('select');
`);
await evaluate(`new Promise((ok, fail) => {
  const t0 = Date.now();
  const tick = () => document.querySelector('.text-panel__area') ? ok() : Date.now() - t0 > 15000 ? fail(new Error('sin el panel de texto')) : setTimeout(tick, 200);
  tick();
})`);
await evaluate(`(() => { const a = document.querySelector('.text-panel__area'); a.focus(); a.select(); })()`);
await type('Pieza {serie}: {csv:Nombre}');
await wait(800);
await app(`
  const { editVariableText } = await import('/src/features/variables/settings.ts');
  const { loadCsv } = await import('/src/features/variables/csvSync.ts');
  editVariableText({ csv_path: 'ejemplos/etiquetas.csv', csv_row: 1, serial: 7, serial_digits: 3 }, 'csv');
  await loadCsv('ejemplos/etiquetas.csv');
  await commands.run('tools.variableText');
`);
await wait(1500);
await shot('43-texto-variable.png');
await closeDialog();

// 44. Guías en el lienzo: dos verticales y dos horizontales alrededor de la placa
await fresh();
await app(`
  st().addGuide({ axis: 'x', pos: 60 });
  st().addGuide({ axis: 'x', pos: 280 });
  st().addGuide({ axis: 'y', pos: 70 });
  st().addGuide({ axis: 'y', pos: 210 });
  st().addGuide({ axis: 'x', pos: 170 });
  st().select([]);
  st().fitContent();
`);
await wait(800);
await shot('44-guias.png');

// 45. Puentes manuales (cuatro) y flecha de inicio en la placa, con las marcas a la vista
await fresh();
await app(`
  const m = await import('/src/features/canvas/marksOps.ts');
  st().applyDocChange('canvas.history.tabs', (doc) => {
    let d = doc;
    for (const p of [[170, 70], [60, 140], [280, 140], [170, 210]]) d = m.toggleTab(d, 'obj-placa', p, 4).doc;
    return d;
  });
  st().applyDocChange('canvas.history.start', (doc) => m.setStartPoint(doc, 'obj-placa', [280, 100]));
  const corte = st().doc.layers.find((l) => l.name.includes('Corte'));
  st().updateLayer(corte.color, { tabs: { ...corte.tabs, enabled: true, width: 6 } });
  st().select(['obj-placa']);
  st().setTool('start');
  st().fitContent();
`);
await wait(900);
await shot('45-puentes-inicio.png');

// 46. Biblioteca de arte abierta
await fresh();
await app(`
  const { useArtPanel } = await import('/src/features/art/artStore.ts');
  useArtPanel.getState().setOpen(true);
`);
await wait(1000);
await shot('46-biblioteca-arte.png');

// 47. Optimizar formas: un círculo hecho de 360 segmentos (típico de un SVG exportado) con su conteo de nodos
await fresh();
await app(`
  const { polylineBez } = await import('/src/lib/bezpath.ts');
  const pts = Array.from({ length: 360 }, (_, i) => [335 + 40 * Math.cos((2 * Math.PI * i) / 360), 140 + 40 * Math.sin((2 * Math.PI * i) / 360)]);
  const engrave = st().doc.layers.find((l) => l.name.includes('Grabado'));
  st().applyDocChange('canvas.history.tabs', (doc) => ({
    ...doc,
    objects: [...doc.objects, { id: 'obj-circulo', layer: engrave.color, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [polylineBez(pts, true)] }, locked: false, hidden: false, group: null }],
  }));
  st().select(['obj-circulo']);
  await commands.run('arrange.optimize');
`);
await wait(1200);
await shot('47-optimizar-formas.png');
await closeDialog();


// ---- Ola 4: máquinas (capturas 48 a 57) -----------------------------------------------------------------
// Todas en modo mock. Los comandos que abren diálogos se llaman sin `await` (la promesa no resuelve hasta cerrarlos).

/**
 * Mueve el scroll del contenedor desplazable más cercano (nunca el de la página) hasta el primer
 * elemento hoja cuyo texto empieza por `text`. `block`: 'start' lo pone arriba, 'end' abajo.
 */
const scrollToText = (text, block = 'start', selector = 'h2, h3, h4, legend, label, span, div, p') => evaluate(`(() => {
  const el = [...document.querySelectorAll(${JSON.stringify(selector)})].find((e) => e.childElementCount <= 1 && e.textContent.trim().startsWith(${JSON.stringify(text)}));
  if (!el) throw new Error('sin ${text}');
  let box = el.parentElement;
  while (box && box !== document.body && !(box.scrollHeight > box.clientHeight + 1 && /(auto|scroll)/.test(getComputedStyle(box).overflowY))) box = box.parentElement;
  if (!box || box === document.body) return;
  const r = el.getBoundingClientRect(), b = box.getBoundingClientRect();
  box.scrollTop += ${JSON.stringify(block)} === 'end' ? r.bottom - b.bottom + 12 : r.top - b.top - 8;
})()`);

/** Guarda y activa un perfil derivado del de reserva. */
const useDevice = (patch) => app(`
  const { FALLBACK_DEVICE } = await import('/src/lib/device.ts');
  const dev = { ...FALLBACK_DEVICE, ...${JSON.stringify(patch)} };
  await st().saveDevice(dev);
  st().setActiveDevice(dev.id);
  await st().refreshPorts();
`);

// 48. Máquina virtual Ruida conectada: sección "Controladora Ruida" con la información y los archivos leídos
await fresh();
await useDevice({ id: 'ruida-1', name: 'CO2 Ruida', kind: 'ruida', bed: [600, 400], connection: { type: 'udp', host: '192.168.1.60' } });
await app(`
  st().setDockTab('machine');
  st().setSelectedPort('virtual:ruida');
  await st().connect();
`);
await wait(1500);
await clickText('.btn', 'Leer información');
await wait(600);
await clickText('.btn', 'Leer lista');
await wait(600);
await scrollToText('Controladora Ruida');
await wait(400);
await shot('48-maquina-ruida.png');

// 48b. Lo mismo, desplazado hasta la lista de documentos de la memoria (vacía en la máquina virtual)
await scrollToText('Documentos en la memoria');
await wait(300);
await shot('48b-ruida-documentos.png');

// 49. Máquina virtual K40 (M2 Nano) conectada: panel experimental, sin G-code, sin posición hasta el home
await fresh();
await useDevice({ id: 'k40-1', name: 'Mi K40', kind: 'lhy_micro', bed: [300, 200], origin: 'top_left', lhymicro: { board: 'M2' } });
await app(`
  st().setDockTab('machine');
  st().setSelectedPort('virtual:k40');
  await st().connect();
`);
await wait(1500);
await scrollToText('K40 con M2 Nano', 'start', 'h3');
await wait(300);
await shot('49-maquina-k40.png');

// 50. Diálogo de inicio de un trabajo K40 con los avisos del codificador (potencia por perilla, grises binarizados)
await app(`void commands.run('machine.start');`);
await wait(1200);
await shot('50-k40-inicio.png');
await closeDialog();

// 50b. Aviso del paro de emergencia: la placa K40 no confirmó detenerse (se simula el mensaje del motor)
await fresh();
await useDevice({ id: 'k40-1', name: 'Mi K40', kind: 'lhy_micro', bed: [300, 200], origin: 'top_left', lhymicro: { board: 'M2' } });
await app(`
  st().setDockTab('machine');
  st().setSelectedPort('virtual:k40');
  await st().connect();
`);
await wait(1200);
await app(`
  const { useStore: u } = await import('/src/store/index.ts');
  u.setState({ jobError: 'Detener: NO hubo confirmación de la placa.', jobMessage: 'Detener: NO hubo confirmación de la placa.' });
`);
await wait(500);
await shot('50b-k40-paro-emergencia.png');

// 50c. Diálogo de inicio con Ruida en modo "solo guardar" (el trabajo no arranca solo)
await fresh();
await useDevice({ id: 'ruida-1', name: 'CO2 Ruida', kind: 'ruida', bed: [600, 400], connection: { type: 'udp', host: '192.168.1.60' }, ruida: { magic: null, run_after_upload: false } });
await app(`
  st().setDockTab('machine');
  st().setSelectedPort('virtual:ruida');
  await st().connect();
`);
await wait(1500);
await app(`void commands.run('machine.start');`);
await wait(1200);
await shot('50c-ruida-solo-guarda.png');
await closeDialog();

// 50d. Diálogo de inicio del K40 en inglés: los avisos de seguridad caben igual en el otro idioma
await app(`st().setPref('language', 'en');`);
await wait(400);
await fresh();
await useDevice({ id: 'k40-1', name: 'Mi K40', kind: 'lhy_micro', bed: [300, 200], origin: 'top_left', lhymicro: { board: 'M2' } });
await app(`
  st().setDockTab('machine');
  st().setSelectedPort('virtual:k40');
  await st().connect();
`);
await wait(1500);
await app(`void commands.run('machine.start');`);
await wait(1200);
await shot('50d-k40-inicio-en.png');
await closeDialog();
await app(`st().setPref('language', 'es');`);
await wait(400);

// 51. Perfil de máquina con conexión TCP y el botón "Probar conexión"
await fresh();
await useDevice({ id: 'grbl-red', name: 'GRBL por red', kind: 'grbl', connection: { type: 'tcp', host: '192.168.1.50', port: 23 } });
await app(`void commands.run('machine.devices');`);
await wait(900);
await scrollToText('Tipo de conexión');
await wait(300);
await evaluate(`(() => { const b = [...document.querySelectorAll('.btn')].find((e) => e.textContent.trim().startsWith('Probar conexión')); if (b && !b.disabled) b.click(); })()`);
await wait(900);
await shot('51-perfil-tcp.png');
await closeDialog();

// 52. Editor de macros dentro del perfil (con un riesgo advertido) y los botones en el panel
await fresh();
await useDevice({
  id: 'grbl-macros', name: 'Falcon con macros', kind: 'grbl',
  macros: [
    { name: 'Aire encendido', gcode: 'M8', confirm: true },
    { name: 'Origen de trabajo', gcode: 'G92 X0 Y0', confirm: true },
    { name: 'Pausa de 2 s', gcode: 'G4 P2', confirm: false },
  ],
});
await app(`void commands.run('machine.devices');`);
await wait(900);
await scrollToText('Macros');
await wait(400);
await shot('52-editor-macros.png');
await closeDialog();

// 53. Ajustes de la máquina ($$) con la máquina virtual GRBL 1.1
await fresh();
await app(`
  st().setDockTab('machine');
  st().setSelectedPort('virtual:grbl11');
  await st().connect();
`);
await wait(1500);
await app(`void commands.run('machine.settings');`);
await wait(2000);
await shot('53-ajustes-maquina.png');
await closeDialog();

// 54. Mover con Z, posiciones guardadas y disparo de prueba (con los botones de macro justo debajo)
await fresh();
await useDevice({
  id: 'grbl-z', name: 'Falcon con Z', kind: 'grbl', z_axis: true,
  macros: [{ name: 'Aire encendido', gcode: 'M8', confirm: true }, { name: 'Origen de trabajo', gcode: 'G92 X0 Y0', confirm: true }],
  positions: [{ name: 'Esquina', x: 10, y: 10 }, { name: 'Centro de la cama', x: 200, y: 200 }],
});
await app(`
  st().setDockTab('machine');
  st().setSelectedPort('virtual:grbl11');
  await st().connect();
`);
await wait(1500);
await scrollToText('Ejes extra', 'start', 'h4');
await wait(400);
await shot('54-mover-z-posiciones.png');

// 54b. Disparo de prueba: el diálogo con su doble confirmación
await clickText('.btn', 'Disparo de prueba');
await wait(600);
await shot('54b-disparo-prueba.png');
await closeDialog();

// 55. Calibración de lente con el selector de modelo (ojo de pez)
await fresh();
await app(`
  st().setDockTab('camera');
  st().setCameraCalibrationOpen(true);
`);
await wait(2500);
await evaluate(`(() => { const s = document.querySelector('#cam-model'); const set = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set; set.call(s, 'fisheye'); s.dispatchEvent(new Event('change', { bubbles: true })); })()`);
await wait(500);
await shot('55-calibracion-lente.png');
await closeDialog();

// 56. Cámara en el cabezal y buscador de centro
await fresh();
await app(`
  st().setDockTab('camera');
  const { setHeadEnabled } = await import('/src/features/camera/actions.ts');
  setHeadEnabled(true);
`);
await wait(900);
await scrollToText('Cámara en el cabezal', 'start', 'h3');
await wait(300);
await shot('56-camara-cabezal.png');

// 57. Preferencias, sección Automatización
await fresh();
await app(`
  const { useSettingsUi } = await import('/src/features/settings/uiStore.ts');
  useSettingsUi.getState().openSettings('automation');
  st().setPref('automationEnabled', true);
  st().setPref('automationAllowStart', true);
`);
await wait(800);
await shot('57-automatizacion.png');
await closeDialog();


// ---- Ola 5.5: modo sencillo, materiales de fábrica y control desde el celular (capturas 58 a 63) --------
// 58 (la pregunta del primer arranque) se toma al principio del script, antes de la 01: el diálogo solo aparece
// mientras la preferencia `uiMode` está en "ask", y la captura 01 necesita la interfaz completa sin él.

const setUiMode = (mode) => app(`(await import('/src/features/simple/mode.ts')).setUiMode(${JSON.stringify(mode)})`);
/** Cambia un `<select>` de React (el valor se asigna con el setter nativo para que el evento llegue). */
const pickSimple = (n, label) => evaluate(`(() => { const s = [...document.querySelectorAll('.simple select')][${n}]; const o = [...s.options].find(o => o.textContent.trim() === ${JSON.stringify(label)}); if (!o) throw new Error('sin opción ${label}'); const set = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set; set.call(s, o.value); s.dispatchEvent(new Event('change', { bubbles: true })); })()`);
/** Modo sencillo en el paso `step` (0 a 4), con 20 W y el material pedido; deja las acciones por color sin decidir. */
const simpleAt = async (step, material, watts = '20 W') => {
  await fresh();
  await app(`
    const { setUiMode } = await import('/src/features/simple/mode.ts');
    const { goToStep } = await import('/src/features/simple/actions.ts');
    setUiMode('simple');
    goToStep(${step});
  `);
  await wait(1200);
  await pickSimple(1, watts);
  await wait(300);
  await pickSimple(2, material);
  await wait(500);
};
/** Cortar el contorno, grabar el relleno y la foto, ignorar las marcas (las capas del diseño de ejemplo). */
const decideColors = () => app(`
  const { chooseAction } = await import('/src/features/simple/actions.ts');
  chooseAction('#ff3b4a', 'cut');
  chooseAction('#2f7bff', 'engrave');
  chooseAction('#ffb020', 'ignore');
  chooseAction('#9aa4b2', 'engrave');
`);

// 59. Modo sencillo, paso Material: se contestó la potencia (20 W) y se eligió un material con corte y grabado para Diodo 20 W (Madera de tilo 3 mm, de Creality); avisos de "valores de partida"
await simpleAt(1, 'Madera de tilo 3 mm');
await decideColors();
await wait(400);
await shot('59-simple-material.png');

// 59b. Lo mismo, con "Ajustar" abierto en Corte, un cambio hecho y "Ver números" desplegado (Volver al material / Guardar como mi material)
await scrollToText('Corte', 'start', 'strong');
await wait(300);
await clickText('.simple__color[data-color="#ff3b4a"] .btn', 'Ajustar');
await wait(300);
await clickText('.simple__color[data-color="#ff3b4a"] .btn', 'Quema mucho');
await wait(300);
await clickText('.simple__color[data-color="#ff3b4a"] summary', 'Ver números');
await wait(400);
await scrollToText('Corte', 'start', 'strong');
await wait(300);
await shot('59b-simple-ajustar.png');

// 59c. Acrílico transparente con un láser de diodo: "Cortar" no se deja elegir y dice por qué
await simpleAt(1, 'Acrílico transparente 3 mm');
await scrollToText('Material', 'start', 'label');
await wait(300);
await shot('59c-simple-acrilico.png');

// 60. Paso Revisar: tiempo estimado y vista previa del recorrido; 60b. paso Iniciar con la máquina virtual conectada
await simpleAt(1, 'Madera de tilo 3 mm');
await decideColors();
await app(`(await import('/src/features/simple/actions.ts')).goToStep(3)`);
await wait(3000);
await shot('60-simple-revisar.png');
await app(`(await import('/src/features/simple/actions.ts')).goToStep(4)`);
await wait(1500);
await clickText('.simple .btn', 'Probar con la máquina virtual');
await wait(2500);
await shot('60b-simple-iniciar.png');

// 61. Panel Materiales con la clase de máquina elegida (Diodo 20 W); 61b. la lista "De fábrica" con confianza y fuente
await fresh();
await setUiMode('full');
await app(`st().setDockTab('materials')`);
await wait(1500);
await evaluate(`(() => { const s = document.querySelector('.mat-panel select'); const set = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set; const o = [...s.options].find(o => /20/.test(o.textContent)); set.call(s, o.value); s.dispatchEvent(new Event('change', { bubbles: true })); })()`);
await wait(500);
await shot('61-materiales-clase.png');
await scrollToText('De fábrica', 'start', 'h3');
await wait(400);
await shot('61b-materiales-de-fabrica.png');

// 62. Preferencias > Control remoto con el QR. El mock no sabe dibujar un QR: se le pone uno real (de un enlace de ejemplo)
const QR_SVG = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 37 37" shape-rendering="crispEdges"><rect width="37" height="37" fill="#fff"/><path d="M2 2h7v1h-7zM12 2h2v1h-2zM17 2h2v1h-2zM20 2h3v1h-3zM24 2h2v1h-2zM28 2h7v1h-7zM2 3h1v1h-1zM8 3h1v1h-1zM10 3h2v1h-2zM15 3h2v1h-2zM19 3h1v1h-1zM22 3h5v1h-5zM28 3h1v1h-1zM34 3h1v1h-1zM2 4h1v1h-1zM4 4h3v1h-3zM8 4h1v1h-1zM10 4h3v1h-3zM15 4h1v1h-1zM18 4h3v1h-3zM22 4h1v1h-1zM25 4h1v1h-1zM28 4h1v1h-1zM30 4h3v1h-3zM34 4h1v1h-1zM2 5h1v1h-1zM4 5h3v1h-3zM8 5h1v1h-1zM10 5h1v1h-1zM12 5h1v1h-1zM14 5h1v1h-1zM17 5h1v1h-1zM25 5h2v1h-2zM28 5h1v1h-1zM30 5h3v1h-3zM34 5h1v1h-1zM2 6h1v1h-1zM4 6h3v1h-3zM8 6h1v1h-1zM11 6h1v1h-1zM14 6h1v1h-1zM17 6h1v1h-1zM20 6h1v1h-1zM23 6h1v1h-1zM26 6h1v1h-1zM28 6h1v1h-1zM30 6h3v1h-3zM34 6h1v1h-1zM2 7h1v1h-1zM8 7h1v1h-1zM11 7h2v1h-2zM14 7h1v1h-1zM16 7h1v1h-1zM18 7h1v1h-1zM22 7h3v1h-3zM26 7h1v1h-1zM28 7h1v1h-1zM34 7h1v1h-1zM2 8h7v1h-7zM10 8h1v1h-1zM12 8h1v1h-1zM14 8h1v1h-1zM16 8h1v1h-1zM18 8h1v1h-1zM20 8h1v1h-1zM22 8h1v1h-1zM24 8h1v1h-1zM26 8h1v1h-1zM28 8h7v1h-7zM10 9h1v1h-1zM12 9h5v1h-5zM23 9h2v1h-2zM26 9h1v1h-1zM2 10h1v1h-1zM8 10h1v1h-1zM10 10h2v1h-2zM13 10h1v1h-1zM17 10h2v1h-2zM22 10h3v1h-3zM26 10h3v1h-3zM31 10h3v1h-3zM3 11h5v1h-5zM9 11h1v1h-1zM13 11h2v1h-2zM18 11h1v1h-1zM20 11h1v1h-1zM22 11h1v1h-1zM25 11h1v1h-1zM27 11h1v1h-1zM29 11h1v1h-1zM31 11h4v1h-4zM2 12h7v1h-7zM12 12h2v1h-2zM20 12h1v1h-1zM23 12h1v1h-1zM30 12h1v1h-1zM32 12h1v1h-1zM34 12h1v1h-1zM2 13h2v1h-2zM12 13h1v1h-1zM14 13h1v1h-1zM16 13h1v1h-1zM18 13h1v1h-1zM21 13h1v1h-1zM24 13h1v1h-1zM26 13h1v1h-1zM29 13h1v1h-1zM31 13h2v1h-2zM2 14h2v1h-2zM5 14h1v1h-1zM8 14h2v1h-2zM11 14h1v1h-1zM14 14h3v1h-3zM18 14h10v1h-10zM30 14h1v1h-1zM34 14h1v1h-1zM2 15h1v1h-1zM11 15h2v1h-2zM15 15h2v1h-2zM18 15h2v1h-2zM22 15h3v1h-3zM26 15h1v1h-1zM31 15h1v1h-1zM33 15h2v1h-2zM2 16h1v1h-1zM4 16h1v1h-1zM8 16h1v1h-1zM10 16h1v1h-1zM14 16h1v1h-1zM16 16h1v1h-1zM20 16h2v1h-2zM23 16h1v1h-1zM25 16h1v1h-1zM31 16h3v1h-3zM2 17h2v1h-2zM5 17h1v1h-1zM7 17h1v1h-1zM11 17h5v1h-5zM17 17h1v1h-1zM19 17h3v1h-3zM25 17h2v1h-2zM29 17h1v1h-1zM31 17h1v1h-1zM34 17h1v1h-1zM4 18h2v1h-2zM8 18h1v1h-1zM12 18h1v1h-1zM15 18h1v1h-1zM21 18h1v1h-1zM23 18h1v1h-1zM25 18h1v1h-1zM29 18h4v1h-4zM34 18h1v1h-1zM2 19h1v1h-1zM4 19h1v1h-1zM6 19h1v1h-1zM13 19h4v1h-4zM18 19h2v1h-2zM23 19h1v1h-1zM26 19h1v1h-1zM29 19h5v1h-5zM4 20h1v1h-1zM7 20h4v1h-4zM14 20h1v1h-1zM17 20h1v1h-1zM21 20h1v1h-1zM31 20h2v1h-2zM34 20h1v1h-1zM3 21h2v1h-2zM6 21h2v1h-2zM10 21h2v1h-2zM14 21h4v1h-4zM19 21h2v1h-2zM22 21h2v1h-2zM26 21h1v1h-1zM28 21h2v1h-2zM31 21h2v1h-2zM34 21h1v1h-1zM3 22h1v1h-1zM7 22h3v1h-3zM11 22h3v1h-3zM16 22h1v1h-1zM18 22h1v1h-1zM21 22h4v1h-4zM31 22h1v1h-1zM33 22h2v1h-2zM2 23h1v1h-1zM6 23h2v1h-2zM9 23h3v1h-3zM13 23h1v1h-1zM15 23h1v1h-1zM18 23h3v1h-3zM22 23h1v1h-1zM24 23h1v1h-1zM26 23h3v1h-3zM31 23h2v1h-2zM2 24h1v1h-1zM4 24h2v1h-2zM8 24h2v1h-2zM12 24h4v1h-4zM17 24h4v1h-4zM22 24h2v1h-2zM26 24h2v1h-2zM29 24h2v1h-2zM32 24h2v1h-2zM2 25h1v1h-1zM4 25h3v1h-3zM9 25h3v1h-3zM14 25h1v1h-1zM19 25h2v1h-2zM24 25h3v1h-3zM28 25h2v1h-2zM32 25h1v1h-1zM2 26h4v1h-4zM7 26h4v1h-4zM13 26h3v1h-3zM17 26h1v1h-1zM19 26h1v1h-1zM21 26h2v1h-2zM25 26h8v1h-8zM34 26h1v1h-1zM10 27h3v1h-3zM14 27h1v1h-1zM16 27h1v1h-1zM18 27h1v1h-1zM20 27h2v1h-2zM25 27h2v1h-2zM30 27h1v1h-1zM32 27h1v1h-1zM2 28h7v1h-7zM17 28h1v1h-1zM20 28h1v1h-1zM22 28h2v1h-2zM25 28h2v1h-2zM28 28h1v1h-1zM30 28h1v1h-1zM32 28h1v1h-1zM2 29h1v1h-1zM8 29h1v1h-1zM14 29h1v1h-1zM16 29h1v1h-1zM21 29h2v1h-2zM26 29h1v1h-1zM30 29h1v1h-1zM32 29h3v1h-3zM2 30h1v1h-1zM4 30h3v1h-3zM8 30h1v1h-1zM14 30h2v1h-2zM17 30h1v1h-1zM20 30h2v1h-2zM23 30h1v1h-1zM25 30h6v1h-6zM34 30h1v1h-1zM2 31h1v1h-1zM4 31h3v1h-3zM8 31h1v1h-1zM11 31h5v1h-5zM18 31h2v1h-2zM22 31h1v1h-1zM24 31h2v1h-2zM27 31h1v1h-1zM29 31h3v1h-3zM2 32h1v1h-1zM4 32h3v1h-3zM8 32h1v1h-1zM11 32h2v1h-2zM18 32h2v1h-2zM22 32h3v1h-3zM28 32h2v1h-2zM32 32h3v1h-3zM2 33h1v1h-1zM8 33h1v1h-1zM11 33h1v1h-1zM13 33h1v1h-1zM16 33h4v1h-4zM21 33h3v1h-3zM25 33h2v1h-2zM28 33h1v1h-1zM32 33h1v1h-1zM2 34h7v1h-7zM10 34h1v1h-1zM12 34h1v1h-1zM15 34h1v1h-1zM18 34h1v1h-1zM21 34h2v1h-2zM25 34h1v1h-1zM29 34h1v1h-1zM31 34h3v1h-3z" fill="#000"/></svg>';
await fresh();
await app(`
  const { useSettingsUi } = await import('/src/features/settings/uiStore.ts');
  useSettingsUi.getState().openSettings('remote');
  st().setPref('remoteEnabled', true);
`);
await wait(1500);
await app(`
  const { useRemote } = await import('/src/features/remote/remote.ts');
  const s = useRemote.getState().status;
  if (!s || !s.running) throw new Error('el control remoto no arrancó');
  useRemote.setState({ status: { ...s, ip: '192.168.1.20', port: 47802, url: 'http://192.168.1.20:47802/', full_url: 'http://192.168.1.20:47802/#k7Qm2xVd9RwP4ZtB8cHn', qr_svg: ${JSON.stringify(QR_SVG)} } });
`);
await wait(600);
await shot('62-control-remoto.png');
// 62b. Lo que queda debajo: quién está conectado y qué puede hacer el celular
await evaluate(`(() => { const el = [...document.querySelectorAll('*')].find((e) => e.scrollHeight > e.clientHeight + 40 && /(auto|scroll)/.test(getComputedStyle(e).overflowY) && e.closest('[role=dialog]')); if (el) el.scrollTop = el.scrollHeight; })()`);
await wait(400);
await shot('62b-control-remoto-reglas.png');
await closeDialog();

// ---- Ola 6D: revisar diseño, edición en el lienzo, imágenes, registro de trabajos y galvo (capturas 64 a 71) ----
// Todas en modo mock. El mock de la vista previa de imagen y el de «Revisar diseño» dan datos de ejemplo, no los del motor.

// 64. Revisar diseño
await fresh();
await app(`
  const { polylineBez } = await import('/src/lib/bezpath.ts');
  const cut = st().doc.layers.find((l) => l.name.includes('Corte'));
  const mk = (id, layer, paths, tr = [1, 0, 0, 1, 0, 0]) => ({ id, layer, transform: tr, kind: { type: 'vector', paths }, locked: false, hidden: false, group: null });
  st().applyDocChange('canvas.history.tabs', (doc) => ({
    ...doc,
    layers: [...doc.layers, { ...cut, color: '#22c55e', name: 'Pruebas', output: false }],
    objects: [
      ...doc.objects,
      mk('obj-casi', cut.color, [polylineBez([[20, 20], [60, 20], [60, 50], [20, 50], [20, 20.3]], false)]),
      mk('obj-dup-a', cut.color, [polylineBez([[80, 20], [120, 20]], false)]),
      mk('obj-dup-b', cut.color, [polylineBez([[80, 20], [120, 20]], false)]),
      mk('obj-mota', cut.color, [polylineBez([[150, 20], [150.1, 20], [150.1, 20.1], [150, 20.1]], true)]),
      mk('obj-fuera', cut.color, [polylineBez([[0, 0], [30, 0], [30, 30], [0, 30]], true)], [1, 0, 0, 1, 520, 100]),
      mk('obj-silencio', '#22c55e', [polylineBez([[170, 20], [200, 20], [200, 50], [170, 50]], true)]),
    ],
  }));
  st().fitContent();
  await commands.run('review.open');
`);
await wait(1500);
await shot('64-revisar-diseno.png');
await closeDialog();
// 65. Deformar en el lienzo con la malla de 4×4
await fresh();
await app(`
  st().select(['obj-placa', 'obj-engranaje']);
  st().fitContent();
  st().setTool('warp');
`);
await wait(600);
await app(`await commands.run('warp.mode.mesh');`);
await wait(500);
// Se arrastran manijas de la malla con el ratón de verdad
const drag = async (x0, y0, x1, y1) => {
  const m = (type, x, y) => send('Input.dispatchMouseEvent', { type, x, y, button: type === 'mouseMoved' ? 'none' : 'left', buttons: type === 'mouseReleased' ? 0 : (type === 'mousePressed' ? 1 : 1), clickCount: type === 'mouseMoved' ? 0 : 1 });
  await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: x0, y: y0, button: 'none', buttons: 0 });
  await m('mousePressed', x0, y0);
  for (let i = 1; i <= 6; i++) await m('mouseMoved', x0 + ((x1 - x0) * i) / 6, y0 + ((y1 - y0) * i) / 6);
  await m('mouseReleased', x1, y1);
  await wait(200);
};
await drag(437, 383, 390, 330);
await drag(741, 576, 800, 650);
await drag(741, 383, 790, 330);
await drag(437, 576, 400, 620);
await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 600, y: 520, button: 'none', buttons: 0 });
await wait(900);
await shot('65-deformar-lienzo.png');
await key('Escape', 'Escape');
await wait(300);
// 66. Redondear esquinas con un clic: la barra de la herramienta y la vista previa de una esquina
await fresh();
await app(`
  const { polylineBez } = await import('/src/lib/bezpath.ts');
  const cut = st().doc.layers.find((l) => l.name.includes('Corte'));
  st().applyDocChange('canvas.history.tabs', () => ({
    ...st().doc,
    objects: [{ id: 'obj-regla', layer: cut.color, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [polylineBez([[100, 300], [260, 300]], false)] }, locked: false, hidden: false, group: null }, { id: 'obj-caja', layer: cut.color, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [polylineBez([[100, 100], [260, 100], [260, 220], [100, 220]], true)] }, locked: false, hidden: false, group: null }],
  }));
  st().select([]);
  st().fitContent();
  st().select(['obj-caja']);
  const ops = (await import('/src/features/arrange/shapeOpsUi.ts')).useShapeOps;
  ops.getState().patchFillet({ style: 'round', radius: 12 });
  st().setTool('fillet');
`);
await wait(500);
// Un clic en la esquina de arriba a la izquierda (queda redondeada) y el puntero sobre la de arriba a la derecha
const mouse = (type, x, y) => send('Input.dispatchMouseEvent', { type, x, y, button: type === 'mouseMoved' ? 'none' : 'left', buttons: type === 'mousePressed' ? 1 : 0, clickCount: type === 'mouseMoved' ? 0 : 1 });
await mouse('mouseMoved', 356, 422);
await wait(200);
await mouse('mousePressed', 356, 422);
await mouse('mouseReleased', 356, 422);
await wait(800);
await evaluate(`document.querySelectorAll('.toast button').forEach(b => b.click())`);
await mouse('mouseMoved', 380, 740);
await mouse('mouseMoved', 357, 771);
await wait(700);
await shot('66-redondear-clic.png');
await key('Escape', 'Escape');
await wait(300);
// 67. Panel flotante: el Historial sacado a una ventana sobre el lienzo
await fresh();
await app(`
  st().select(['obj-placa']);
  const dl = await import('/src/lib/dockLayout.ts');
  st().setPref('dockLayout', dl.floatDockTab(st().prefs.dockLayout, 'history', { x: 150, y: 150, w: 330, h: 330 }));
`);
await wait(300);
await key('ArrowRight', 'ArrowRight', SHIFT);
await key('ArrowUp', 'ArrowUp', SHIFT);
await wait(900);
await shot('67-panel-flotante.png');
// La distribución se guarda en las preferencias: se devuelve a la de fábrica para que no salga flotando en las capturas siguientes.
await app(`st().setPref('dockLayout', (await import('/src/lib/dockLayout.ts')).DEFAULT_DOCK_LAYOUT);`);
await wait(300);
// 68. Editor de la capa Foto: preset de imagen «Foto en madera» y su vista previa por imagen
await fresh();
await evaluate(`(() => {
  const row = [...document.querySelectorAll('.lyr-main')].find((e) => e.textContent.includes('Foto'));
  if (!row) throw new Error('sin la capa Foto');
  row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
})()`);
await wait(700);
await evaluate(`(() => {
  const s = document.querySelector('.img-presets select');
  if (!s) throw new Error('sin el selector de presets');
  const o = [...s.options].find((o) => o.textContent.trim() === 'Foto en madera');
  if (!o) throw new Error('sin el preset');
  const set = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set;
  set.call(s, o.value);
  s.dispatchEvent(new Event('change', { bubbles: true }));
})()`);
await wait(500);
await evaluate(`(() => { const b = [...document.querySelectorAll('button')].find((e) => e.textContent.trim().startsWith('Vista previa de la imagen')); if (!b) throw new Error('sin el botón'); b.click(); })()`);
await wait(1800);
await evaluate(`(() => {
  const el = document.querySelector('.img-presets');
  el.scrollIntoView({ block: 'start' });
  let box = el.parentElement;
  while (box && box !== document.body && !(box.scrollHeight > box.clientHeight + 1 && /(auto|scroll)/.test(getComputedStyle(box).overflowY))) box = box.parentElement;
  if (box && box !== document.body) box.scrollTop -= 110; // lo que tapa el encabezado fijo del editor
})()`);
await wait(500);
await shot('68-vista-previa-imagen.png');
await closeDialog();
// 69. Registro de trabajos (Ayuda): tres trabajos, uno terminado, uno cancelado y uno con error
await fresh();
await app(`
  const { useJobLog } = await import('/src/features/help/jobLog.ts');
  useJobLog.getState().clear();
  const now = Date.now();
  const add = (id, minsAgo, machine, document, estimatedS, actualS, result, message) => useJobLog.getState().append({ id, at: now - minsAgo * 60000, machine, document, estimatedS, actualS, result, message });
  add('a', 5 * 24 * 60, 'Falcon2 22 W', 'placa-engranaje.tlaku', 842, 861, 'finished', 'Trabajo terminado');
  add('b', 3 * 24 * 60 + 90, 'Falcon2 22 W', 'llaveros.tlaku', 1260, 410, 'cancelled', 'Detenido por el usuario');
  add('c', 26 * 60, 'Banco pequeño 300 x 200', null, null, 52, 'error', 'ALARM:1 Límite duro alcanzado');
  add('d', 130, 'Falcon2 22 W', 'placa-engranaje.tlaku', 842, 848, 'finished', 'Trabajo terminado');
  await commands.run('help.jobLog');
`);
await wait(900);
await shot('69-registro-de-trabajos.png');
await app(`(await import('/src/features/help/jobLog.ts')).useJobLog.getState().clear();`);
await closeDialog();
// 70. Galvo (tarjeta simulada): el panel Máquina con el encuadre (casco convexo, contador de vueltas) y el ajuste en vivo
await fresh();
await app(`
  const { FALLBACK_DEVICE } = await import('/src/lib/device.ts');
  const { polylineBez, ellipseBez } = await import('/src/lib/bezpath.ts');
  const { shapes } = await import('/src/lib/geometry.ts');
  const dev = { ...structuredClone(FALLBACK_DEVICE), id: 'galvo-1', name: 'Fibra 20 W (galvo)', kind: 'galvo', bed: [110, 110], origin: 'bottom_left' };
  await st().saveDevice(dev);
  st().setActiveDevice(dev.id);
  await st().refreshPorts();
  const layer = st().doc.layers.find((l) => l.name.includes('Marcas'));
  const mk = (id, paths) => ({ id, layer: layer.color, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths }, locked: false, hidden: false, group: null });
  st().applyDocChange('canvas.history.tabs', () => ({
    ...st().doc,
    objects: [
      mk('g-estrella', [polylineBez(shapes.star(62, 62, 30, 12), true)]),
      mk('g-caja', [polylineBez([[12, 78], [38, 78], [38, 98], [12, 98]], true)]),
      mk('g-circulo', [ellipseBez(88, 24, 10, 10)]),
    ],
  }));
  st().select([]);
  st().fitContent();
  st().setDockTab('machine');
  st().setSelectedPort('virtual:galvo');
`);
await wait(500);
await app(`await st().connect();`);
await wait(2000);
await scrollToText('Encuadre con el puntero rojo', 'start', 'h2, h3, h4, legend, label, span, div, p');
await wait(500);
await evaluate(`(() => {
  const set = (el, proto, v) => { Object.getOwnPropertyDescriptor(proto, 'value').set.call(el, v); };
  const sel = [...document.querySelectorAll('select')].find((e) => [...e.options].some((o) => o.textContent.trim() === 'Casco convexo'));
  if (!sel) throw new Error('sin el selector de forma');
  set(sel, HTMLSelectElement.prototype, [...sel.options].find((o) => o.textContent.trim() === 'Casco convexo').value);
  sel.dispatchEvent(new Event('change', { bubbles: true }));
})()`);
await wait(300);
await evaluate(`(() => {
  const inp = [...document.querySelectorAll('input')].find((e) => e.closest('div')?.textContent.includes('Cuántas vueltas dibuja'));
  if (!inp) throw new Error('sin el campo de vueltas');
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(inp, '40');
  inp.dispatchEvent(new Event('input', { bubbles: true }));
  inp.dispatchEvent(new Event('change', { bubbles: true }));
  inp.blur();
})()`);
await wait(300);
await click('.mc-galvo-frame > button.btn--accent');
await wait(900);
const adjustBtn = (label) => evaluate(`(() => { const b = [...document.querySelectorAll('button')].find((e) => e.getAttribute('aria-label') === ${JSON.stringify(label)}); if (!b) throw new Error('sin ${label}'); b.click(); })()`);
await adjustBtn('Derecha');
await adjustBtn('Más grande');
await wait(300);
await scrollToText('Encuadre con el puntero rojo', 'start', 'h2, h3, h4, legend, label, span, div, p');
await wait(200);
await shot('70-galvo-encuadre.png');
// Se detiene el encuadre antes de seguir
await evaluate(`(() => { const b = [...document.querySelectorAll('button')].find((e) => e.textContent.trim() === 'Detener y apagar'); if (b) b.click(); })()`);
await wait(500);
// 71. Diálogo de inicio con el aviso de Revisar diseño (un objeto fuera de la cama), máquina virtual GRBL 1.1 conectada
await fresh();
await app(`
  const { polylineBez } = await import('/src/lib/bezpath.ts');
  const cut = st().doc.layers.find((l) => l.name.includes('Corte'));
  st().applyDocChange('canvas.history.tabs', (doc) => ({
    ...doc,
    objects: [...doc.objects, { id: 'obj-fuera', layer: cut.color, transform: [1, 0, 0, 1, 380, 100], kind: { type: 'vector', paths: [polylineBez([[0, 0], [40, 0], [40, 40], [0, 40]], true)] }, locked: false, hidden: false, group: null }],
  }));
  st().setDockTab('machine');
  st().setSelectedPort('virtual:grbl11');
  await st().connect();
`);
await wait(1500);
await app(`void commands.run('machine.start');`);
await wait(1500);
await shot('71-inicio-revisar-aviso.png');
await closeDialog();

// 74. Vista previa «por tonos»: un diseño mixto (corte, relleno y foto) simulado sobre madera clara
await fresh();
await app(`
  const { polylineBez } = await import('/src/lib/bezpath.ts');
  const { makeLayer } = await import('/src/lib/layers.ts');
  const { useTonal } = await import('/src/features/tonal/store.ts');
  const poly = (pts) => polylineBez(pts, true);
  const base = { locked: false, hidden: false, group: null };
  const rect = (x, y, w, h) => [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
  const hexagon = (cx, cy, r) => Array.from({ length: 6 }, (_, i) => [cx + r * Math.cos((i * Math.PI) / 3), cy + r * Math.sin((i * Math.PI) / 3)]);
  const objects = [
    { id: 'marco', layer: '#ff3b4a', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [poly(rect(10, 10, 170, 110))] }, ...base },
    { id: 'sello', layer: '#2fd18f', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [poly(hexagon(145, 65, 26))] }, ...base },
    { id: 'banda', layer: '#2fd18f', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [poly(rect(20, 22, 70, 14))] }, ...base },
    { id: 'foto', layer: '#2f7bff', transform: [1, 0, 0, 1, 24, 44], kind: { type: 'image', source_path: 'retrato.png', width_px: 600, height_px: 600, size_mm: [66, 66], mask: [], variable: null }, ...base },
  ];
  const layers = [
    makeLayer('#ff3b4a', 'Corte', { mode: 'line', speed: 300, power: 85, passes: 2 }),
    makeLayer('#2fd18f', 'Relleno', { mode: 'fill', interval: 0.1, speed: 2200, power: 45 }),
    makeLayer('#2f7bff', 'Foto', { mode: 'image', interval: 0.1, speed: 1400, power: 60 }),
  ];
  st().replaceDocument({ version: 2, objects, layers, device_id: null }, null);
  st().setPreviewVisible(true);
  await st().computePreview();
  st().fitContent();
  useTonal.getState().setMode('tonal');
`);
await wait(2500);
await shot('74-vista-por-tonos.png');

// 63. La página del celular en un viewport de teléfono (390 x 844). Es la página real (`page.html`) servida por el
// servidor de demostración del motor contra una máquina virtual: se levanta con `cargo test ... demo_server`.
// Si ya hay uno abierto, REMOTE_DEMO_URL=<enlace con clave> evita compilar y arrancar otro.
let demo = null;
let demoUrl = process.env.REMOTE_DEMO_URL ?? '';
if (!demoUrl && process.env.SCREENSHOTS_SKIP_PHONE !== '1') {
  const root = join(here, '..', '..', '..');
  demo = spawn(process.env.CARGO ?? 'cargo', ['test', '-p', 'tlaku-desktop', 'demo_server', '--', '--ignored', '--nocapture'], {
    cwd: root, env: { ...process.env, TLAKU_DEMO_SECS: '300' }, stdio: ['ignore', 'pipe', 'inherit'], detached: true,
  });
  let buf = '';
  demo.stdout.on('data', (d) => { buf += d; const m = /Enlace para el celular: (\S+)/.exec(buf); if (m) demoUrl = m[1]; });
  for (let i = 0; i < 1200 && !demoUrl; i++) await wait(250); // hasta 5 minutos de compilación
}
if (demoUrl) {
  await send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 2, mobile: true });
  await send('Emulation.setUserAgentOverride', { userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1', acceptLanguage: 'es-MX,es' });
  await send('Emulation.setTouchEmulationEnabled', { enabled: true });
  await send('Page.navigate', { url: demoUrl });
  await wait(3500);
  // La imagen de la cámara de la demostración es un degradado de relleno: se oculta para no aparentar una cámara real.
  const hideCam = () => evaluate(`(() => { const c = document.getElementById('cam'); if (c) c.style.display = 'none'; })()`);
  const scrollEnd = () => evaluate(`window.scrollTo(0, document.body.scrollHeight)`);
  await hideCam();
  await shot('63-celular.png');
  await scrollEnd();
  await wait(400);
  await shot('63b-celular-botones.png');
  await click('#btn-start');
  await wait(800);
  await click('#c1'); await click('#c2'); await click('#c3');
  await wait(300);
  await shot('63c-celular-checklist.png');
  await click('#sheet-go');
  await wait(5000);
  await hideCam();
  await evaluate(`window.scrollTo(0, 0)`);
  await wait(300);
  await shot('63d-celular-trabajando.png');
  await scrollEnd();
  await wait(400);
  await shot('63e-celular-trabajando-botones.png');
  await send('Emulation.setTouchEmulationEnabled', { enabled: false });
  await send('Emulation.setUserAgentOverride', { userAgent: '' });
  await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
} else {
  console.warn('AVISO: sin servidor de demostración (no se pudo levantar o SCREENSHOTS_SKIP_PHONE=1); se omiten las capturas 63.');
}
if (demo) { try { process.kill(-demo.pid); } catch { /* ya terminó */ } }

await send('Browser.close').catch(() => {});
ws.close();
proc.kill();
for (const [name, png] of shots) writeFileSync(join(out, name), png);
console.log(`guardadas ${shots.size} capturas en ${out}`);
