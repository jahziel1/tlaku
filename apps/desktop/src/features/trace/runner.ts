/**
 * Ejecuta un cálculo asíncrono tras una pausa (debounce) y descarta los resultados obsoletos:
 * cada `schedule` invalida lo anterior, tanto el temporizador como un cálculo ya en vuelo, así
 * que un resultado viejo que llega tarde nunca pisa al nuevo.
 */
export interface LatestRunnerOptions<A, R> {
  run: (args: A) => Promise<R>;
  delayMs: number;
  /** Se llama al programar (hay un resultado pendiente). */
  onPending?: (args: A) => void;
  onResult: (result: R, args: A) => void;
  onError: (error: unknown, args: A) => void;
}

export interface LatestRunner<A> {
  /** Programa un cálculo tras `delayMs`, anulando el anterior. */
  schedule: (args: A) => void;
  /** Igual, pero sin esperar. */
  runNow: (args: A) => void;
  /** Anula lo pendiente y lo que esté en vuelo. */
  cancel: () => void;
}

export function createLatestRunner<A, R>(opts: LatestRunnerOptions<A, R>): LatestRunner<A> {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let seq = 0;

  const clear = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  };

  const start = (args: A, token: number) => {
    opts.run(args).then(
      (result) => {
        if (token === seq) opts.onResult(result, args);
      },
      (error: unknown) => {
        if (token === seq) opts.onError(error, args);
      },
    );
  };

  return {
    schedule: (args) => {
      clear();
      const token = ++seq;
      opts.onPending?.(args);
      timer = setTimeout(() => {
        timer = null;
        start(args, token);
      }, opts.delayMs);
    },
    runNow: (args) => {
      clear();
      const token = ++seq;
      opts.onPending?.(args);
      start(args, token);
    },
    cancel: () => {
      clear();
      seq++;
    },
  };
}
