import type { Canvas2D } from './layers';

/** Lienzo de mentira que cuenta las llamadas de dibujo (jsdom no trae canvas). Solo para pruebas. */
export interface Calls {
  beginPath: number;
  stroke: number;
  moveTo: number;
  lineTo: number;
  clearRect: number;
  drawImage: number;
  fillRect: number;
  arc: number;
  fill: number;
  styles: string[];
  /** Último punto de cada `lineTo`. */
  points: [number, number][];
}

export function fakeContext(): { ctx: CanvasRenderingContext2D; calls: Calls } {
  const calls: Calls = { beginPath: 0, stroke: 0, moveTo: 0, lineTo: 0, clearRect: 0, drawImage: 0, fillRect: 0, arc: 0, fill: 0, styles: [], points: [] };
  const state: Record<string, unknown> = {};
  const count = (k: keyof Omit<Calls, 'styles' | 'points'>) => () => void (calls[k] += 1);
  const ctx = new Proxy(state, {
    get(target, prop: string) {
      if (prop in target) return target[prop];
      switch (prop) {
        case 'beginPath': return count('beginPath');
        case 'stroke': return () => void (calls.stroke += 1, calls.styles.push(String(target.strokeStyle)));
        case 'moveTo': return () => void (calls.moveTo += 1);
        case 'lineTo': return (x: number, y: number) => void (calls.lineTo += 1, calls.points.push([x, y]));
        case 'clearRect': return count('clearRect');
        case 'drawImage': return count('drawImage');
        case 'fillRect': return count('fillRect');
        case 'arc': return count('arc');
        case 'fill': return count('fill');
        case 'createRadialGradient': return () => ({ addColorStop() {} });
        default: return () => undefined;
      }
    },
    set(target, prop: string, value) {
      target[prop] = value;
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
  return { ctx, calls };
}

export interface FakeCanvas extends Canvas2D {
  calls: Calls;
}

export function fakeCanvasFactory(made: FakeCanvas[] = []): () => Canvas2D {
  return () => {
    const { ctx, calls } = fakeContext();
    const c: FakeCanvas = { width: 0, height: 0, calls, getContext: () => ctx };
    made.push(c);
    return c;
  };
}
