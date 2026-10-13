import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Point } from '@/api/types';
import { useShapeOps } from '@/features/arrange/shapeOpsUi';
import { createAppStore } from '@/store';
import { clearGesture, useInteraction } from './interaction';
import { createTrimTool, TRIM_PREVIEW_MS } from './trimTool';
import type { ToolDeps } from './toolDeps';

const run = vi.fn(async () => true);
const preview = vi.fn(async (): Promise<Point[] | null> => [[0, 0], [10, 0]]);
vi.mock('@/features/arrange/shapeOps', () => ({
  runTrimAt: (...a: unknown[]) => (run as (...x: unknown[]) => Promise<boolean>)(...a),
  previewTrimAt: (...a: unknown[]) => (preview as (...x: unknown[]) => Promise<Point[] | null>)(...a),
}));

let store: ReturnType<typeof createAppStore>;

function deps(): ToolDeps {
  return {
    el: {} as HTMLElement,
    st: () => store.getState(),
    viewOf: () => ({ zoom: 1, panX: 0, panY: 0 }),
    world: (sx, sy) => [sx, sy] as Point,
    capture: () => {},
    release: () => {},
    snapOpts: () => null,
    targetsFor: () => ({ xs: [], ys: [] }),
    setTip: () => {},
    snapTo: (p) => p,
  };
}

beforeEach(() => {
  vi.useFakeTimers();
  store = createAppStore();
  run.mockClear();
  preview.mockClear();
  clearGesture();
  useShapeOps.getState().setLastPoint(null);
});
afterEach(() => vi.useRealTimers());

describe('herramienta Recortar formas', () => {
  it('un clic recorta en ese punto, lo recuerda como último clic y Mayús conserva el tramo', () => {
    store.getState().select(['a', 'b']);
    const tool = createTrimTool(deps());
    tool.down(12, 34, false);
    const maxDist = 8 / store.getState().zoom;
    expect(run).toHaveBeenCalledWith(store.getState(), [12, 34], { maxDist });
    expect(useShapeOps.getState().lastPoint).toEqual([12, 34]);
    tool.down(1, 2, true);
    expect(run).toHaveBeenLastCalledWith(store.getState(), [1, 2], { maxDist, keepRemoved: true });
  });

  it('sin selección no recorta y avisa qué hacer', () => {
    const tool = createTrimTool(deps());
    tool.down(5, 5, false);
    expect(run).not.toHaveBeenCalled();
    expect(store.getState().notices.at(-1)?.text).toMatch(/Selecciona/);
    tool.hover(5, 5);
    expect(useInteraction.getState().tip?.text).toMatch(/Selecciona/);
  });

  it('al pasar el puntero resalta el tramo que se borraría, tras una pausa', async () => {
    store.getState().select(['a']);
    const tool = createTrimTool(deps());
    tool.hover(3, 3);
    tool.hover(4, 4);
    expect(preview).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(TRIM_PREVIEW_MS + 5);
    expect(preview).toHaveBeenCalledTimes(1);
    expect(preview).toHaveBeenCalledWith(store.getState(), [4, 4], 8 / store.getState().zoom);
    expect(useInteraction.getState().draft?.paths).toHaveLength(1);
  });

  it('si no hay trazo cerca el resaltado se quita, y salir o cancelar lo limpia', async () => {
    store.getState().select(['a']);
    const tool = createTrimTool(deps());
    tool.hover(3, 3);
    await vi.advanceTimersByTimeAsync(TRIM_PREVIEW_MS + 5);
    expect(useInteraction.getState().draft).not.toBeNull();
    preview.mockResolvedValueOnce(null);
    tool.hover(500, 500);
    await vi.advanceTimersByTimeAsync(TRIM_PREVIEW_MS + 5);
    expect(useInteraction.getState().draft).toBeNull();
    tool.hover(3, 3);
    await vi.advanceTimersByTimeAsync(TRIM_PREVIEW_MS + 5);
    tool.leave();
    expect(useInteraction.getState().draft).toBeNull();
  });

  it('una respuesta que llega tarde (ya se movió el puntero) se descarta', async () => {
    store.getState().select(['a']);
    let release: (v: Point[] | null) => void = () => {};
    preview.mockImplementationOnce(() => new Promise<Point[] | null>((r) => (release = r)));
    const tool = createTrimTool(deps());
    tool.hover(3, 3);
    await vi.advanceTimersByTimeAsync(TRIM_PREVIEW_MS + 5);
    tool.cancel(); // el puntero salió o cambió la herramienta
    release([[0, 0], [9, 9]]);
    await vi.advanceTimersByTimeAsync(1);
    expect(useInteraction.getState().draft).toBeNull();
  });
});
