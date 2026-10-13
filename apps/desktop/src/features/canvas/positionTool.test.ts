import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Point } from '@/api/types';
import { createAppStore } from '@/store';
import { clearGesture, useInteraction } from './interaction';
import { createPositionTool } from './positionTool';
import type { Mods, ToolDeps } from './toolDeps';

const NONE: Mods = { shift: false, alt: false, ctrl: false };
const ev = () => ({ pointerId: 1 }) as PointerEvent;

let store: ReturnType<typeof createAppStore>;
let tips: string[];
let calls: [number, number][];

function deps(): ToolDeps {
  return {
    el: {} as HTMLElement,
    st: () => store.getState(),
    viewOf: () => ({ zoom: 1, panX: 0, panY: 0 }),
    world: (sx, sy) => [sx / 2, 100 - sy] as Point, // un mapa cualquiera con Y hacia arriba
    capture: () => {},
    release: () => {},
    snapOpts: () => null,
    targetsFor: () => ({ xs: [], ys: [] }),
    setTip: (_x, _y, text) => void tips.push(text),
    snapTo: (p) => p,
  };
}

beforeEach(() => {
  store = createAppStore();
  tips = [];
  calls = [];
  clearGesture();
  store.setState({ positionLaserAt: vi.fn(async (x: number, y: number) => void calls.push([x, y])) });
});

describe('modo Posicionar el láser en el lienzo', () => {
  it('apagado no consume el clic ni el movimiento', () => {
    const tool = createPositionTool(deps());
    expect(tool.active()).toBe(false);
    expect(tool.down(ev(), 10, 10, NONE)).toBe(false);
    expect(tool.move(ev(), 10, 10, NONE)).toBe(false);
    expect(calls).toEqual([]);
  });

  it('cada clic llama a positionLaserAt con el punto en mm y no deja gesto ni selección', () => {
    store.setState({ positioningLaser: true });
    store.getState().select(['a']);
    const tool = createPositionTool(deps());
    expect(tool.down(ev(), 40, 30, NONE)).toBe(true);
    expect(tool.down(ev(), 60, 90, NONE)).toBe(true);
    expect(calls).toEqual([[20, 70], [30, 10]]);
    expect(useInteraction.getState().gesture).toBeNull();
    expect(store.getState().selectedIds).toEqual(['a']);
  });

  it('al mover deja el cursor de mira y muestra el punto', () => {
    store.setState({ positioningLaser: true });
    const tool = createPositionTool(deps());
    expect(tool.move(ev(), 40, 30, NONE)).toBe(true);
    expect(useInteraction.getState().cursor).toBe('crosshair');
    expect(tips.at(-1)).toContain('20');
    expect(tips.at(-1)).toContain('70');
  });
});
