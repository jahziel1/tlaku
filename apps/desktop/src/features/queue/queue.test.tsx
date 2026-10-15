import type { ReactElement, ReactNode } from 'react';
import { renderToString } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { buildSnapshot } from '@/features/remote/snapshot';
import { handleRemoteStart, remoteStartBlock } from '@/features/remote/remote';
import { bindMachineEvents, useStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { frameMarkOf } from '@/store/machine';
import {
  addAllTabs,
  addFile,
  addTab,
  confirmStep,
  ensureItemTab,
  isMissing,
  moveInQueue,
  pauseQueue,
  removeFromQueue,
  resumeQueue,
  setItemCopies,
  skipCurrent,
  startQueue,
  startQueuePersistence,
  startQueueWatcher,
  stopQueue,
} from './queue';
import { QueueBody } from './QueueDialog';
import { QueueStepDialog, stepChecks } from './QueueStepDialog';
import { CHECKLIST_ALL, CHECKLIST_ITEMS } from '@/features/machine/checklist';
import { currentItem } from './queueModel';
import { loadQueue, saveQueue } from './queuePersist';
import { resetQueue, useQueue } from './queueStore';

// El marco real usa un portal (no sale al pintar en el servidor): se sustituye por uno plano.
vi.mock('@/features/machine/Modal', () => ({
  Modal: ({ title, subtitle, children, footer }: { title: string; subtitle?: string; children: ReactNode; footer?: ReactNode }) => (
    <section>
      <h2>{title}</h2>
      <p>{subtitle}</p>
      {children}
      <footer>{footer}</footer>
    </section>
  ),
}));

/** `renderToString` lee el estado INICIAL de las tiendas de zustand: se copia el de ahora mientras se pinta. */
function ssrAll(node: ReactElement): string {
  const stores = [useStore, useQueue] as unknown as { getInitialState: () => Record<string, unknown>; getState: () => Record<string, unknown> }[];
  const saved = stores.map((st) => ({ ...st.getInitialState() }));
  stores.forEach((st) => Object.assign(st.getInitialState(), st.getState()));
  try {
    return renderToString(node);
  } finally {
    stores.forEach((st, i) => Object.assign(st.getInitialState(), saved[i]));
  }
}

let api: MockApi;
let unbind: () => void;
let unwatch: () => void;
const s = () => useStore.getState();
const q = () => useQueue.getState().q;

const doc = () => ({ ...emptyDocument(), objects: sampleObjects(), layers: sampleLayers() });
let seq = 0;
/** Abre un documento con salida en una pestaña nueva y devuelve su id. */
const openDoc = (name: string) => {
  const id = s().openInTab(doc(), `/p/${name}-${++seq}.tlaku`);
  return id;
};

/** Lo que hace una persona: confirmar el paso y luego «Iniciar» en el diálogo de inicio normal. */
async function userStartsCurrentStep(): Promise<void> {
  expect(await confirmStep()).toBe(true);
  expect(s().dialog?.kind).toBe('start');
  await vi.waitFor(() => expect(s().previewStatus).toBe('ready'));
  await s().confirmStart();
}

const settle = (ms = 150) => new Promise((r) => setTimeout(r, ms));

/** Máquina simulada nueva con trabajos de `minJobMs` (los largos dejan cancelar o fallar a media cola). */
async function boot(minJobMs: number): Promise<void> {
  unwatch?.();
  unbind?.();
  api = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs, jobSpeedup: 1e6 });
  setApi(api);
  resetQueue();
  unbind = await bindMachineEvents(api);
  await s().loadDevices();
  await s().refreshPorts();
  s().setSelectedPort('virtual:falcon2');
  await s().connect();
  await vi.waitFor(() => expect(s().status !== null).toBe(true));
  // La tienda es una sola para todo el archivo: se parte de «sin trabajo» y sin diálogos.
  useStore.setState({ jobState: 'idle', jobOrigin: null, jobMessage: '', jobError: null });
  s().closeDialog();
  unwatch = startQueueWatcher();
}

