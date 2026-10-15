// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { StatusReport } from '@/api/types';
import { FALLBACK_DEVICE } from '@/lib/device';
import { setLanguage } from '@/i18n';
import { useStore } from '@/store';
import { getApi } from '@/api';
import { audit, cleanup, click, describeViolations, mount, press } from '@/test/dom';
import { fakeContext } from './fakeCanvas';
import { LiveWindow } from './LiveWindow';
import { useLiveWindow } from './detach';
import { LiveToggle } from './LiveToggle';
import { clearJobStart } from './capture';
import { buildLiveRoute, pointAt, posAtTime } from './route';
import { startLive, useLive } from './service';
import { squareCut } from './testRoutes';

const s = () => useStore.getState();
const preview = squareCut();
const route = buildLiveRoute(preview)!;
const at = (t: number): [number, number] => {
  const p = posAtTime(route, t);
  return pointAt(route, p.move, p.frac);
};
const status = (t: number): StatusReport => ({ state: 'run', machine_pos: at(t), work_pos: at(t), feed: 600, spindle: 0, buffer: { planner: 15, rx: 127 } });

let stop: () => void;
let area: HTMLElement;
let win: () => HTMLElement | null;
let ctxCalls: ReturnType<typeof fakeContext>['calls'];

beforeEach(() => {
  setLanguage('es');
  clearJobStart();
  const { ctx, calls } = fakeContext();
  ctxCalls = calls;
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(ctx as never);
  area = document.createElement('div');
  area.className = 'canvas-area';
  document.body.append(area);
  win = () => document.querySelector<HTMLElement>('[data-testid="live-window"]');
  useLiveWindow.setState({ detached: false });
  useStore.setState({
    devices: [{ ...FALLBACK_DEVICE, id: 'dev' }],
    activeDeviceId: 'dev',
    preview,
    previewStatus: 'ready',
    previewStale: false,
    viewport: { width: 800, height: 500 },
    zoom: 3,
    panX: 20,
    panY: 480,
    jobState: 'idle',
    status: null,
    progress: null,
    prefs: { ...s().prefs, liveView: false, liveWindow: null },
  });
  useLive.setState({ jobSeq: 0, route: 'none', source: null, fraction: 0, emitting: false, ended: null });
  stop = startLive(useStore);
});
afterEach(async () => {
  stop();
  await cleanup();
  vi.restoreAllMocks();
});

const waitFrames = () => act(async () => new Promise((r) => setTimeout(r, 60)));
const begin = async () => {
  await act(async () => s().onJob({ state: 'started', message: '', position_lost: false }));
};

describe('interruptor del panel', () => {
  it('apagado de fábrica; encenderlo guarda la preferencia y se puede apagar de nuevo', async () => {
    const m = await mount(<LiveToggle />);
    const box = m.host.querySelector<HTMLInputElement>('input[type="checkbox"]')!;
    expect(box.checked).toBe(false);
    expect(m.host.textContent).toContain('Vista en vivo');
    await click(box);
    expect(s().prefs.liveView).toBe(true);
    await click(box);
    expect(s().prefs.liveView).toBe(false);
  });

  it('el texto sale en español y en inglés (distintos)', async () => {
    setLanguage('en');
    const m = await mount(<LiveToggle />);
    expect(m.host.textContent).toContain('Live view');
    expect(m.host.textContent).not.toContain('Vista en vivo');
    setLanguage('es');
  });

  it('pasa la auditoría de accesibilidad', async () => {
    const m = await mount(<LiveToggle />);
    const res = await audit(m.host);
    expect(describeViolations(res)).toEqual([]);
  });
});

