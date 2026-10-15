// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { emptyLibrary } from '@/features/materials/library';
import { useMaterials } from '@/features/materials/state';
import { defaultForm } from '@/features/materials/testPlan';
import { en, es, setLanguage } from '@/i18n';
import { useStore } from '@/store';
import { audit, cleanup, click, describeViolations, mount } from '@/test/dom';
import { CalibrateDialog } from './CalibrateDialog';
import { gridCells } from './calibration';
import { presetCurve, toneDarkness } from './curve';
import { useTonal } from './store';
import { TonalControls } from './TonalControls';

afterEach(cleanup);

let api: ReturnType<typeof createMockApi>;

beforeEach(() => {
  setLanguage('es');
  api = createMockApi({ latencyMs: 0 });
  setApi(api);
  useMaterials.setState({ library: emptyLibrary(), loaded: true, loadError: null, testForm: null });
  useTonal.setState({ mode: 'normal', material: 'plywood', spotMm: 0.1, status: 'idle', error: null, calibrateOpen: false });
  useStore.setState({ previewVisible: false, preview: null });
});

const cellButtons = () => [...document.querySelectorAll<HTMLButtonElement>('.tn-cell')];
const stepButton = (n: number) => document.querySelector<HTMLButtonElement>(`.tn-step[aria-label="Tono ${n} %"]`)!;
const button = (text: string) => [...document.querySelectorAll('button')].find((b) => b.textContent?.trim() === text)!;

describe('textos', () => {
  it('los textos de la vista por tonos existen en español e inglés (la igualdad la vigila i18n.test)', () => {
    const keys = Object.keys(es).filter((k) => k.startsWith('preview.tonal.'));
    expect(keys.length).toBeGreaterThan(40);
    for (const k of keys) {
      const english = (en as Record<string, string | undefined>)[k];
      expect(english, k).toBeTruthy();
    }
  });
});

describe('selector Normal / Por tonos', () => {
  it('muestra el aviso de aproximado y el material solo en modo por tonos', async () => {
    await mount(<TonalControls />);
    expect(document.body.textContent).not.toContain('Simulación aproximada');
    expect(document.querySelector('select')).toBeNull();
    await click(button('Por tonos'));
    expect(useTonal.getState().mode).toBe('tonal');
    expect(document.body.textContent).toContain('Simulación aproximada');
    expect(document.querySelectorAll('select option')).toHaveLength(9);
    await click(button('Normal'));
    expect(useTonal.getState().mode).toBe('normal');
  });

  it('axe no encuentra problemas', async () => {
    await mount(<TonalControls />);
    await click(button('Por tonos'));
    const r = await audit();
    expect(describeViolations(r)).toEqual([]);
  });

  it('el botón Calibrar abre el diálogo', async () => {
    await mount(<TonalControls />);
    await click(button('Por tonos'));
    await click([...document.querySelectorAll('button')].find((b) => b.textContent?.includes('Calibrar'))!);
    expect(useTonal.getState().calibrateOpen).toBe(true);
    expect(document.querySelector('[role="dialog"], .mat-modal')).not.toBeNull();
  });
});

describe('calibración con la prueba de materiales', () => {
  it('muestra la cuadrícula 5 x 5 de la prueba y pasa axe', async () => {
    await mount(<CalibrateDialog />);
    expect(cellButtons()).toHaveLength(25);
    const r = await audit();
    expect(describeViolations(r)).toEqual([]);
  });

  it('una observación ajusta la curva y se guarda en la biblioteca local', async () => {
    const saved = vi.spyOn(api, 'saveMaterialLibrary');
    await mount(<CalibrateDialog />);
    // La celda del medio; el material real se ve mucho más claro de lo que la curva predice.
    const cells = gridCells(defaultForm(null), 0.1);
    const mid = cells[2]![2]!;
    expect(toneDarkness(presetCurve('plywood'), mid.dose)).toBeGreaterThan(0.3);
    await click(cellButtons()[12]!);
    await click(stepButton(10));
    expect(document.querySelector('.tn-cell[data-observed="true"]')).not.toBeNull();
    await click(button('Guardar calibración'));

    const tones = useMaterials.getState().library.tones ?? [];
    expect(tones).toHaveLength(1);
    expect(tones[0]!.material).toBe('plywood');
    expect(tones[0]!.curve.dose_char).toBeGreaterThan(presetCurve('plywood').dose_char);
    expect(tones[0]!.samples).toEqual([{ dose: mid.dose, darkness: 0.1 }]);
    expect(saved).toHaveBeenCalledTimes(1);
    expect(saved.mock.calls[0]![0].library.tones).toEqual(tones);
    expect(useTonal.getState().calibrateOpen).toBe(false);
    // Lo guardado vuelve al reabrir: la observación sigue en su celda.
    useTonal.setState({ calibrateOpen: true });
    await cleanup();
    await mount(<CalibrateDialog />);
    expect(document.querySelectorAll('.tn-cell[data-observed="true"]')).toHaveLength(1);
  });

  it('volver a los valores de partida quita la calibración', async () => {
    const curve = { ...presetCurve('plywood'), dose_char: 0.9 };
    useMaterials.setState({ library: { ...emptyLibrary(), tones: [{ material: 'plywood', curve, samples: [] }, { material: 'mdf', curve: presetCurve('mdf'), samples: [] }] } });
    await mount(<CalibrateDialog />);
    await click(button('Volver a los valores de partida'));
    expect((useMaterials.getState().library.tones ?? []).map((c) => c.material)).toEqual(['mdf']);
  });

  it('con la biblioteca sin leer no guarda nada', async () => {
    useMaterials.setState({ loadError: 'dañada' });
    const saved = vi.spyOn(api, 'saveMaterialLibrary');
    await mount(<CalibrateDialog />);
    await click(cellButtons()[0]!);
    await click(stepButton(50));
    await click(button('Guardar calibración'));
    expect(useMaterials.getState().library.tones).toBeUndefined();
    expect(saved).not.toHaveBeenCalled();
    expect(useStore.getState().notices.at(-1)?.text).toContain('no se pudo leer');
  });
});
