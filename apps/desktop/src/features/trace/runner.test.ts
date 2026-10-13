import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createLatestRunner } from './runner';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

/** Promesa que se resuelve a mano, para ordenar las respuestas. */
function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function setup(delayMs = 100) {
  const calls: { arg: number; d: ReturnType<typeof deferred<string>> }[] = [];
  const results: [string, number][] = [];
  const errors: unknown[] = [];
  const pending: number[] = [];
  const runner = createLatestRunner<number, string>({
    delayMs,
    run: (arg) => {
      const d = deferred<string>();
      calls.push({ arg, d });
      return d.promise;
    },
    onPending: (a) => pending.push(a),
    onResult: (r, a) => results.push([r, a]),
    onError: (e) => errors.push(e),
  });
  return { runner, calls, results, errors, pending };
}

describe('createLatestRunner', () => {
  it('junta una ráfaga de cambios en un solo cálculo, con el último valor', async () => {
    const { runner, calls, pending } = setup();
    runner.schedule(1);
    await vi.advanceTimersByTimeAsync(50);
    runner.schedule(2);
    await vi.advanceTimersByTimeAsync(50);
    runner.schedule(3);
    expect(calls).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(100);
    expect(calls.map((c) => c.arg)).toEqual([3]);
    expect(pending).toEqual([1, 2, 3]);
  });

  it('un resultado viejo que llega tarde no pisa al nuevo', async () => {
    const { runner, calls, results } = setup();
    runner.schedule(1);
    await vi.advanceTimersByTimeAsync(100);
    runner.schedule(2); // el 1 sigue en vuelo
    await vi.advanceTimersByTimeAsync(100);
    expect(calls.map((c) => c.arg)).toEqual([1, 2]);
    calls[1]!.d.resolve('nuevo');
    await vi.advanceTimersByTimeAsync(0);
    calls[0]!.d.resolve('viejo');
    await vi.advanceTimersByTimeAsync(0);
    expect(results).toEqual([['nuevo', 2]]);
  });

  it('un resultado en vuelo se descarta si luego se programa otro, aunque el nuevo no haya salido', async () => {
    const { runner, calls, results } = setup();
    runner.runNow(1);
    runner.schedule(2);
    calls[0]!.d.resolve('viejo');
    await vi.advanceTimersByTimeAsync(0);
    expect(results).toEqual([]);
  });

  it('los errores obsoletos tampoco se reportan', async () => {
    const { runner, calls, errors } = setup();
    runner.runNow(1);
    runner.runNow(2);
    calls[0]!.d.reject(new Error('viejo'));
    calls[1]!.d.reject(new Error('nuevo'));
    await vi.advanceTimersByTimeAsync(0);
    expect(errors).toHaveLength(1);
    expect((errors[0] as Error).message).toBe('nuevo');
  });

  it('cancel anula el temporizador y lo que esté en vuelo', async () => {
    const { runner, calls, results } = setup();
    runner.schedule(1);
    runner.cancel();
    await vi.advanceTimersByTimeAsync(500);
    expect(calls).toHaveLength(0);
    runner.runNow(2);
    runner.cancel();
    calls[0]!.d.resolve('tarde');
    await vi.advanceTimersByTimeAsync(0);
    expect(results).toEqual([]);
  });
});
