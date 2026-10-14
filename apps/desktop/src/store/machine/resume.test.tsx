import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import type { InterruptedJob } from '@/api/types';
import { readJobLog, resetJobLogRecorder, useJobLog } from '@/features/help/jobLog';
import { ssr } from '@/features/machine/galvoFixtures';
import { ResumeNotice } from '@/features/machine/ResumeNotice';
import * as prompt from '@/features/history/prompt';
import { bindMachineEvents, createAppStore, useStore } from '../index';

// El marco real usa un portal (no sale en `renderToStaticMarkup`): se sustituye por uno plano.
vi.mock('@/features/machine/Modal', () => ({
  Modal: ({ title, subtitle, children, footer }: { title: string; subtitle?: string; children: React.ReactNode; footer?: React.ReactNode }) => (
    <section>
      <h2>{title}</h2>
      <p>{subtitle}</p>
      {children}
      <footer>{footer}</footer>
    </section>
  ),
}));

const memory = new Map<string, string>();

const job = (over: Partial<InterruptedJob> = {}): InterruptedJob => ({
  id: 'abc',
  kind: 'gcode',
  started_at_ms: 1_000,
  updated_at_ms: 2_000,
  device_id: 'dev',
  device_name: 'Falcon2',
  device_kind: 'grbl',
  port: 'virtual:falcon2',
  document: '/tmp/pieza.tlaku',
  total_lines: 285,
  done_lines: 120,
  percent: 42.1,
  progress_known: true,
  mpos: [10, 5],
  modal: null,
  can_resume: true,
  note: null,
  resumed: false,
  ...over,
});

describe('continuar un trabajo interrumpido (store + máquina simulada)', () => {
  let api: MockApi;
  let store: ReturnType<typeof createAppStore>;
  const importSample = () => store.getState().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });

  beforeEach(async () => {
    memory.clear();
    vi.stubGlobal('localStorage', {
      getItem: (k: string) => memory.get(k) ?? null,
      setItem: (k: string, v: string) => void memory.set(k, v),
      removeItem: (k: string) => void memory.delete(k),
    });
    resetJobLogRecorder();
    useJobLog.getState().clear();
    // Un trabajo largo para poder cortarlo a mitad.
    api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 60_000, jobSpeedup: 1 });
    setApi(api);
    store = createAppStore();
    await store.getState().loadDevices();
    await bindMachineEvents(api, store);
    store.getState().setSelectedPort('virtual:falcon2');
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().status).not.toBeNull());
    importSample();
  });

  it('un corte de luz a mitad del trabajo avisa «se interrumpió al N %» y lo anota en el registro', async () => {
    await store.getState().startJob();
    await vi.waitFor(() => expect(store.getState().progress?.acked ?? 0).toBeGreaterThan(0));
    await api.simulateFault({ fault: 'power_cut' });
    await vi.waitFor(() => expect(store.getState().interrupted).not.toBeNull());
    const j = store.getState().interrupted!;
    expect(j.can_resume).toBe(true);
    expect(j.percent).toBeGreaterThan(0);
    expect(store.getState().dialog).toEqual({ kind: 'resumeNotice' });
    await vi.waitFor(() => expect(useJobLog.getState().entries[0]?.interrupted?.id).toBe(j.id));
    expect(readJobLog()[0]!.interrupted!.total).toBe(j.total_lines);
  });

  it('cancelar a propósito no deja nada que continuar', async () => {
    await store.getState().startJob();
    await vi.waitFor(() => expect(store.getState().jobState).toBe('running'));
    await store.getState().cancelJob();
    await vi.waitFor(() => expect(store.getState().jobState).toBe('cancelled'));
    expect(await store.getState().checkInterrupted()).toBeNull();
    expect(useJobLog.getState().entries[0]?.interrupted).toBeUndefined();
  });

  it('con un trabajo en marcha no se pregunta ni se avisa', async () => {
    await store.getState().startJob();
    await vi.waitFor(() => expect(store.getState().jobState).toBe('running'));
    expect(await store.getState().checkInterrupted({ open: true })).toBeNull();
    expect(store.getState().dialog).toBeNull();
  });

  it('el motor rechaza reanudar sin home, sin encuadre o sin lista, y con todo continúa desde la línea elegida', async () => {
    await store.getState().startJob();
    await vi.waitFor(() => expect(store.getState().progress?.acked ?? 0).toBeGreaterThan(2));
    await api.simulateFault({ fault: 'power_cut' });
    await vi.waitFor(() => expect(store.getState().interrupted).not.toBeNull());
    store.getState().closeDialog();
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    const view = await api.interruptedJobView({});
    expect(view.suggested.line).toBeGreaterThan(0);
    const args = { id: store.getState().interrupted!.id, line: view.suggested.line, at: null, confirm_unmoved: true, framed_ok: true, checklist_ok: true };
    // La máquina simulada tiene homing ($22=1) y todavía no se hizo.
    expect(await api.continueCheck()).toMatchObject({ homing: true, homed: false });
    await expect(api.continueJob({ args })).rejects.toThrow(/homing/);
    await store.getState().sendLine('$H', { confirmed: true });
    expect(await api.continueCheck()).toMatchObject({ homed: true });
    await expect(api.continueJob({ args: { ...args, framed_ok: false } })).rejects.toThrow(/Encuadra/);
    await expect(api.continueJob({ args: { ...args, checklist_ok: false } })).rejects.toThrow(/lista/);
    await api.continueJob({ args });
    await vi.waitFor(() => expect(store.getState().jobState).toBe('running'));
    // El trabajo reanudado también tiene punto de control, pero ya no cuenta como «interrumpido».
    expect(await store.getState().checkInterrupted()).toBeNull();
  });

  it('descartar pide confirmación y borra el punto de control', async () => {
    await store.getState().startJob();
    await vi.waitFor(() => expect(store.getState().progress?.acked ?? 0).toBeGreaterThan(0));
    await api.simulateFault({ fault: 'power_cut' });
    await vi.waitFor(() => expect(store.getState().interrupted).not.toBeNull());
    const ask = vi.spyOn(prompt, 'askChoice').mockResolvedValueOnce('cancel').mockResolvedValueOnce('ok');
    expect(await store.getState().discardInterrupted()).toBe(false);
    expect(store.getState().interrupted).not.toBeNull();
    expect(await store.getState().discardInterrupted()).toBe(true);
    expect(store.getState().interrupted).toBeNull();
    expect(await api.interruptedJob()).toBeNull();
    ask.mockRestore();
  });
});