beforeEach(async () => {
  await boot(150);
});

afterEach(() => {
  unwatch();
  unbind();
  resetQueue();
  vi.restoreAllMocks();
});

describe('cola guiada con la máquina simulada', () => {
  it('NUNCA avanza sola: al terminar un trabajo pide confirmar el siguiente y no envía nada', async () => {
    const startJob = vi.spyOn(api, 'startJob');
    const a = openDoc('a');
    const b = openDoc('b');
    addTab(a);
    addTab(b);
    startQueue();
    expect(q().run).toBe('awaiting');
    expect(currentItem(q())!.tabId).toBe(a);
    expect(useQueue.getState().stepOpen).toBe(true);
    // Empezar la cola no manda nada.
    await settle();
    expect(startJob).not.toHaveBeenCalled();
    expect(s().jobState).toBe('idle');

    await userStartsCurrentStep();
    expect(startJob).toHaveBeenCalledTimes(1);
    await vi.waitFor(() => expect(q().run).toBe('running'));
    await vi.waitFor(() => expect(s().jobState).toBe('finished'));
    await vi.waitFor(() => expect(q().run).toBe('awaiting'));
    expect(currentItem(q())!.tabId).toBe(b);
    expect(useQueue.getState().stepOpen).toBe(true); // vuelve a pedir confirmación

    // Pasa el tiempo: la cola sigue esperando y la máquina sigue sin recibir nada.
    await settle(400);
    expect(startJob).toHaveBeenCalledTimes(1);
    expect(q().run).toBe('awaiting');
    expect(s().jobState).toBe('finished');
  });

  it('confirmar el paso solo abre el diálogo de inicio normal (con su preflight): no inicia nada', async () => {
    const startJob = vi.spyOn(api, 'startJob');
    const a = openDoc('a');
    addTab(a);
    startQueue();
    s().activateTab(s().tabs[0]!.id); // el usuario está en otra pestaña
    expect(await confirmStep()).toBe(true);
    expect(s().activeTabId).toBe(a); // se llevó al frente el documento del paso
    expect(s().dialog?.kind).toBe('start');
    expect(useQueue.getState().stepOpen).toBe(false);
    await settle();
    expect(startJob).not.toHaveBeenCalled();
    expect(q().run).toBe('awaiting');
    // Si el usuario cierra el diálogo de inicio, la cola sigue esperando.
    s().closeDialog();
    expect(q().run).toBe('awaiting');
    expect(s().jobState).toBe('idle');
  });

  it('con la máquina desconectada, confirmar no abre nada y el paso sigue pendiente', async () => {
    const a = openDoc('a');
    addTab(a);
    startQueue();
    await s().disconnect();
    await vi.waitFor(() => expect(s().connection.connected).toBe(false));
    expect(await confirmStep()).toBe(false);
    expect(s().dialog).toBeNull();
    expect(useQueue.getState().stepOpen).toBe(true);
    expect(q().run).toBe('awaiting');
  });

  it('las copias de un mismo trabajo piden confirmar una por una', async () => {
    const startJob = vi.spyOn(api, 'startJob');
    const a = openDoc('a');
    addTab(a);
    setItemCopies(q().items[0]!.id, 2);
    startQueue();
    await userStartsCurrentStep();
    await vi.waitFor(() => expect(s().jobState).toBe('finished'));
    await vi.waitFor(() => expect(q().run).toBe('awaiting'));
    expect(currentItem(q())).toMatchObject({ done: 1, copies: 2 });
    await settle(300);
    expect(startJob).toHaveBeenCalledTimes(1);
    s().closeDialog();
    await userStartsCurrentStep();
    await vi.waitFor(() => expect(q().run).toBe('done'));
    expect(startJob).toHaveBeenCalledTimes(2);
  });

  it('si el usuario cancela el trabajo, la cola se detiene y no ofrece el siguiente', async () => {
    await boot(60_000);
    const startJob = vi.spyOn(api, 'startJob');
    const a = openDoc('a');
    const b = openDoc('b');
    addTab(a);
    addTab(b);
    startQueue();
    await userStartsCurrentStep();
    await vi.waitFor(() => expect(q().run).toBe('running'));
    await s().cancelJob();
    await vi.waitFor(() => expect(s().jobState).toBe('cancelled'));
    await vi.waitFor(() => expect(q().run).toBe('stopped'));
    expect(q().stopReason).toBe('cancelled');
    expect(useQueue.getState().stepOpen).toBe(false);
    await settle(300);
    expect(startJob).toHaveBeenCalledTimes(1);
    expect(q().items[1]!.status).toBe('pending');
  });

  it('si el trabajo falla (corte de luz), la cola se detiene', async () => {
    await boot(60_000);
    const a = openDoc('a');
    const b = openDoc('b');
    addTab(a);
    addTab(b);
    startQueue();
    await userStartsCurrentStep();
    await vi.waitFor(() => expect(q().run).toBe('running'));
    await api.simulateFault({ fault: 'power_cut' });
    await vi.waitFor(() => expect(q().run).toBe('stopped'));
    expect(q().stopReason).toBe('failed');
    expect(q().items[0]!.status).toBe('failed');
    expect(currentItem(q())!.id).toBe(q().items[0]!.id);
  });

  it('pausar con un trabajo en marcha: al terminar no pide el siguiente hasta reanudar', async () => {
    const a = openDoc('a');
    const b = openDoc('b');
    addTab(a);
    addTab(b);
    startQueue();
    await userStartsCurrentStep();
    await vi.waitFor(() => expect(q().run).toBe('running'));
    pauseQueue();
    await vi.waitFor(() => expect(q().run).toBe('paused'));
    expect(useQueue.getState().stepOpen).toBe(false);
    resumeQueue();
    expect(q().run).toBe('awaiting');
    expect(useQueue.getState().stepOpen).toBe(true);
  });

  it('detener la cola con un trabajo en la máquina no lo cancela', async () => {
    await boot(400);
    const a = openDoc('a');
    addTab(a);
    startQueue();
    await userStartsCurrentStep();
    await vi.waitFor(() => expect(q().run).toBe('running'));
    stopQueue();
    expect(q().run).toBe('stopped');
    expect(s().jobState).toBe('running');
    await vi.waitFor(() => expect(s().jobState).toBe('finished'));
    expect(q().run).toBe('stopped');
  });

  it('saltar el trabajo en espera pasa al siguiente, que también pide confirmación', () => {
    const a = openDoc('a');
    const b = openDoc('b');
    addTab(a);
    addTab(b);
    startQueue();
    skipCurrent();
    expect(q().run).toBe('awaiting');
    expect(currentItem(q())!.tabId).toBe(b);
    expect(useQueue.getState().stepOpen).toBe(true);
    skipCurrent();
    expect(q().run).toBe('done');
    expect(useQueue.getState().stepOpen).toBe(false);
  });

  it('reordenar y quitar cambian qué sigue', () => {
    const a = openDoc('a');
    const b = openDoc('b');
    const c = openDoc('c');
    addAllTabs();
    expect(q().items.map((i) => i.tabId).filter((t) => [a, b, c].includes(t!))).toEqual([a, b, c]);
    const idC = q().items.find((i) => i.tabId === c)!.id;
    moveInQueue(idC, 0);
    expect(q().items[0]!.tabId).toBe(c);
    removeFromQueue(q().items[0]!.id);
    startQueue();
    expect(currentItem(q())!.tabId).not.toBe(c);
  });

  it('agregar dos veces la misma pestaña o el mismo archivo no la duplica', () => {
    const a = openDoc('a');
    expect(addTab(a)).toBe(true);
    expect(addTab(a)).toBe(false);
    expect(addFile('/p/x.tlaku')).toBe(true);
    expect(addFile('/p/x.tlaku')).toBe(false);
    expect(q().items).toHaveLength(2);
  });
});

