import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { IMAGE_PLACEHOLDER, imageUrl, setApi } from './index';
import { createMockApi } from './mock';

const g = globalThis as unknown as { window?: unknown };
const original = g.window;
beforeEach(() => {
  setApi(createMockApi({ latencyMs: 0 }), 'mock');
});
afterEach(() => {
  g.window = original;
});

describe('imageUrl', () => {
  it('en modo mock devuelve el marcador de posición (data: URI permitido por la CSP)', () => {
    expect(imageUrl('/Users/a/foto.png')).toBe(IMAGE_PLACEHOLDER);
    expect(IMAGE_PLACEHOLDER.startsWith('data:image/svg+xml')).toBe(true);
  });

  it('en modo tauri usa convertFileSrc (protocolo asset)', () => {
    const calls: [string, string][] = [];
    g.window = {
      __TAURI_INTERNALS__: {
        convertFileSrc: (p: string, protocol: string) => {
          calls.push([p, protocol]);
          return `${protocol}://localhost/${encodeURIComponent(p)}`;
        },
      },
    };
    setApi(createMockApi({ latencyMs: 0 }), 'tauri');
    expect(imageUrl('/Users/a/mi foto.png')).toBe('asset://localhost/%2FUsers%2Fa%2Fmi%20foto.png');
    expect(calls).toEqual([['/Users/a/mi foto.png', 'asset']]);
  });
});
