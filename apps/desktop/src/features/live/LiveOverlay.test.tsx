// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { StatusReport } from '@/api/types';
import { FALLBACK_DEVICE } from '@/lib/device';
import { setLanguage } from '@/i18n';
import { useStore } from '@/store';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { fakeContext } from './fakeCanvas';
import { LiveOverlay } from './LiveOverlay';
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
    prefs: { ...s().prefs, liveView: false },
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

describe('lienzo de la vista en vivo', () => {
  it('apagada no monta nada, aunque haya un trabajo', async () => {
    await mount(<LiveOverlay />);
    await waitFrames();
    await begin();
    expect(area.querySelector('.live-canvas, .live-hud')).toBeNull();
    expect(ctxCalls.drawImage).toBe(0);
  });

  it('encendida con un trabajo en curso dibuja y ofrece la alternativa textual con porcentaje y estado', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveOverlay />);
    await waitFrames();
    expect(area.querySelector('.live-canvas')).toBeNull(); // todavía no hay trabajo
    await begin();
    await waitFrames();
    // Ya hay HUD con la ruta lista.
    expect(area.querySelector('.live-hud')).not.toBeNull();
    for (const t of [3, 6, 9]) {
      await act(async () => s().onStatus(status(t)));
      await act(async () => new Promise((r) => setTimeout(r, 30)));
    }
    const canvas = area.querySelector<HTMLCanvasElement>('canvas.live-canvas')!;
    expect(canvas.getAttribute('role')).toBe('img');
    expect(canvas.getAttribute('aria-label')).toMatch(/^Vista en vivo del trabajo: \d+ %, en marcha\./);
    expect(canvas.getAttribute('aria-label')).toContain('posición real');
    expect(ctxCalls.drawImage).toBeGreaterThan(0);
    expect(ctxCalls.fillRect).toBeGreaterThan(0);
    // El tamaño del lienzo sigue al del área (px CSS).
    expect(canvas.style.width).toBe('800px');
    // Porcentaje, transcurrido y restante en el cuadro.
    const hud = area.querySelector('.live-hud')!;
    expect(hud.textContent).toMatch(/\d+ %/);
    expect(hud.textContent).toContain('Transcurrido');
    expect(hud.textContent).toContain('Restante');
  });

  it('«Volver al diseño» apaga la vista; el cuadro no tapa los controles del trabajo', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveOverlay />);
    await begin();
    await waitFrames();
    const btn = area.querySelector<HTMLButtonElement>('.live-hud button')!;
    expect(btn.getAttribute('aria-label')).toBe('Volver al diseño');
    await click(btn);
    expect(s().prefs.liveView).toBe(false);
    expect(area.querySelector('.live-hud')).toBeNull();
  });

  it('al terminar muestra 100 % y «terminado»', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveOverlay />);
    await begin();
    await act(async () => s().onStatus(status(5)));
    await act(async () => s().onJob({ state: 'finished', message: '', position_lost: false }));
    await waitFrames();
    const canvas = area.querySelector<HTMLCanvasElement>('canvas.live-canvas')!;
    expect(canvas.getAttribute('aria-label')).toMatch(/100 %, terminado/);
    expect(area.querySelector('.live-hud')!.textContent).toContain('100 %');
  });

  it('el cuadro pasa la auditoría de accesibilidad', async () => {
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveOverlay />);
    await begin();
    await act(async () => s().onStatus(status(5)));
    await waitFrames();
    const res = await audit(area);
    expect(describeViolations(res)).toEqual([]);
  });

  it('en inglés el cuadro y la alternativa textual están en inglés', async () => {
    setLanguage('en');
    useStore.setState({ prefs: { ...s().prefs, liveView: true } });
    await mount(<LiveOverlay />);
    await begin();
    await act(async () => s().onStatus(status(5)));
    await waitFrames();
    expect(area.querySelector('canvas.live-canvas')!.getAttribute('aria-label')).toMatch(/^Live view of the job: \d+%, running\./);
    expect(area.querySelector('.live-hud')!.textContent).toContain('Elapsed');
    setLanguage('es');
  });
});