describe('cola y pestañas', () => {
  it('un trabajo lanzado a mano desde OTRA pestaña no mueve la cola', async () => {
    const a = openDoc('a');
    const other = openDoc('otra');
    addTab(a);
    startQueue();
    s().activateTab(other);
    await s().startJob();
    await vi.waitFor(() => expect(s().jobState).toBe('running'));
    expect(q().run).toBe('awaiting');
    await vi.waitFor(() => expect(s().jobState).toBe('finished'));
    expect(q().run).toBe('awaiting');
    expect(currentItem(q())!.tabId).toBe(a);
    expect(q().items[0]!.done).toBe(0);
  });

  it('un archivo de la cola se abre en una pestaña al confirmar (y no antes)', async () => {
    const before = s().tabs.length;
    addFile('/proyectos/caja-nueva.tlaku');
    startQueue();
    expect(s().tabs).toHaveLength(before);
    expect(await confirmStep()).toBe(true);
    expect(s().tabs.length).toBeGreaterThanOrEqual(before);
    expect(s().filePath).toBe('/proyectos/caja-nueva.tlaku');
    expect(currentItem(q())!.tabId).toBe(s().activeTabId);
  });

  it('un archivo ya abierto en una pestaña se enlaza con ella y no se abre otra', async () => {
    const id = s().openInTab(doc(), '/proyectos/ya-abierto.tlaku');
    const count = s().tabs.length;
    addFile('/proyectos/ya-abierto.tlaku');
    expect(q().items[0]!.tabId).toBe(id);
    expect(await ensureItemTab(q().items[0]!)).toBe(id);
    expect(s().tabs).toHaveLength(count);
  });

  it('una pestaña sin archivo que se cerró no se puede confirmar: la cola se detiene sin enviar nada', async () => {
    const startJob = vi.spyOn(api, 'startJob');
    const a = s().openInTab(doc(), null);
    const keep = openDoc('keep');
    addTab(a);
    startQueue();
    s().removeTab(a);
    expect(isMissing(q().items[0]!)).toBe(true);
    s().activateTab(keep);
    expect(await confirmStep()).toBe(false);
    expect(q()).toMatchObject({ run: 'stopped', stopReason: 'missing' });
    expect(startJob).not.toHaveBeenCalled();
  });

  it('el estado de las pestañas no se pierde: el documento de otra pestaña sigue siendo el suyo', async () => {
    const a = openDoc('a');
    const b = openDoc('b');
    s().applyImport({ objects: [], new_layers: [], warnings: [] });
    addTab(a);
    startQueue();
    await confirmStep();
    expect(s().activeTabId).toBe(a);
    expect(s().tabs.find((t) => t.id === b)).toBeDefined();
  });
});

