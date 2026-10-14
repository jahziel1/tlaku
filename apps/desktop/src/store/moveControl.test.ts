import { afterEach, describe, expect, it, vi } from 'vitest';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { setApi } from '@/api';
import { selectActiveDevice } from './selectors';
import { bindMachineEvents, createAppStore } from './index';
import { FRAME_DONE_LOG } from './moveControl';

const memory = new Map<string, string>();

async function setup(patch: Partial<ReturnType<typeof selectActiveDevice>> = {}) {
  memory.clear();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => memory.get(k) ?? null,
    setItem: (k: string, v: string) => void memory.set(k, v),
  });
  const api: MockApi = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 400, jobSpeedup: 1 });
  setApi(api);
  const store = createAppStore();
  await store.getState().loadDevices();
  await store.getState().saveDevice({ ...selectActiveDevice(store.getState()), z_axis: true, u_axis: true, ...patch });
  await bindMachineEvents(api, store);
  await store.getState().refreshPorts();
  await store.getState().connect();
  await vi.waitFor(() => expect(store.getState().status).not.toBeNull());
  store.getState().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });
  return { api, store };
}

const sent = (store: Awaited<ReturnType<typeof setup>>['store']) => store.getState().log.filter((l) => l.dir === 'out').map((l) => l.text);

afterEach(() => vi.unstubAllGlobals());

describe('ejes extra y enfoque', () => {
  it('Z sube y baja un paso; U usa la letra elegida', async () => {
    const { store } = await setup();
    store.getState().setJogStep(1);
    await store.getState().jogExtra('z', 1);
    await store.getState().jogExtra('z', -1);
    store.getState().setULetter('A');
    await store.getState().jogExtra('u', 1);
    const out = sent(store);
    expect(out.some((l) => /^\$J=G91 G21 Z1 /.test(l))).toBe(true);
    expect(out.some((l) => /^\$J=G91 G21 Z-1 /.test(l))).toBe(true);
    expect(out.some((l) => /^\$J=G91 G21 A1 /.test(l))).toBe(true);
  });

  it('con z_invert el sentido de Z se invierte; sin el eje en el perfil avisa y no envía', async () => {
    const inv = await setup({ z_invert: true });
    inv.store.getState().setJogStep(2);
    await inv.store.getState().jogExtra('z', 1);
    expect(sent(inv.store).some((l) => /Z-2 /.test(l))).toBe(true);
    const none = await setup({ z_axis: false });
    await none.store.getState().jogExtra('z', 1);
    expect(sent(none.store).some((l) => l.includes(' Z'))).toBe(false);
  });

  it('Enfocar lleva Z a la altura guardada; el valor se recuerda', async () => {
    const { store } = await setup();
    store.getState().setFocusZ(12.5);
    expect(memory.get('tlaku.focusZ')).toBe('12.5');
    await store.getState().goFocus();
    expect(sent(store).some((l) => /^\$J=G90 G21 Z12\.5 /.test(l))).toBe(true);
  });

  it('el jog extra está bloqueado durante un trabajo', async () => {
    const { store } = await setup();
    await store.getState().startJob();
    await vi.waitFor(() => expect(store.getState().jobState).toBe('running'));
    await store.getState().jogExtra('z', 1);
    expect(sent(store).some((l) => / Z\d/.test(l))).toBe(false);
    await store.getState().cancelJob();
  });
});

describe('disparo de prueba', () => {
  it('manda M3, pausa y M5, y limita la potencia y la duración', async () => {
    const { store } = await setup();
    store.getState().setFire({ powerPct: 90, durationMs: 99999 });
    expect(store.getState().fire).toEqual({ powerPct: 20, durationMs: 2000 });
    store.getState().setFire({ powerPct: 2, durationMs: 300 });
    expect(await store.getState().fireTest()).toBe(true);
    const out = sent(store);
    const i = out.findIndex((l) => l.startsWith('M3'));
    expect(out.slice(i, i + 3)).toEqual([expect.stringMatching(/^M3 S/), 'G4 P0.3', 'M5']);
    expect(store.getState().firing).toBe(false);
  });

  it('si el motor lo rechaza, igual manda M5 y avisa; no dispara durante un trabajo', async () => {
    const { api, store } = await setup();
    const spy = vi.spyOn(api, 'fireTest').mockRejectedValue(new Error('error:20'));
    expect(await store.getState().fireTest()).toBe(false);
    expect(sent(store).at(-1)).toBe('M5');
    expect(store.getState().firing).toBe(false);
    spy.mockRestore();
    await store.getState().startJob();
    await vi.waitFor(() => expect(store.getState().jobState).toBe('running'));
    expect(await store.getState().fireTest()).toBe(false);
    await store.getState().cancelJob();
  });
});

describe('posiciones guardadas', () => {
  it('guarda la actual en el perfil, la reemplaza por nombre y va a ella', async () => {
    const { store } = await setup();
    await store.getState().jog(10, 5);
    await vi.waitFor(() => expect(store.getState().status?.work_pos[0]).toBeGreaterThan(5));
    expect(await store.getState().savePosition('  Esquina ')).toBe(true);
    const pos = selectActiveDevice(store.getState()).positions;
    expect(pos).toHaveLength(1);
    expect(pos[0]!.name).toBe('Esquina');
    expect(await store.getState().savePosition('Esquina')).toBe(true);
    expect(selectActiveDevice(store.getState()).positions).toHaveLength(1);
    await store.getState().jog(30, 30);
    await store.getState().goToPosition('Esquina');
    expect(sent(store).some((l) => l.startsWith('$J=G90 G21 X'))).toBe(true);
    await store.getState().deletePosition('Esquina');
    expect(selectActiveDevice(store.getState()).positions).toHaveLength(0);
    expect(await store.getState().savePosition('   ')).toBe(false);
  });
});

