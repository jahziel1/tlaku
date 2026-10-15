// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { setLanguage } from '@/i18n';
import { FALLBACK_DEVICE } from '@/lib/device';
import { NO_PANEL_SECTIONS, sanitizePanelSections } from '@/lib/panelSections';
import { MachinePanel } from '@/features/machine/MachinePanel';
import { applyBundle, buildBundle } from '@/features/settings/bundle';
import { PREF_KEYS } from '@/store/prefs';
import { useStore } from '@/store';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { PanelSection, resetPanelSections, revealPanelSection } from './PanelSection';

const toggleOf = (id: string) => document.querySelector<HTMLButtonElement>(`[data-section="${id}"] [data-psec-toggle]`);
const sectionOf = (id: string) => document.querySelector<HTMLElement>(`[data-section="${id}"]`);
const isOpen = (id: string) => sectionOf(id)?.dataset.open === 'true';
const openIds = () => [...document.querySelectorAll<HTMLElement>('[data-section]')].filter((e) => e.dataset.open === 'true').map((e) => e.dataset.section);

beforeEach(() => {
  setLanguage('es');
  setApi(createMockApi({ latencyMs: 0 }));
  resetPanelSections();
  useStore.setState({ devices: [FALLBACK_DEVICE], activeDeviceId: FALLBACK_DEVICE.id, connection: { connected: false, port: null, error: null }, ports: ['virtual:falcon2'], selectedPort: 'virtual:falcon2', dockTab: 'layers' });
});
afterEach(cleanup);

describe('PanelSection: acordeón accesible', () => {
  it('el encabezado es un botón con aria-expanded y aria-controls que apunta al contenido', async () => {
    await mount(<PanelSection id="t.uno" title="Uno"><p>contenido</p></PanelSection>);
    const btn = toggleOf('t.uno')!;
    expect(btn.tagName).toBe('BUTTON');
    expect(btn.type).toBe('button');
    expect(btn.getAttribute('aria-expanded')).toBe('false');
    const body = document.getElementById(btn.getAttribute('aria-controls')!)!;
    expect(body.hidden).toBe(true);
    // El botón está dentro del encabezado de la sección, así que el nombre de la sección es su título.
    expect(btn.closest('h3')).not.toBeNull();
    expect(sectionOf('t.uno')!.getAttribute('aria-labelledby')).toBe(btn.closest('h3')!.id);
    await click(btn);
    expect(btn.getAttribute('aria-expanded')).toBe('true');
    expect(body.hidden).toBe(false);
  });

  it('se opera con el teclado: es un botón nativo, entra en el orden de Tab y se enfoca', async () => {
    await mount(<PanelSection id="t.dos" title="Dos"><button type="button">dentro</button></PanelSection>);
    const btn = toggleOf('t.dos')!;
    expect(btn.tabIndex).toBe(0);
    btn.focus();
    expect(document.activeElement).toBe(btn);
    // Enter y Espacio sobre un botón nativo disparan `click`: es lo que simula el navegador.
    await click(btn);
    expect(isOpen('t.dos')).toBe(true);
    await click(btn);
    expect(isOpen('t.dos')).toBe(false);
  });

  it('axe no encuentra problemas, abierta o cerrada', async () => {
    const m = await mount(
      <>
        <PanelSection id="t.a" title="A" summary="resumen" defaultOpen><label>Campo <input /></label></PanelSection>
        <PanelSection id="t.b" title="B"><label>Otro <input /></label></PanelSection>
        <PanelSection id="t.c" title="C" nested><p>x</p></PanelSection>
      </>,
    );
    expect(describeViolations(await audit(m.host))).toEqual([]);
  });

  it('recuerda por sección en las preferencias y lo recupera al volver a montar', async () => {
    const m = await mount(<PanelSection id="t.tres" title="Tres"><p>x</p></PanelSection>);
    await click(toggleOf('t.tres')!);
    expect(useStore.getState().prefs.panelSections).toEqual({ 't.tres': true });
    await m.unmount();
    await mount(<PanelSection id="t.tres" title="Tres"><p>x</p></PanelSection>);
    expect(isOpen('t.tres')).toBe(true);
    // Cerrar una abierta por omisión también se recuerda.
    await mount(<PanelSection id="t.cuatro" title="Cuatro" defaultOpen><p>x</p></PanelSection>);
    await click(toggleOf('t.cuatro')!);
    expect(useStore.getState().prefs.panelSections['t.cuatro']).toBe(false);
  });

  it('«Restablecer el panel» devuelve cada sección a su valor por omisión', async () => {
    await mount(
      <>
        <PanelSection id="t.o" title="O" defaultOpen><p>x</p></PanelSection>
        <PanelSection id="t.c" title="C"><p>x</p></PanelSection>
      </>,
    );
    await click(toggleOf('t.o')!);
    await click(toggleOf('t.c')!);
    expect(openIds()).toEqual(['t.c']);
    await Promise.resolve(resetPanelSections());
    await click(document.body);
    expect(useStore.getState().prefs.panelSections).toEqual(NO_PANEL_SECTIONS);
    expect(openIds()).toEqual(['t.o']);
  });

  it('sin plegado (alwaysOpen) no hay botón y el contenido se ve', async () => {
    await mount(<PanelSection id="t.fijo" title="Fijo" alwaysOpen><p>x</p></PanelSection>);
    expect(toggleOf('t.fijo')).toBeNull();
    expect(isOpen('t.fijo')).toBe(true);
  });
});

