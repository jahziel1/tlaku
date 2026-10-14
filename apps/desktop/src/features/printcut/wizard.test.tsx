import { renderToString } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { mockSheetPoint, mockSheetTransform } from '@/api/mock/printcut';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { commands } from '@/commands';
import { bindMachineEvents, useStore } from '@/store';
import { DEFAULT_JOB_OPTIONS } from '@/store/preview';
import { acceptRegistration, addRegistrationMarks, goToCurrentMark, laserBedPosition, resetRegistration, saveLaserPosition, savePickedPoint, takeSelectionAsMarks } from './actions';
import { PrintCutHost, PrintCutPanel } from './PrintCutPanel';
import { ResultView } from './StepResult';
import { computeRegistration } from './flow';
import type { RegistrationState } from './useRegistration';
import { usePrintCut } from './printcutStore';

const memory = new Map<string, string>();
let api: MockApi;

async function connectedMachine() {
  memory.clear();
  vi.stubGlobal('localStorage', { getItem: (k: string) => memory.get(k) ?? null, setItem: (k: string, v: string) => void memory.set(k, v) });
  api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 200, jobSpeedup: 1e6 });
  setApi(api);
  const s = useStore.getState();
  await s.loadDevices();
  await bindMachineEvents(api, useStore);
  await s.refreshPorts();
  await s.connect();
  await vi.waitFor(() => expect(useStore.getState().status).not.toBeNull());
}

beforeEach(() => {
  usePrintCut.setState({ open: false, step: 'marks', marks: [], physical: [], current: 0, method: 'laser', picking: false, mode: 'auto', applied: null });
  useStore.setState({ jobOptions: { ...DEFAULT_JOB_OPTIONS } });
  useStore.getState().replaceDocument({ version: 2, objects: [], layers: [], device_id: null }, null);
  useStore.getState().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });
});

afterEach(() => {
  api?.dispose();
  useStore.setState({ connection: { connected: false, port: null, error: null }, status: null });
  vi.unstubAllGlobals();
});

describe('asistente Print & Cut: paso 1', () => {
  it('cerrado no pinta nada y abierto muestra el paso de marcas', () => {
    expect(renderToString(<PrintCutHost />)).toBe('');
    const html = renderToString(<PrintCutPanel />);
    expect(html).toContain('Print &amp; Cut');
    expect(html).toContain('Agregar marcas de registro');
    expect(html).toContain('Usar la selección como marcas');
    expect(html).toContain('Todavía no hay marcas elegidas.');
    expect(html).toContain('aria-disabled="true"'); // «Siguiente» espera a que haya marcas
  });

  it('el comando abre el asistente y «quitar» solo se habilita con un registro activo', async () => {
    expect(commands.availability('printcut.reset')).toBe('printcut.reason.none');
    await commands.run('printcut.open');
    expect(usePrintCut.getState().open).toBe(true);
    useStore.getState().setJobOptions({ transform: [1, 0, 0, 1, 2, 3] });
    expect(commands.availability('printcut.reset')).toBe(true);
    await commands.run('printcut.reset');
    expect(useStore.getState().jobOptions.transform).toBeNull();
  });

  it('«Agregar marcas» inserta objetos en la capa de guías y los deja elegidos', () => {
    const before = useStore.getState().doc.objects.length;
    expect(addRegistrationMarks('circle', 3)).toBe(true);
    const s = useStore.getState();
    expect(s.doc.objects.length).toBe(before + 3);
    const layer = s.doc.layers.find((l) => l.color === s.doc.objects.at(-1)!.layer)!;
    expect(layer.output).toBe(false); // no se cortan
    expect(s.selectedIds).toHaveLength(3);
    expect(usePrintCut.getState().marks).toHaveLength(3);
    expect(usePrintCut.getState().physical).toEqual([null, null, null]);
    // El historial lo deshace de una vez.
    s.undo();
    expect(useStore.getState().doc.objects.length).toBe(before);
  });

  it('con un documento vacío avisa y no agrega nada', () => {
    useStore.getState().replaceDocument({ version: 2, objects: [], layers: [], device_id: null }, null);
    expect(addRegistrationMarks('cross', 2)).toBe(false);
    expect(useStore.getState().doc.objects).toHaveLength(0);
  });

  it('la selección sirve como marcas solo si son 2 a 4 y están separadas', () => {
    const s = useStore.getState();
    s.select([s.doc.objects[0]!.id]);
    expect(takeSelectionAsMarks()).toBe(false);
    expect(usePrintCut.getState().marks).toHaveLength(0);
    addRegistrationMarks('cross', 2);
    useStore.getState().select([]);
    expect(takeSelectionAsMarks()).toBe(false);
    useStore.getState().select(usePrintCut.getState().marks.flatMap((m) => m.ids));
    expect(takeSelectionAsMarks()).toBe(true);
    expect(usePrintCut.getState().marks).toHaveLength(2);
  });
});