describe('macros', () => {
  const macro = (gcode: string, confirm = false) => ({ name: 'M', gcode, confirm });

  it('una macro de solo lectura sale directo, línea por línea', async () => {
    const { store } = await setup({ macros: [macro('$I\n$G')] });
    await store.getState().runMacro(0);
    expect(store.getState().dialog).toBeNull();
    expect(sent(store).filter((l) => l === '$I' || l === '$G')).toHaveLength(2);
  });

  it('si mueve la máquina o está marcada, pide confirmación y no envía nada antes', async () => {
    const { store } = await setup({ macros: [macro('G91\nG0 X5'), macro('$I', true)] });
    await store.getState().runMacro(0);
    expect(store.getState().dialog).toEqual({ kind: 'macro', index: 0 });
    await store.getState().runMacro(1);
    expect(store.getState().dialog).toEqual({ kind: 'macro', index: 1 });
    expect(sent(store).some((l) => l.startsWith('G0'))).toBe(false);
    expect(await store.getState().sendMacro(0)).toBe(true);
    expect(sent(store)).toContain('G0X5');
  });

  it('se detiene en la primera línea que la máquina rechaza', async () => {
    const { store } = await setup({ macros: [macro('$I\nZZ\n$G', true)] });
    expect(await store.getState().sendMacro(0)).toBe(false);
    const out = sent(store);
    expect(out).toContain('ZZ');
    expect(out).not.toContain('$G');
  });

  it('nunca durante un trabajo ni sin conexión; una macro inválida no se envía', async () => {
    const { store } = await setup({ macros: [macro('$I'), macro('hold'), macro('G0 X1 ?')] });
    await store.getState().startJob();
    await vi.waitFor(() => expect(store.getState().jobState).toBe('running'));
    expect(await store.getState().sendMacro(0)).toBe(false);
    await store.getState().cancelJob();
    await vi.waitFor(() => expect(store.getState().jobState).not.toBe('running'));
    expect(await store.getState().sendMacro(1)).toBe(false);
    expect(await store.getState().sendMacro(2)).toBe(false);
    expect(sent(store)).not.toContain('HOLD');
  });
});

describe('encuadre', () => {
  it('con el láser encendido envía la potencia; apagado, no envía el campo', async () => {
    const { api, store } = await setup();
    const spy = vi.spyOn(api, 'frameJob');
    await store.getState().frameJob();
    expect(spy.mock.calls[0]![0]).not.toHaveProperty('power_pct');
    store.getState().setFrameLaser({ on: true, powerPct: 3 });
    await store.getState().frameJob();
    expect(spy.mock.calls[1]![0]).toMatchObject({ power_pct: 3 });
    store.getState().setFrameLaser({ powerPct: 99 });
    expect(store.getState().frameLaser.powerPct).toBe(20);
  });

  it('el bucle repite al terminar cada vuelta y se detiene con Detener', async () => {
    const { api, store } = await setup();
    const spy = vi.spyOn(api, 'frameJob');
    store.getState().setFrameLoop(true);
    await store.getState().frameJob();
    expect(store.getState().frameLooping).toBe(true);
    store.getState().noteFrameLog(FRAME_DONE_LOG);
    await vi.waitFor(() => expect(spy).toHaveBeenCalledTimes(2), { timeout: 2000 });
    await store.getState().cancelJob();
    expect(store.getState().frameLooping).toBe(false);
    store.getState().noteFrameLog(FRAME_DONE_LOG);
    await new Promise((r) => setTimeout(r, 600));
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('sin el bucle activado una vuelta no se repite', async () => {
    const { api, store } = await setup();
    const spy = vi.spyOn(api, 'frameJob');
    await store.getState().frameJob();
    store.getState().noteFrameLog(FRAME_DONE_LOG);
    await new Promise((r) => setTimeout(r, 600));
    expect(spy).toHaveBeenCalledTimes(1);
  });
});

describe('líneas del registro con código del motor', () => {
  it('el bucle del encuadre se guía por el código, también con el motor en inglés', async () => {
    const { api, store } = await setup();
    const spy = vi.spyOn(api, 'frameJob');
    store.getState().setFrameLoop(true);
    await store.getState().frameJob();
    expect(store.getState().frameLooping).toBe(true);
    store.getState().noteFrameLog('[frame_done] ; frame finished');
    await vi.waitFor(() => expect(spy).toHaveBeenCalledTimes(2), { timeout: 2000 });
    // Cancelado (en inglés): el bucle se detiene.
    store.getState().noteFrameLog('[frame_cancelled] ; frame cancelled');
    expect(store.getState().frameLooping).toBe(false);
  });

  it('el texto sin código no cuenta', async () => {
    const { api, store } = await setup();
    const spy = vi.spyOn(api, 'frameJob');
    store.getState().setFrameLoop(true);
    await store.getState().frameJob();
    store.getState().noteFrameLog('; frame finished');
    store.getState().noteFrameLog('; encuadre terminado');
    await new Promise((r) => setTimeout(r, 600));
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it('la consola muestra la línea sin el código', async () => {
    const { api, store } = await setup();
    store.getState().setFrameLoop(true);
    await store.getState().frameJob();
    store.getState().onLog({ dir: 'in', text: '[frame_done] ; frame finished' });
    expect(store.getState().log.at(-1)?.text).toBe('; frame finished');
    void api;
  });
});