describe('preferencia panelSections', () => {
  it('se sanea: solo ids con forma de id y valores booleanos', () => {
    expect(sanitizePanelSections({ 'machine.job': true, 'x y': true, ok: 'sí', 'machine.jog': false })).toEqual({ 'machine.job': true, 'machine.jog': false });
    expect(sanitizePanelSections(null)).toBe(NO_PANEL_SECTIONS);
    expect(sanitizePanelSections([true])).toBe(NO_PANEL_SECTIONS);
    expect(PREF_KEYS).toContain('panelSections');
  });

  it('viaja en el paquete de preferencias', () => {
    const prefs = { ...useStore.getState().prefs, panelSections: { 'machine.macros': true } };
    const bundle = buildBundle(prefs, {});
    expect(bundle.prefs.panelSections).toEqual({ 'machine.macros': true });
    const out = applyBundle(bundle, { prefs: useStore.getState().prefs, shortcuts: {} }, new Set());
    expect(out.prefs.panelSections).toEqual({ 'machine.macros': true });
  });
});

describe('panel Máquina: secciones por omisión', () => {
  it('abiertas solo Trabajo y Mover cabezal (y la conexión, completa, mientras no hay máquina)', async () => {
    await mount(<MachinePanel />);
    const order = [...document.querySelectorAll<HTMLElement>('[data-section]:not(.psec--nested)')].map((e) => e.dataset.section);
    // Trabajo arriba, luego Mover cabezal, luego el resto.
    expect(order.indexOf('machine.job')).toBeLessThan(order.indexOf('machine.jog'));
    expect(order.indexOf('machine.jog')).toBeLessThan(order.indexOf('machine.macros') === -1 ? order.length : order.indexOf('machine.macros'));
    expect(order.indexOf('machine.jog')).toBeLessThan(order.indexOf('machine.overrides'));
    expect(openIds().filter((id) => !id!.startsWith('machine.job.') && id !== 'machine.status.detail')).toEqual(['machine.connection', 'machine.job', 'machine.jog']);
    for (const id of ['machine.positions', 'machine.overrides', 'machine.finish', 'machine.fire', 'machine.moveSettings', 'machine.settings', 'machine.job.startFrom', 'machine.job.selection', 'machine.job.frame']) {
      expect(sectionOf(id), id).not.toBeNull();
      expect(isOpen(id), id).toBe(false);
    }
  });

  it('el panel entero pasa axe (sin conexión)', async () => {
    const m = await mount(<MachinePanel />);
    expect(describeViolations(await audit(m.host))).toEqual([]);
  });

  it('la vista en vivo y los botones del trabajo están a la vista sin desplegar nada', async () => {
    await mount(<MachinePanel />);
    const job = sectionOf('machine.job')!;
    expect(job.querySelector('.psec__body')!.hasAttribute('hidden')).toBe(false);
    expect(job.textContent).toContain('Vista en vivo');
    expect(job.textContent).toContain('Pausar');
  });
});

