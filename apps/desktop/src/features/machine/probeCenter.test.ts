import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { bindMachineEvents, useStore } from '@/store';
import { useCenter } from '@/features/camera/centerFinder';
import {
  PROBE_DEFAULTS,
  cancelTouch,
  confirmTouch,
  probeBlockedReason,
  probeParamsProblem,
  recordPointerPoint,
  removeLastPoint,
  requestTouch,
  touchDelta,
  useProbeCenter,
} from './probeCenter';
import { probeLine } from './ProbeCenterDialog';

let api: MockApi;
let unbind: (() => void) | null = null;

const idle = () => vi.waitFor(() => expect(useStore.getState().status?.state).toBe('idle'));
const moveTo = async (x: number, y: number) => {
  await api.jogTo({ x, y, speed: 6000 });
  await vi.waitFor(() => expect(useStore.getState().status?.work_pos[0]).toBeCloseTo(x, 3));
  await idle();
};

beforeEach(async () => {
  api = createMockApi({ latencyMs: 0, statusIntervalMs: 15, minJobMs: 100, jobSpeedup: 1e6 });
  setApi(api);
  const s = useStore.getState();
  await s.loadDevices();
  unbind = await bindMachineEvents(api);
  s.setSelectedPort('virtual:grbl11');
  await s.connect();
  await idle();
  useProbeCenter.setState({ open: true, mode: 'probe', ...PROBE_DEFAULTS, points: [], confirm: null, busy: false, error: null, missed: false });
  useCenter.getState().reset();
});

afterEach(async () => {
  await useStore.getState().disconnect();
  unbind?.();
});

describe('M48: buscador de centro con el cabezal', () => {
  it('valida los parámetros con los topes del motor', () => {
    expect(probeParamsProblem({ feed: 100, travel: 30, retract: 2 })).toBeNull();
    expect(probeParamsProblem({ feed: 0, travel: 30, retract: 2 })).toBe('machine.probe.problem.feed');
    expect(probeParamsProblem({ feed: 501, travel: 30, retract: 2 })).toBe('machine.probe.problem.feed');
    expect(probeParamsProblem({ feed: 100, travel: 101, retract: 2 })).toBe('machine.probe.problem.travel');
    expect(probeParamsProblem({ feed: 100, travel: 30, retract: 21 })).toBe('machine.probe.problem.retract');
    expect(probeParamsProblem({ feed: NaN, travel: 30, retract: 2 })).toBe('machine.probe.problem.feed');
  });

  it('el recorrido sigue el origen del perfil y la línea mostrada es la que se envía', () => {
    expect(touchDelta({ origin: 'bottom_left' }, 'right', 30)).toEqual([30, 0]);
    expect(touchDelta({ origin: 'bottom_left' }, 'up', 30)).toEqual([0, 30]);
    // Con el origen arriba, «hacia arriba» del documento es Y negativa en la máquina.
    expect(touchDelta({ origin: 'top_left' }, 'up', 30)).toEqual([0, -30]);
    expect(probeLine(30, -0, 100)).toBe('G38.3 X30 Y0 F100');
  });

  it('no mueve nada hasta confirmar, y cancelar no mueve nada', async () => {
    await moveTo(70, 100);
    requestTouch('right');
    expect(useProbeCenter.getState().confirm).toBe('right');
    await new Promise((r) => setTimeout(r, 50));
    expect(useStore.getState().status?.work_pos[0]).toBeCloseTo(70, 3);
    cancelTouch();
    expect(useProbeCenter.getState().confirm).toBeNull();
    await confirmTouch();
    expect(useProbeCenter.getState().points).toHaveLength(0);
    expect(useStore.getState().status?.work_pos[0]).toBeCloseTo(70, 3);
  });

  it('tres toques dan el centro y el radio del objeto', async () => {
    for (const [from, dir] of [
      [[70, 100], 'right'],
      [[100, 70], 'up'],
      [[100, 130], 'down'],
    ] as const) {
      await moveTo(from[0], from[1]);
      requestTouch(dir);
      await confirmTouch();
      await idle();
    }
    const { points, error } = useProbeCenter.getState();
    expect(error).toBeNull();
    expect(points.map((p) => p.doc.map((v) => Math.round(v)))).toEqual([[85, 100], [100, 85], [100, 115]]);
    const c = useCenter.getState().result!;
    expect(c.source).toBe('probe');
    expect(c.circle.center[0]).toBeCloseTo(100, 3);
    expect(c.circle.center[1]).toBeCloseTo(100, 3);
    expect(c.circle.radius).toBeCloseTo(15, 3);
  });

  it('un toque sin contacto no es un error ni guarda un punto', async () => {
    await moveTo(20, 20);
    requestTouch('right');
    await confirmTouch();
    const s = useProbeCenter.getState();
    expect(s.missed).toBe(true);
    expect(s.error).toBeNull();
    expect(s.points).toHaveLength(0);
  });

  it('con la sonda ya activa el motor avisa y no se guarda nada', async () => {
    await moveTo(100, 100);
    requestTouch('right');
    await confirmTouch();
    expect(useProbeCenter.getState().error).toContain('ya está activa');
    expect(useProbeCenter.getState().points).toHaveLength(0);
  });

  it('el modo del puntero guarda la posición del cabezal y quitar el último rehace el círculo', async () => {
    useProbeCenter.setState({ mode: 'pointer' });
    for (const [x, y] of [[85, 100], [100, 85], [100, 115]] as const) {
      await moveTo(x, y);
      expect(recordPointerPoint()).toBe(true);
    }
    const c = useCenter.getState().result!;
    expect(c.source).toBe('pointer');
    expect(c.circle.center[0]).toBeCloseTo(100, 3);
    expect(c.circle.radius).toBeCloseTo(15, 3);
    removeLastPoint();
    expect(useProbeCenter.getState().points).toHaveLength(2);
    expect(useCenter.getState().result).toBeNull();
  });

  it('tres puntos en línea recta no dan un círculo', async () => {
    for (const x of [10, 20, 30]) {
      await moveTo(x, 50);
      recordPointerPoint();
    }
    expect(useCenter.getState().result).toBeNull();
    expect(useProbeCenter.getState().error).toBeTruthy();
  });

  it('sin máquina conectada la sonda está bloqueada con su motivo', async () => {
    await useStore.getState().disconnect();
    expect(probeBlockedReason(useStore.getState())).toBe('reason.needMachine');
    requestTouch('left');
    expect(useProbeCenter.getState().confirm).toBeNull();
    expect(useProbeCenter.getState().error).toBeTruthy();
  });
});