describe('ventana flotante de la vista en vivo', () => {
  it('apagada no monta nada, aunque haya un trabajo', async () => {
    await mount(<LiveWindow />);
    await waitFrames();
    await begin();
    expect(win()).toBeNull();
    expect(ctxCalls.drawImage).toBe(0);
  });

  it('encendida sin trabajo muestra la ventana con el aviso de iniciar un trabajo', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveWindow />);
    expect(win()).not.toBeNull();
    expect(win()!.textContent).toContain('Inicia un trabajo');
  });

  it('NO toca el lienzo del diseño: la ventana va aparte y el área del lienzo queda intacta', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveWindow />);
    await begin();
    await act(async () => s().onStatus(status(4)));
    await waitFrames();
    expect(area.childElementCount).toBe(0);
    expect(area.querySelector('canvas, .live-canvas, .live-foot')).toBeNull();
    expect(area.contains(win())).toBe(false);
    // Es un diálogo no modal con nombre.
    expect(win()!.getAttribute('role')).toBe('dialog');
    expect(win()!.getAttribute('aria-modal')).toBe('false');
    expect(win()!.getAttribute('aria-label')).toBe('Vista en vivo');
  });

  it('dibuja con un trabajo en curso y ofrece la alternativa textual con porcentaje y estado', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveWindow />);
    await begin();
    await waitFrames();
    for (const t of [3, 6, 9]) {
      await act(async () => s().onStatus(status(t)));
      await act(async () => new Promise((r) => setTimeout(r, 30)));
    }
    const canvas = win()!.querySelector<HTMLCanvasElement>('canvas.live-canvas')!;
    expect(canvas.getAttribute('role')).toBe('img');
    expect(canvas.getAttribute('aria-label')).toMatch(/^Vista en vivo del trabajo: \d+ %, en marcha\./);
    expect(canvas.getAttribute('aria-label')).toContain('posición real');
    expect(ctxCalls.drawImage).toBeGreaterThan(0);
    expect(ctxCalls.fillRect).toBeGreaterThan(0);
    // Porcentaje, láser, transcurrido y restante en la franja.
    const foot = win()!.querySelector('.live-foot')!;
    expect(foot.textContent).toMatch(/\d+ %/);
    expect(foot.textContent).toContain('Transcurrido');
    expect(foot.textContent).toContain('Restante');
    expect(foot.textContent).toMatch(/láser (en|a)/);
  });

  it('al terminar muestra 100 % y «terminado»', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveWindow />);
    await begin();
    await act(async () => s().onStatus(status(5)));
    await act(async () => s().onJob({ state: 'finished', message: '', position_lost: false }));
    await waitFrames();
    expect(win()!.querySelector('canvas.live-canvas')!.getAttribute('aria-label')).toMatch(/100 %, terminado/);
    expect(win()!.querySelector('.live-foot')!.textContent).toContain('100 %');
  });

  it('el botón «Cerrar» apaga la vista (la misma preferencia del interruptor)', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveWindow />);
    const btn = win()!.querySelector<HTMLButtonElement>('button[aria-label="Cerrar la vista en vivo"]')!;
    await click(btn);
    expect(s().prefs.liveView).toBe(false);
    expect(win()).toBeNull();
  });

  it('agrandar y restaurar: se recuerda en la preferencia y la ventana ocupa casi toda el área', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveWindow />);
    const small = win()!.style.width;
    await click(win()!.querySelector('button[aria-label="Agrandar"]')!);
    expect(s().prefs.liveWindow?.max).toBe(true);
    expect(win()!.dataset.maximized).toBe('true');
    expect(parseInt(win()!.style.width)).toBeGreaterThan(parseInt(small));
    expect(win()!.querySelector('.floatwin__grip')).toBeNull();
    await click(win()!.querySelector('button[aria-label="Restaurar tamaño"]')!);
    expect(s().prefs.liveWindow?.max).toBe(false);
    expect(win()!.style.width).toBe(small);
  });

  it('el teclado mueve la ventana (flechas) y cambia su tamaño (Mayús + flechas, o la esquina), y se recuerda', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true, liveWindow: { x: 100, y: 100, w: 400, h: 300, max: false } } });
    await mount(<LiveWindow />);
    const head = win()!.querySelector<HTMLElement>('.floatwin__head')!;
    await act(async () => head.focus());
    await press('ArrowRight');
    await press('ArrowDown');
    expect(s().prefs.liveWindow).toMatchObject({ x: 116, y: 116, w: 400, h: 300 });
    await press('ArrowRight', { shiftKey: true });
    expect(s().prefs.liveWindow).toMatchObject({ x: 116, y: 116, w: 416, h: 300 });
    const grip = win()!.querySelector<HTMLElement>('.floatwin__grip')!;
    await act(async () => grip.focus());
    await press('ArrowDown');
    expect(s().prefs.liveWindow).toMatchObject({ w: 416, h: 316 });
    expect(win()!.style.left).toBe('116px');
    expect(win()!.style.height).toBe('316px');
  });

  it('el tamaño mínimo se respeta y la ventana no se sale de la de la app', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true, liveWindow: { x: 100, y: 100, w: 290, h: 230, max: false } } });
    await mount(<LiveWindow />);
    const grip = win()!.querySelector<HTMLElement>('.floatwin__grip')!;
    await act(async () => grip.focus());
    for (let i = 0; i < 5; i++) await press('ArrowLeft');
    expect(s().prefs.liveWindow).toMatchObject({ w: 280, h: 230 });
    // Hacia la esquina de la ventana: nunca queda fuera.
    const head = win()!.querySelector<HTMLElement>('.floatwin__head')!;
    await act(async () => head.focus());
    for (let i = 0; i < 80; i++) await press('ArrowRight');
    const p = s().prefs.liveWindow!;
    expect(p.x + p.w).toBeLessThanOrEqual(window.innerWidth);
  });

  it('se reacomoda si la ventana de la app se achica', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true, liveWindow: { x: 900, y: 600, w: 400, h: 300, max: false } } });
    await mount(<LiveWindow />);
    const before = win()!.style.left;
    const old = window.innerWidth;
    Object.defineProperty(window, 'innerWidth', { value: 700, configurable: true });
    await act(async () => window.dispatchEvent(new Event('resize')));
    const left = parseInt(win()!.style.left);
    expect(left + parseInt(win()!.style.width)).toBeLessThanOrEqual(700);
    expect(win()!.style.left).not.toBe(before);
    Object.defineProperty(window, 'innerWidth', { value: old, configurable: true });
  });

  it('recuerda posición y tamaño: otra ventana nueva aparece donde se dejó', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true, liveWindow: { x: 37, y: 53, w: 333, h: 277, max: false } } });
    await mount(<LiveWindow />);
    expect(win()!.style.left).toBe('37px');
    expect(win()!.style.top).toBe('53px');
    expect(win()!.style.width).toBe('333px');
    expect(win()!.style.height).toBe('277px');
  });

  it('«Abrir en ventana aparte» oculta la flotante; al cerrarse la aparte vuelve', async () => {
    const open = vi.fn(async () => undefined);
    const api = getApi();
    vi.spyOn(api, 'liveWindowOpen').mockImplementation(open);
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveWindow />);
    await click(win()!.querySelector('button[aria-label="Abrir en ventana aparte"]')!);
    expect(open).toHaveBeenCalledTimes(1);
    expect(useLiveWindow.getState().detached).toBe(true);
    expect(win()).toBeNull();
    // La ventana aparte se cerró: vuelve la flotante porque la vista sigue encendida.
    await act(async () => useLiveWindow.setState({ detached: false }));
    expect(win()).not.toBeNull();
  });

  it('si la ventana aparte no se puede abrir, avisa y la flotante sigue', async () => {
    vi.spyOn(getApi(), 'liveWindowOpen').mockRejectedValue('sin ventanas');
    useStore.setState({ prefs: { ...s().prefs, liveView: true }, notices: [] });
    await mount(<LiveWindow />);
    await click(win()!.querySelector('button[aria-label="Abrir en ventana aparte"]')!);
    expect(win()).not.toBeNull();
    expect(s().notices.some((n) => n.level === 'error' && n.text.includes('sin ventanas'))).toBe(true);
  });

  it('la ventana pasa la auditoría de accesibilidad', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveWindow />);
    await begin();
    await act(async () => s().onStatus(status(5)));
    await waitFrames();
    const res = await audit(win()!);
    expect(describeViolations(res)).toEqual([]);
  });

  it('en inglés los textos y la alternativa textual están en inglés', async () => {
    setLanguage('en');
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveWindow />);
    await begin();
    await act(async () => s().onStatus(status(5)));
    await waitFrames();
    expect(win()!.querySelector('canvas.live-canvas')!.getAttribute('aria-label')).toMatch(/^Live view of the job: \d+%, running\./);
    expect(win()!.querySelector('.live-foot')!.textContent).toContain('Elapsed');
    expect(win()!.querySelector('button[aria-label="Open in a separate window"]')).not.toBeNull();
    expect(win()!.querySelector('button[aria-label="Close the live view"]')).not.toBeNull();
    setLanguage('es');
  });
});

