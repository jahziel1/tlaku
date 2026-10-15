import { describe, expect, it } from 'vitest';
import { DEFAULT_PREFS, sanitizePrefs } from '@/store/prefs';
import { defaultLiveRect, fitLiveRect, LIVE_MARGIN, LIVE_MIN_H, LIVE_MIN_W, maxLiveRect, resizeLiveRect, sanitizeLiveWindow } from './liveWindow';

describe('sanitizeLiveWindow', () => {
  it('acepta lo válido y lo acota a los límites', () => {
    expect(sanitizeLiveWindow({ x: 10.4, y: 20.6, w: 500, h: 400, max: true })).toEqual({ x: 10, y: 21, w: 500, h: 400, max: true });
    expect(sanitizeLiveWindow({ x: -5, y: 1e9, w: 10, h: 99999 })).toEqual({ x: 0, y: 20000, w: LIVE_MIN_W, h: 2400, max: false });
  });
  it('lo que no sirve da null', () => {
    for (const bad of [null, undefined, 3, 'x', {}, { x: 1, y: 1, w: 'a', h: 2 }, { x: NaN, y: 1, w: 300, h: 300 }]) expect(sanitizeLiveWindow(bad)).toBeNull();
  });
  it('es una preferencia: de fábrica null; lo inválido vuelve a null y lo válido se conserva', () => {
    expect(DEFAULT_PREFS.liveWindow).toBeNull();
    expect(sanitizePrefs({ liveWindow: 'x' }).liveWindow).toBeNull();
    expect(sanitizePrefs({ liveWindow: { x: 1, y: 2, w: 400, h: 300, max: false } }).liveWindow).toEqual({ x: 1, y: 2, w: 400, h: 300, max: false });
  });
});

describe('fitLiveRect', () => {
  const bounds = { w: 1000, h: 700 };
  it('deja entero dentro de la ventana', () => {
    expect(fitLiveRect({ x: 900, y: 650, w: 400, h: 300 }, bounds)).toEqual({ x: 600, y: 400, w: 400, h: 300 });
    expect(fitLiveRect({ x: -50, y: -9, w: 400, h: 300 }, bounds)).toEqual({ x: 0, y: 0, w: 400, h: 300 });
  });
  it('si la ventana se achica, también el rectángulo, sin bajar del mínimo', () => {
    expect(fitLiveRect({ x: 600, y: 400, w: 800, h: 600 }, { w: 500, h: 400 })).toEqual({ x: 0, y: 0, w: 500, h: 400 });
    const tiny = fitLiveRect({ x: 50, y: 50, w: 400, h: 300 }, { w: 100, h: 100 });
    expect(tiny.w).toBe(LIVE_MIN_W);
    expect(tiny.h).toBe(LIVE_MIN_H);
    expect(tiny.x).toBe(0);
  });
});

describe('resizeLiveRect', () => {
  const bounds = { w: 1000, h: 700 };
  it('crece y encoge desde la esquina, entre el mínimo y el borde de la ventana', () => {
    const from = { x: 100, y: 100, w: 400, h: 300 };
    expect(resizeLiveRect(from, 50, 20, bounds)).toEqual({ x: 100, y: 100, w: 450, h: 320 });
    expect(resizeLiveRect(from, -999, -999, bounds)).toMatchObject({ w: LIVE_MIN_W, h: LIVE_MIN_H });
    expect(resizeLiveRect(from, 5000, 5000, bounds)).toMatchObject({ w: 900, h: 600 });
  });
});

describe('defaultLiveRect y maxLiveRect', () => {
  const area = { x: 70, y: 120, w: 1000, h: 700 };
  it('la primera vez va en la esquina inferior derecha del área, con margen', () => {
    const r = defaultLiveRect(area);
    expect(r.x + r.w).toBe(area.x + area.w - LIVE_MARGIN);
    expect(r.y + r.h).toBe(area.y + area.h - LIVE_MARGIN);
  });
  it('en un área chica no se pasa de ella (salvo el mínimo)', () => {
    const r = defaultLiveRect({ x: 0, y: 0, w: 320, h: 260 });
    expect(r.w).toBeLessThanOrEqual(320);
    expect(r.h).toBeGreaterThanOrEqual(LIVE_MIN_H);
  });
  it('agrandada ocupa casi toda el área', () => {
    expect(maxLiveRect(area)).toEqual({ x: 82, y: 132, w: 976, h: 676 });
  });
});
