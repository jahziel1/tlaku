import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { useJobLog } from '@/features/help/jobLog';
import { bindMachineEvents, createAppStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { calibrationFactor, CALIBRATION_MIN_JOBS } from './costMetrics';

/**
 * La calibración del tiempo de costos sale del registro de trabajos. El estimado que se anota es el de la
 * vista previa de la pestaña desde la que se lanzó el trabajo, aunque el usuario ya esté en otra.
 */

let store: ReturnType<typeof createAppStore>;
const s = () => store.getState();

beforeEach(async () => {
  const api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 200, jobSpeedup: 1e6 });
  setApi(api);
  store = createAppStore();
  await s().loadDevices();
  await bindMachineEvents(api, store);
  await s().refreshPorts();
  await s().connect();
  await vi.waitFor(() => expect(s().status !== null).toBe(true));
  useJobLog.setState({ entries: [] });
});

describe('calibración de costos con pestañas', () => {
  it('el estimado anotado es el de la pestaña de origen, no el de la que esté activa al terminar', async () => {
    s().openInTab({ ...emptyDocument(), objects: sampleObjects(), layers: sampleLayers() }, '/p/placa.tlaku');
    await s().computePreview();
    const estimated = s().preview!.total_s;
    expect(estimated).toBeGreaterThan(0);
    await s().startJob();

    // El usuario se va a otra pestaña y calcula allí una vista previa distinta (vacía).
    s().newTab();
    await s().computePreview();
    expect(s().preview!.total_s).not.toBe(estimated);

    await vi.waitFor(() => expect(s().jobState).toBe('finished'));
    await vi.waitFor(() => expect(useJobLog.getState().entries.length).toBeGreaterThan(0));
    const entry = useJobLog.getState().entries[0]!;
    expect(entry).toMatchObject({ document: 'placa.tlaku', result: 'finished', estimatedS: estimated });
    expect(entry.actualS).toBeGreaterThan(0);

    // Con suficientes trabajos iguales, el factor es real ÷ estimado de ese trabajo.
    const log = Array.from({ length: CALIBRATION_MIN_JOBS }, () => entry);
    expect(calibrationFactor(log, entry.machine)).toEqual({ factor: entry.actualS / estimated, jobs: CALIBRATION_MIN_JOBS });
  });
});
