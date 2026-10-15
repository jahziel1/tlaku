// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { createMockApi } from '@/api/mock';
import { mockLiveWindow } from '@/api/mock/live';
import { isLiveWindow, LIVE_WINDOW_LABEL } from './windowMode';

afterEach(() => {
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  window.history.replaceState(null, '', '/');
});

describe('¿es la ventana aparte?', () => {
  it('la ventana normal no lo es', () => {
    expect(isLiveWindow()).toBe(false);
  });
  it('en Tauri lo dice la etiqueta de la ventana', () => {
    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = { metadata: { currentWindow: { label: LIVE_WINDOW_LABEL } } };
    expect(isLiveWindow()).toBe(true);
    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = { metadata: { currentWindow: { label: 'doc-2' } } };
    expect(isLiveWindow()).toBe(false);
  });
  it('en el navegador (modo simulado) lo dice ?liveWindow=1', () => {
    window.history.replaceState(null, '', '/?liveWindow=1');
    expect(isLiveWindow()).toBe(true);
  });
  it('la etiqueta es la que cubre la capacidad `live` (live.json) y la que abre Rust', () => {
    expect(LIVE_WINDOW_LABEL).toBe('live');
  });
});

describe('ventana aparte en el modo simulado', () => {
  it('los eventos viajan por el canal entre pestañas; sin ventana abierta no hay a quién mandar', async () => {
    const owner = mockLiveWindow((v) => Promise.resolve(v()));
    const live = mockLiveWindow((v) => Promise.resolve(v()));
    expect(await owner.liveWindowSend({ kind: 'state', data: '{}' })).toBe(false);
    const got = new Promise<null>((ok) => live.onLive('live://ready', ok));
    await live.liveWindowHello();
    // Las dos instancias son «dueñas» en esta prueba (no hay `?liveWindow`): la otra recibe el aviso.
    await expect(Promise.race([got, new Promise((r) => setTimeout(() => r('nada'), 300))])).resolves.toBe('nada');
    // Pero el mensaje sí le llega a quien lo escucha como dueña.
    const ready = new Promise<null>((ok) => owner.onLive('live://ready', ok));
    await live.liveWindowHello();
    await expect(ready).resolves.toBeNull();
  });

  it('la API simulada expone los comandos y los eventos live://', async () => {
    const api = createMockApi({ latencyMs: 0 });
    expect(typeof api.liveWindowOpen).toBe('function');
    expect(typeof api.liveWindowClose).toBe('function');
    expect(typeof api.liveWindowSend).toBe('function');
    expect(typeof api.liveWindowHello).toBe('function');
    const off = await api.on('live://closed', () => undefined);
    off();
    expect(await api.liveWindowSend({ kind: 'route', data: '{}' })).toBe(false);
  });
});