describe('conexión compacta', () => {
  const connect = () => useStore.setState({ connection: { connected: true, port: 'virtual:falcon2', error: null } });

  it('sin máquina conectada se muestra completa y sin plegado', async () => {
    await mount(<MachinePanel />);
    expect(toggleOf('machine.connection')).toBeNull();
    expect(document.getElementById('sel-port')).not.toBeNull();
    expect(sectionOf('machine.connection')!.querySelector('.psec__body')!.hasAttribute('hidden')).toBe(false);
  });

  it('conectada: una línea resumen con la máquina, el puerto y «Desconectar»; el detalle queda plegado', async () => {
    connect();
    await mount(<MachinePanel />);
    const sec = sectionOf('machine.connection')!;
    const line = sec.querySelector('.mc-connline')!;
    expect(line.textContent).toContain(FALLBACK_DEVICE.name);
    expect(line.textContent).toContain('Conectada en');
    expect(line.querySelector('button')!.textContent).toContain('Desconectar');
    expect(isOpen('machine.connection')).toBe(false);
    expect(sec.querySelector('.psec__body')!.hasAttribute('hidden')).toBe(true);
    // Solo hay un botón de conectar/desconectar.
    expect([...sec.querySelectorAll('button')].filter((b) => /conectar/i.test(b.textContent ?? '')).length).toBe(1);
    // El detalle (máquina, puerto, baudios) se despliega con el encabezado.
    await click(toggleOf('machine.connection')!);
    expect(sec.querySelector('.psec__body')!.hasAttribute('hidden')).toBe(false);
    expect(document.getElementById('sel-device')).not.toBeNull();
    expect(document.getElementById('sel-port')).not.toBeNull();
  });

  it('conectada: el panel entero pasa axe', async () => {
    connect();
    const m = await mount(<MachinePanel />);
    expect(describeViolations(await audit(m.host))).toEqual([]);
  });

  it('el estado es una franja: chip, X/Y de trabajo y F y S; la posición de máquina está en el detalle', async () => {
    connect();
    useStore.setState({ status: { state: 'idle', machine_pos: [10, 20], work_pos: [1.5, 2.5], feed: 1200, spindle: 300 } as never });
    await mount(<MachinePanel />);
    const strip = document.querySelector('.mc-strip')!;
    expect(strip.querySelector('.badge')).not.toBeNull();
    expect(strip.querySelector('.mc-strip__pos')!.textContent).toContain('1,50');
    expect(strip.querySelector('.mc-strip__kv')!.textContent).toContain('1200');
    const detail = sectionOf('machine.status.detail')!;
    expect(detail.querySelector('.psec__body')!.hasAttribute('hidden')).toBe(true);
    expect(detail.textContent).toContain('10,00');
  });
});

describe('abrir una sección desde otro lugar', () => {
  it('revealPanelSection abre la sección, muestra la pestaña, enfoca el encabezado y no mueve la ventana', async () => {
    await mount(<MachinePanel />);
    expect(isOpen('machine.positions')).toBe(false);
    const before = [window.scrollX, window.scrollY];
    revealPanelSection('machine.positions');
    await click(document.body);
    await new Promise((r) => requestAnimationFrame(() => r(null)));
    expect(isOpen('machine.positions')).toBe(true);
    expect(useStore.getState().dockTab).toBe('machine');
    expect(document.activeElement).toBe(toggleOf('machine.positions'));
    expect([window.scrollX, window.scrollY]).toEqual(before);
  });

  it('una sección anidada abre también la que la contiene', async () => {
    await click(document.body);
    resetPanelSections();
    useStore.getState().setPref('panelSections', { 'machine.job': false });
    await mount(<MachinePanel />);
    expect(isOpen('machine.job')).toBe(false);
    revealPanelSection('machine.job.startFrom');
    await click(document.body);
    expect(isOpen('machine.job')).toBe(true);
    expect(isOpen('machine.job.startFrom')).toBe(true);
  });

  it('pedida antes de que exista la sección, se muestra cuando aparece', async () => {
    revealPanelSection('machine.macros', { focus: false });
    expect(useStore.getState().prefs.panelSections['machine.macros']).toBe(true);
    useStore.setState({ devices: [{ ...FALLBACK_DEVICE, macros: [{ name: 'Home', gcode: 'G0 X0 Y0', confirm: false }] }] });
    await mount(<MachinePanel />);
    expect(isOpen('machine.macros')).toBe(true);
  });

  it('el comando «Ir a la vista en vivo» abre y muestra Trabajo', async () => {
    const { ALL_COMMANDS } = await import('@/commands');
    useStore.getState().setPref('panelSections', { 'machine.job': false });
    await mount(<MachinePanel />);
    expect(isOpen('machine.job')).toBe(false);
    const cmd = ALL_COMMANDS.find((c) => c.id === 'machine.showLive')!;
    await click(document.body);
    cmd.run(useStore.getState());
    await click(document.body);
    expect(isOpen('machine.job')).toBe(true);
  });
});
