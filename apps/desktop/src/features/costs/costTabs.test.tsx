// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DesignObject, JobPreview } from '@/api/types';
import { commands } from '@/commands';
import { setLanguage } from '@/i18n';
import { polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { emptyDocument } from '@/store/document';
import { useStore } from '@/store';
import { cleanup, mount } from '@/test/dom';
import { resetTabs } from '@/test/tabs';
import { CostDialog } from './CostDialog';
import { resetCostStore, useCostStore } from './costStore';

/** La calculadora de costos usa el diseño y la vista previa de la pestaña ACTIVA y se actualiza al cambiar de pestaña. */

const RED = '#ff3b4a';
const rect = (id: string, w: number, h: number): DesignObject => ({
  id, layer: RED, transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null,
  kind: { type: 'vector', paths: [polylineBez([[0, 0], [w, 0], [w, h], [0, h]], true)] },
});
const preview = (totalS: number): JobPreview => ({
  layers: [{ color: RED, mode: 'line', seconds: totalS, summary: 'Rojo', travel_saved_pct: null, explanations: [] }],
  moves: [0, 0, 0, 0, 1, 100, 0, 80, 1, 100, 100, 80, 1, 0, 100, 80],
  move_times: [0, totalS / 3, (2 * totalS) / 3, totalS],
  total_s: totalS,
  bounds: [[0, 0], [100, 100]],
  order_note: null,
  warnings: [],
});
const docWith = (w: number, h: number) => ({ ...emptyDocument(), objects: [rect('a', w, h)], layers: [makeLayer(RED, 'Rojo')] });
const s = () => useStore.getState();

const memory = new Map<string, string>();
const dialog = () => document.querySelector<HTMLElement>('[role="dialog"]')!;
const cost = () => dialog().querySelector('[data-testid="cost-cost"]')!.textContent;

async function type(label: string, value: string) {
  const l = [...dialog().querySelectorAll('label')].find((x) => x.textContent === label)!;
  const input = dialog().querySelector<HTMLInputElement>(`#${CSS.escape(l.htmlFor)}`)!;
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, value);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await act(async () => {
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  });
}

let computePreview: ReturnType<typeof vi.fn<() => Promise<void>>>;
const original = useStore.getState().computePreview;

beforeEach(() => {
  memory.clear();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => memory.get(k) ?? null,
    setItem: (k: string, v: string) => void memory.set(k, v),
    removeItem: (k: string) => void memory.delete(k),
  });
  Object.defineProperty(navigator, 'language', { value: 'es-MX', configurable: true });
  setLanguage('es');
  resetCostStore();
  resetTabs();
  computePreview = vi.fn(async (): Promise<void> => undefined);
  useStore.setState({ computePreview });
});
afterEach(async () => {
  await cleanup();
  act(() => useCostStore.getState().closeDialog());
  useStore.setState({ computePreview: original });
  resetTabs();
  vi.unstubAllGlobals();
});

describe('costos y pestañas', () => {
  it('muestra el diseño y la vista previa de la pestaña activa y cambia al cambiar de pestaña', async () => {
    // Pestaña A: 1 h de máquina; pestaña B: 2 h.
    s().openInTab(docWith(100, 100), '/p/a.tlaku');
    const a = s().activeTabId;
    act(() => useStore.setState({ preview: preview(3600), previewStatus: 'ready', previewStale: false }));
    s().openInTab(docWith(200, 100), '/p/b.tlaku');
    const b = s().activeTabId;
    act(() => useStore.setState({ preview: preview(7200), previewStatus: 'ready', previewStale: false }));
    act(() => s().activateTab(a));

    await mount(<CostDialog />);
    await act(async () => commands.run('costs.open'));
    await type('Costo de la máquina por hora', '60');
    await type('Precio de la hoja', '0');
    expect(cost()).toBe('$60.00');
    expect(computePreview).not.toHaveBeenCalled(); // la vista previa de A estaba al día

    act(() => s().activateTab(b));
    expect(cost()).toBe('$120.00');
    act(() => s().activateTab(a));
    expect(cost()).toBe('$60.00');
  });

  it('al pasar a una pestaña sin vista previa al día, la calcula', async () => {
    s().openInTab(docWith(100, 100), '/p/a.tlaku');
    act(() => useStore.setState({ preview: preview(3600), previewStatus: 'ready', previewStale: false }));
    await mount(<CostDialog />);
    await act(async () => commands.run('costs.open'));
    expect(computePreview).not.toHaveBeenCalled();

    act(() => void s().openInTab(docWith(50, 50), '/p/b.tlaku')); // pestaña nueva: sin vista previa
    expect(computePreview).toHaveBeenCalledTimes(1);
  });
});