describe('cola y control remoto', () => {
  it('el celular ve la cola en el informe, solo lectura', () => {
    const a = openDoc('a');
    const b = openDoc('b');
    addTab(a);
    addTab(b);
    startQueue();
    const snap = buildSnapshot(s(), Date.now());
    expect(snap.queue).toMatchObject({ state: 'awaiting', done: 0, total: 2, copy: 1 });
    expect(snap.queue!.current!.name).toMatch(/^a-/);
    expect(snap.queue!.items).toHaveLength(2);
  });

  it('sin cola el informe no la trae', () => {
    expect(buildSnapshot(s(), Date.now()).queue).toBeNull();
  });

  it('confirmar desde el celular exige estar en el documento que la cola espera', () => {
    const a = openDoc('a');
    const b = openDoc('b');
    addTab(a);
    addTab(b);
    startQueue();
    // La pestaña activa es la B (la última abierta) pero la cola espera la A.
    expect(s().activeTabId).toBe(b);
    expect(remoteStartBlock(s())).toBe('queue_other_tab');
    expect(buildSnapshot(s(), Date.now()).framed).toBe(false);
    s().activateTab(a);
    expect(remoteStartBlock(s())).not.toBe('queue_other_tab');
  });

  it('el celular no puede iniciar otro documento mientras la cola espera confirmar uno', async () => {
    const startJob = vi.spyOn(api, 'startJob');
    const a = openDoc('a');
    const b = openDoc('b');
    addTab(a);
    addTab(b);
    startQueue();
    await handleRemoteStart({ id: 1, client: '192.168.1.9 · iPhone' });
    expect(startJob).not.toHaveBeenCalled();
    expect(s().jobState).toBe('idle');
    expect(q().run).toBe('awaiting');
  });

  it('con el documento correcto y encuadrado, el celular confirma el paso y la cola pasa a «running»', async () => {
    const a = openDoc('a');
    addTab(a);
    startQueue();
    expect(s().activeTabId).toBe(a);
    await s().computePreview();
    // Encuadrado en la PC (lo que hace el botón «Encuadrar» al terminar).
    useStore.setState({ framedMark: frameMarkOf(s()) });
    expect(buildSnapshot(s(), Date.now()).framed).toBe(true);
    expect(remoteStartBlock(s())).toBeNull();
    await handleRemoteStart({ id: 2, client: '192.168.1.9 · iPhone' });
    await vi.waitFor(() => expect(['running', 'done']).toContain(q().run));
  });
});

