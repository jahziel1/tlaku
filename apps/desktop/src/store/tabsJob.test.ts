import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { setApi } from '@/api';
import type { DesignObject, Document, TextObject } from '@/api/types';
import { DEFAULT_VARIABLE_TEXT } from '@/features/variables/settings';
import { useJobLog } from '@/features/help/jobLog';
import { jobFraction } from '@/features/remote/snapshot';
import { buildSnapshot } from '@/features/remote/snapshot';
import { createAppStore, bindMachineEvents, selectJobPreview, tabViews } from './index';
import { emptyDocument } from './document';

let api: MockApi;
let store: ReturnType<typeof createAppStore>;
const s = () => store.getState();

const textKind = (text: string): TextObject => ({
  text, font_family: 'Arial', bold: false, italic: false, size_mm: 10, letter_spacing_mm: 0,
  line_spacing: 1.2, align: 'left', on_path: null, outline: [],
});
const label: DesignObject = {
  id: 'etiqueta', layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null,
  kind: { type: 'text', ...textKind('N {serie}') },
};
const serialDoc = (): Document => ({
  ...emptyDocument(),
  objects: [...sampleObjects(), label],
  layers: sampleLayers(),
  variable_text: { ...DEFAULT_VARIABLE_TEXT, serial: 5, advance_after_job: true },
});

beforeEach(async () => {
  api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 200, jobSpeedup: 1e6 });
  setApi(api);
  store = createAppStore();
  await s().loadDevices();
  await bindMachineEvents(api, store);
  await s().refreshPorts();
  await s().connect();
  await vi.waitFor(() => expect(s().status !== null).toBe(true));
});

describe('máquina compartida y pestañas', () => {
  it('el trabajo recuerda la pestaña desde la que se lanzó, su nombre y su vista previa', async () => {
    s().openInTab({ ...emptyDocument(), objects: sampleObjects(), layers: sampleLayers() }, '/p/placa.tlaku');
    const a = s().activeTabId;
    await s().computePreview();
    const preview = s().preview;
    expect(preview).not.toBeNull();
    await s().startJob();
    expect(s().jobState).toBe('running');
    expect(s().jobOrigin).toMatchObject({ tabId: a, name: 'placa.tlaku', path: '/p/placa.tlaku' });
    expect(s().jobOrigin!.preview).toBe(preview);

    // El usuario se va a otra pestaña y la edita: el trabajo sigue siendo el de la primera.
    s().newTab();
    s().applyImport({ objects: sampleObjects().slice(0, 1), new_layers: [], warnings: [] });
    expect(selectJobPreview(s())).toBe(preview);
    expect(s().jobOrigin!.tabId).toBe(a);
    const snap = buildSnapshot(s(), Date.now());
    expect(snap.doc_name).toBe('placa.tlaku'); // el celular ve el documento del trabajo, no el de la pestaña activa
    expect(snap.total_s).toBe(preview!.total_s);
    expect(jobFraction(s(), Date.now())).toBeGreaterThanOrEqual(0);
  });

  it('el registro de trabajos anota el archivo de la pestaña de origen aunque haya otra activa', async () => {
    s().openInTab({ ...emptyDocument(), objects: sampleObjects(), layers: sampleLayers() }, '/p/placa.tlaku');
    await s().startJob();
    s().newTab(); // sin archivo
    const jobs = useJobLog.getState();
    await vi.waitFor(() => expect(s().jobState).toBe('finished'));
    const last = jobs.entries?.[0] ?? useJobLog.getState().entries?.[0];
    expect(last?.document).toBe('placa.tlaku');
  });

  it('al terminar bien, la serie avanza en la pestaña de origen y no en la que esté activa', async () => {
    s().openInTab(serialDoc(), '/p/etiquetas.tlaku');
    const a = s().activeTabId;
    await s().startJob();
    const b = s().newTab();
    s().applyImport({ objects: [{ ...label, id: 'otra' }], new_layers: [], warnings: [] });
    await vi.waitFor(() => expect(s().jobState).toBe('finished'));
    await vi.waitFor(() => expect(s().jobOrigin).toBeNull());
    expect(s().activeTabId).toBe(b);
    expect(s().doc.variable_text).toBeUndefined(); // la otra pestaña no se tocó
    s().activateTab(a);
    expect(s().doc.variable_text?.serial).toBe(6);
  });

  it('si la pestaña de origen se cierra durante el trabajo, no se avanza nada en otra', async () => {
    s().openInTab(serialDoc(), '/p/etiquetas.tlaku');
    const a = s().activeTabId;
    await s().startJob();
    const b = s().newTab();
    s().applyImport({ objects: [{ ...label, id: 'otra' }], new_layers: [], warnings: [] });
    s().removeTab(a);
    await vi.waitFor(() => expect(s().jobState).toBe('finished'));
    expect(s().activeTabId).toBe(b);
    expect(s().doc.variable_text).toBeUndefined();
  });

  it('un inicio rechazado no deja una pestaña de origen colgada', async () => {
    s().openInTab({ ...emptyDocument(), objects: sampleObjects(), layers: sampleLayers() }, '/p/placa.tlaku');
    api.machine.state = 'alarm';
    await s().startJob();
    expect(s().jobError).toMatch(/alarma/i);
    expect(s().jobOrigin).toBeNull();
  });

  it('encuadrar y la vista previa actúan sobre la pestaña activa', async () => {
    s().openInTab({ ...emptyDocument(), objects: sampleObjects(), layers: sampleLayers() }, '/p/a.tlaku');
    const a = s().activeTabId;
    s().newTab();
    await s().computePreview();
    expect(s().preview?.moves.length ?? 0).toBe(0); // la pestaña vacía no tiene recorrido
    s().activateTab(a);
    await s().computePreview();
    expect(s().preview?.moves.length ?? 0).toBeGreaterThan(0);
    expect(tabViews(s()).find((v) => v.active)!.id).toBe(a);
  });
});
