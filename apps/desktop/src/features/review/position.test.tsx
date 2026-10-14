// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { setLanguage } from '@/i18n';
import { audit, cleanup, describeViolations, mount, press } from '@/test/dom';
import { clampPos, defaultPos, PANEL_W } from './position';
import { ReviewPanel } from './ReviewPanel';
import { useReviewUi } from './store';

const WIN = { w: 1440, h: 900 };

describe('posición de «Revisar diseño»', () => {
  it('abre pegado a la derecha y debajo de la barra de pestañas', () => {
    const p = defaultPos(WIN, 120);
    expect(p.y).toBeGreaterThanOrEqual(120);
    expect(p.x + PANEL_W).toBeLessThanOrEqual(WIN.w);
    expect(p.x).toBeGreaterThan(WIN.w / 2);
  });

  it('sin barra de pestañas a la vista queda bajo la barra superior', () => {
    expect(defaultPos(WIN, null).y).toBeGreaterThanOrEqual(44);
  });

  it('no sale de la ventana ni se mete bajo la barra superior o la de estado', () => {
    expect(clampPos({ x: -50, y: -50 }, WIN)).toEqual({ x: 0, y: 44 });
    const far = clampPos({ x: 5000, y: 5000 }, WIN);
    expect(far.x).toBe(WIN.w - PANEL_W);
    expect(far.y).toBeLessThan(WIN.h - 26);
  });

  it('en una ventana angosta el panel se encoge y cabe', () => {
    const p = defaultPos({ w: 360, h: 700 }, 100);
    expect(p.x).toBeGreaterThanOrEqual(0);
    expect(p.x + (360 - 32)).toBeLessThanOrEqual(360);
  });
});

describe('panel «Revisar diseño» montado', () => {
  beforeEach(() => {
    setApi(createMockApi({ latencyMs: 0 }));
    setLanguage('es');
    useReviewUi.setState({ open: true, ignored: [], showIgnored: false, busy: false, pos: null });
  });
  afterEach(cleanup);

  it('abre debajo de la barra de pestañas del dock y las flechas lo mueven', async () => {
    const dock = document.createElement('div');
    dock.className = 'dock';
    const tabs = document.createElement('div');
    tabs.setAttribute('role', 'tablist');
    tabs.getBoundingClientRect = () => ({ x: 0, y: 44, left: 0, top: 44, right: 300, bottom: 140, width: 300, height: 96, toJSON: () => ({}) });
    dock.append(tabs);
    document.body.append(dock);

    await mount(<ReviewPanel />);
    const panel = document.querySelector<HTMLElement>('.rv-panel')!;
    const top = parseFloat(panel.style.top);
    expect(top).toBeGreaterThanOrEqual(140);

    const head = document.querySelector<HTMLElement>('.rv-head')!;
    head.focus();
    await press('ArrowDown');
    expect(parseFloat(panel.style.top)).toBe(top + 16);
    expect(useReviewUi.getState().pos?.y).toBe(top + 16);
    dock.remove();
  });

  it('no tiene violaciones de accesibilidad', async () => {
    await mount(<ReviewPanel />);
    const r = await audit();
    expect(describeViolations(r)).toEqual([]);
  });
});
