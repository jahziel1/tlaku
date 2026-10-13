import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mockNest } from '@/api/mock/nest';
import { selectionUnits } from '@/features/arrange/units';
import { buildNestRequest, DEFAULT_NEST_OPTIONS } from './request';
import { createNestRunner, IDLE_NEST_RUN, NEST_POLL_MS, type NestRunState } from './runner';
import { docOf, rectObject } from './testUtils';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

const units = selectionUnits(docOf([rectObject('a', 20, 10)]), ['a']);
const request = (copies = 6) =>
  buildNestRequest({ units, copies: { a: copies }, container: { type: 'rect', width: 100, height: 60 }, options: DEFAULT_NEST_OPTIONS });

function setup() {
  const api = mockNest((f) => Promise.resolve().then(f));
  const states: NestRunState[] = [];
  const spies = { cancel: vi.spyOn(api, 'nestCancel'), poll: vi.spyOn(api, 'nestPoll'), start: vi.spyOn(api, 'nestStart') };
  const runner = createNestRunner({ api: () => api, onChange: (s) => states.push(s) });
  return { runner, states, spies };
}

describe('createNestRunner', () => {
  it('sondea cada 250 ms y termina cuando running es false', async () => {
    const { runner, states, spies } = setup();
    await runner.start(request());
    expect(runner.get()).toMatchObject({ phase: 'running', status: null });
    expect(spies.poll).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(NEST_POLL_MS);
    expect(spies.poll).toHaveBeenCalledTimes(1);
    expect(runner.get().status?.running).toBe(true);
    await vi.advanceTimersByTimeAsync(NEST_POLL_MS * 8);
    expect(runner.get()).toMatchObject({ phase: 'finished', error: null });
    expect(runner.get().status).toMatchObject({ running: false, placed: 6, total: 6 });
    const polls = spies.poll.mock.calls.length;
    await vi.advanceTimersByTimeAsync(5000);
    expect(spies.poll.mock.calls.length).toBe(polls);
    // la utilización sube durante el trabajo
    const seen = states.map((s) => s.status?.utilization ?? 0);
    expect([...seen].sort((a, b) => a - b)).toEqual(seen);
  });

  it('un rechazo de nestStart queda como error y vuelve a idle', async () => {
    const { runner } = setup();
    await runner.start({ ...request(), pieces: [] });
    expect(runner.get()).toMatchObject({ phase: 'idle', error: 'No hay piezas que acomodar' });
  });

  it('stop cancela, espera el último estado y lo devuelve', async () => {
    const { runner, spies } = setup();
    await runner.start(request(9));
    await vi.advanceTimersByTimeAsync(NEST_POLL_MS * 2);
    const final = await runner.stop();
    expect(spies.cancel).toHaveBeenCalledTimes(1);
    expect(final?.running).toBe(false);
    expect(final?.placed).toBeGreaterThan(0);
    expect(final?.placed).toBeLessThan(9);
    expect(runner.get()).toMatchObject({ phase: 'finished', status: final });
    await vi.advanceTimersByTimeAsync(5000);
    expect(runner.get().status).toBe(final);
    // parar de nuevo no vuelve a cancelar
    await runner.stop();
    expect(spies.cancel).toHaveBeenCalledTimes(1);
  });

  it('dispose cancela el trabajo en curso y deja de sondear', async () => {
    const { runner, spies } = setup();
    await runner.start(request());
    await vi.advanceTimersByTimeAsync(NEST_POLL_MS);
    runner.dispose();
    expect(spies.cancel).toHaveBeenCalledTimes(1);
    const polls = spies.poll.mock.calls.length;
    await vi.advanceTimersByTimeAsync(2000);
    expect(spies.poll.mock.calls.length).toBe(polls);
  });

  it('parar mientras nestStart sigue en vuelo cancela ese trabajo', async () => {
    const { runner, spies } = setup();
    const starting = runner.start(request());
    await runner.stop();
    await starting;
    await vi.advanceTimersByTimeAsync(10);
    expect(spies.cancel).toHaveBeenCalledTimes(1);
    expect(runner.get().phase).toBe('idle');
  });

  it('reset olvida el resultado terminado pero no toca un trabajo en marcha', async () => {
    const { runner } = setup();
    await runner.start(request(2));
    runner.reset();
    expect(runner.get().phase).toBe('running');
    await vi.advanceTimersByTimeAsync(NEST_POLL_MS * 8);
    expect(runner.get().phase).toBe('finished');
    runner.reset();
    expect(runner.get()).toBe(IDLE_NEST_RUN);
  });

  it('un fallo al sondear se muestra y termina', async () => {
    const { runner, spies } = setup();
    spies.poll.mockRejectedValueOnce('El motor se cayó');
    await runner.start(request());
    await vi.advanceTimersByTimeAsync(NEST_POLL_MS);
    expect(runner.get()).toMatchObject({ phase: 'finished', error: 'El motor se cayó' });
  });
});