describe('asistente Print & Cut: medir y aceptar', () => {
  it('sin máquina no se puede guardar la posición del láser', () => {
    addRegistrationMarks('cross', 2);
    expect('reason' in laserBedPosition(useStore.getState())).toBe(true);
    expect(saveLaserPosition()).toBe(false);
    expect(usePrintCut.getState().physical).toEqual([null, null]);
  });

  it('con la hoja simulada desfasada y girada recupera la transformación y la aplica al trabajo', async () => {
    await connectedMachine();
    addRegistrationMarks('cross', 2);
    const sheet = mockSheetTransform();

    // Medir con el láser: ir a la marca impresa simulada y guardar.
    for (let i = 0; i < 2; i++) {
      const mark = usePrintCut.getState().marks[usePrintCut.getState().current]!;
      const [x, y] = mockSheetPoint(mark.point, sheet);
      await useStore.getState().goToDocPoint(x, y);
      await vi.waitFor(() => expect(useStore.getState().status?.work_pos[0]).toBeCloseTo(x, 2));
      expect(saveLaserPosition()).toBe(true);
    }
    const ui = usePrintCut.getState();
    expect(ui.physical.every((p) => p !== null)).toBe(true);
    // Tras guardar la primera pasa a la segunda.
    expect(ui.physical[0]![0]).toBeCloseTo(mockSheetPoint(ui.marks[0]!.point, sheet)[0], 2);

    const before = await api.computeJob({ doc: useStore.getState().doc, device: useStore.getState().devices[0]!, options: { ...DEFAULT_JOB_OPTIONS } });
    useStore.getState().setJobOptions({ start_from: 'current_position' });
    expect(await acceptRegistration()).toBe(true);
    const o = useStore.getState().jobOptions;
    expect(o.start_from).toBe('absolute'); // Print & Cut pide coordenadas absolutas
    for (let i = 0; i < 6; i++) expect(o.transform![i]).toBeCloseTo(sheet[i]!, 1);
    expect(usePrintCut.getState().applied?.rotationDeg).toBeCloseTo(0.8, 1);

    // La vista previa (la del mock) sale con el trabajo ya movido.
    const after = await api.computeJob({ doc: useStore.getState().doc, device: useStore.getState().devices[0]!, options: o });
    const truth = await api.computeJob({ doc: useStore.getState().doc, device: useStore.getState().devices[0]!, options: { ...DEFAULT_JOB_OPTIONS, transform: sheet } });
    expect(Math.abs(after.bounds[0][0] - before.bounds[0][0])).toBeGreaterThan(0.3);
    for (const i of [0, 1] as const) for (const j of [0, 1] as const) expect(after.bounds[i][j]).toBeCloseTo(truth.bounds[i][j], 0);
    expect(after.moves.length).toBe(before.moves.length);

    // Restablecer lo quita.
    resetRegistration();
    expect(useStore.getState().jobOptions.transform).toBeNull();
    expect(usePrintCut.getState().applied).toBeNull();
  });

  it('con la cámara un clic sobre la imagen da la posición de la marca en curso', () => {
    addRegistrationMarks('cross', 3);
    savePickedPoint([10.12345, 20]);
    const ui = usePrintCut.getState();
    expect(ui.physical[0]).toEqual([10.123, 20]);
    expect(ui.current).toBe(1);
    expect(ui.picking).toBe(false);
  });

  it('no acepta si faltan marcas por medir y «ir a la marca» necesita la máquina', async () => {
    addRegistrationMarks('cross', 2);
    savePickedPoint([5, 5]);
    expect(await acceptRegistration()).toBe(false);
    expect(useStore.getState().jobOptions.transform).toBeNull();
    expect(await goToCurrentMark()).toBe(false);
  });

  it('el paso de resultado muestra el error residual y los avisos de escala', async () => {
    const marks = [
      { ids: ['a'], point: [0, 0] as [number, number] },
      { ids: ['b'], point: [200, 100] as [number, number] },
    ];
    const view = async (physical: ([number, number] | null)[], moved = false) => {
      const result = await computeRegistration(marks, physical, 'auto');
      const state: RegistrationState = result ? { status: 'ready', result } : { status: 'incomplete' };
      return renderToString(<ResultView marks={marks} physical={physical} state={state} moved={moved} />);
    };
    // La hoja salió un 1 % más grande: debe avisar.
    const html = await view([[0, 0], [202, 101]]);
    expect(html).toContain('Error medio');
    expect(html).toContain('Error máximo');
    expect(html).toContain('más de 0,5 %');
    expect(html).toContain('revisa los avisos'); // el error es bajo pero la escala no
    const good = await view([[3, 2], [203, 102]]);
    expect(good).toContain('pc-verdict--ok');
    expect(good).not.toContain('más de 0,5 %');
    expect(await view([[3, 2], null])).toContain('Faltan marcas por medir.');
    expect(await view([[3, 2], [203, 102]], true)).toContain('se movieron o se borraron');
    // Mientras el motor responde, se dice que se está calculando.
    const pending = renderToString(<ResultView marks={marks} physical={[[3, 2], [203, 102]]} state={{ status: 'pending' }} moved={false} />);
    expect(pending).toContain('Calculando el ajuste');
    // Un ajuste imposible llega como dato, con su motivo.
    const bad = renderToString(<ResultView marks={marks} physical={[[0, 0], [0, 0]]} state={{ status: 'ready', result: { ok: false, error: { kind: 'few', need: 2, have: 1 } } }} moved={false} />);
    expect(bad).toContain('role="alert"');
  });
});