describe('cama simulada: zoom y desplazamiento propios', () => {
  const view = () => win()!.querySelector<HTMLElement>('[data-testid="live-view"]')!;

  it('la rueda, + - 0 y las flechas no tocan el zoom ni el desplazamiento del lienzo del diseño', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveWindow />);
    await begin();
    await act(async () => s().onStatus(status(3)));
    const before = { zoom: s().zoom, panX: s().panX, panY: s().panY };
    const el = view();
    await act(async () => el.dispatchEvent(new WheelEvent('wheel', { deltaY: -240, bubbles: true, cancelable: true })));
    await act(async () => el.focus());
    await press('+');
    await press('ArrowLeft');
    await press('0');
    expect({ zoom: s().zoom, panX: s().panX, panY: s().panY }).toEqual(before);
  });

  it('las teclas de la cama no llegan a los atajos globales', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveWindow />);
    const seen = vi.fn();
    document.addEventListener('keydown', seen);
    const el = view();
    await act(async () => el.focus());
    await press('+');
    await press('ArrowUp');
    expect(seen).not.toHaveBeenCalled();
    await press('Tab');
    expect(seen).toHaveBeenCalledTimes(1);
    document.removeEventListener('keydown', seen);
  });

  it('el botón «Encajar la cama» existe y es operable', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveWindow />);
    const btn = win()!.querySelector<HTMLButtonElement>('button[aria-label="Encajar la cama"]')!;
    await click(btn);
    expect(btn.getAttribute('title')).toContain('Encaja');
  });
});