describe('aviso de trabajo interrumpido', () => {
  beforeEach(() => {
    useStore.setState({ interrupted: null });
  });

  it('dice el porcentaje, la línea y ofrece Continuar, Elegir el punto y Descartar', () => {
    useStore.setState({ interrupted: job() });
    const html = ssr(<ResumeNotice />);
    expect(html).toContain('42 %');
    expect(html).toContain('línea 120 de 285');
    for (const label of ['Continuar', 'Elegir el punto', 'Descartar']) expect(html).toContain(label);
  });

  it('con Ruida, K40 o galvo solo explica y no ofrece continuar', () => {
    useStore.setState({ interrupted: job({ kind: 'ruida', can_resume: false, note: 'La máquina sigue sola.', total_lines: 0, done_lines: 0, percent: 0, progress_known: false }) });
    const html = ssr(<ResumeNotice />);
    expect(html).toContain('La máquina sigue sola.');
    expect(html).toContain('Ver el estado de la máquina');
    expect(html).not.toContain('Elegir el punto');
  });

  it('sin avance legible lo dice y deja elegir a mano', () => {
    useStore.setState({ interrupted: job({ progress_known: false, done_lines: 0, percent: 0 }) });
    expect(ssr(<ResumeNotice />)).toContain('no se pudo leer hasta dónde llegó');
  });
});

describe('diálogo de reanudación', () => {
  it('sin máquina conectada pide conectar la del trabajo y no deja continuar', async () => {
    const { ResumeDialog } = await import('@/features/machine/ResumeDialog');
    useStore.setState({ interrupted: job(), connection: { connected: false, port: null, error: null } });
    const html = ssr(<ResumeDialog choose={false} />);
    expect(html).toContain('Conecta la máquina «Falcon2»');
    expect(html).toContain('Leyendo el trabajo guardado');
    expect(html).toContain('Qué se hizo y qué falta');
    expect(html).toMatch(/<button[^>]*disabled[^>]*>Continuar desde la línea/);
  });
});