describe('dependencias de la interfaz', () => {
  it('el diálogo de gestión muestra la lista y avisa que nunca avanza sola', () => {
    const a = openDoc('a');
    addTab(a);
    const html = ssrAll(<QueueBody />);
    expect(html).toContain('nunca envía el siguiente trabajo por sí sola');
    expect(html).toContain('Empezar la cola');
    expect(html).toContain('a-');
  });

  it('el paso de confirmación trae las casillas y deja «Continuar» deshabilitado', () => {
    const a = openDoc('a');
    addTab(a);
    startQueue();
    const html = ssrAll(<QueueStepDialog />);
    expect(html).toContain('Cambié el material');
    expect(html).toContain('el área está despejada');
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Continuar al diálogo de inicio/);
    expect(html).toContain('no envía nada');
  });

  it('las casillas del paso son las propias de la cola más la lista de comprobación, sin ids repetidos', () => {
    const all = stepChecks(CHECKLIST_ALL);
    expect(all.map((c) => c.id)).toEqual(['queue-material', 'queue-area', ...CHECKLIST_ITEMS.map((i) => i.id)]);
    expect(new Set(all.map((c) => c.id)).size).toBe(all.length);
    // Sin lista de comprobación configurada quedan las dos propias: la cola siempre pide el material y el área.
    expect(stepChecks(0).map((c) => c.id)).toEqual(['queue-material', 'queue-area']);
  });

  it('la definición se recuerda (solo archivos) y se restaura detenida y sin copias hechas', () => {
    const memory = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (k: string) => memory.get(k) ?? null,
      setItem: (k: string, v: string) => void memory.set(k, v),
      removeItem: (k: string) => void memory.delete(k),
    });
    try {
      const a = s().openInTab(doc(), null); // sin archivo: no se puede recordar
      addTab(a);
      addFile('/p/uno.tlaku');
      addFile('/p/dos.tlaku');
      setItemCopies(q().items.find((i) => i.path === '/p/dos.tlaku')!.id, 4);
      saveQueue(q().items);
      const loaded = loadQueue();
      expect(loaded.map((i) => [i.path, i.copies])).toEqual([['/p/uno.tlaku', 1], ['/p/dos.tlaku', 4]]);
      resetQueue();
      const stop = startQueuePersistence();
      expect(q().run).toBe('idle');
      expect(q().items.map((i) => i.path)).toEqual(['/p/uno.tlaku', '/p/dos.tlaku']);
      expect(q().items.every((i) => i.done === 0 && i.status === 'pending')).toBe(true);
      stop();
      memory.set('tlaku.queue', '{no es json');
      expect(loadQueue()).toEqual([]);
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
