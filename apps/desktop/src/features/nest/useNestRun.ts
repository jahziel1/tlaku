import { useCallback, useEffect, useRef, useState } from 'react';
import { getApi } from '@/api';
import { createNestRunner, IDLE_NEST_RUN, type NestRunner, type NestRunState } from './runner';
import type { NestRequest, NestStatus } from './types';

export { NEST_POLL_MS } from './runner';

export interface NestRun extends NestRunState {
  start: (request: NestRequest) => Promise<void>;
  /** Pide parar y espera el último estado (la mejor solución). */
  stop: () => Promise<NestStatus | null>;
  reset: () => void;
}

/** Enlaza `createNestRunner` con React: al desmontar (cerrar el diálogo) cancela lo que siga corriendo. */
export function useNestRun(): NestRun {
  const [state, setState] = useState<NestRunState>(IDLE_NEST_RUN);
  const runner = useRef<NestRunner | null>(null);

  useEffect(() => {
    const r = createNestRunner({ api: getApi, onChange: setState });
    runner.current = r;
    return () => r.dispose();
  }, []);

  const start = useCallback((request: NestRequest) => runner.current?.start(request) ?? Promise.resolve(), []);
  const stop = useCallback(() => runner.current?.stop() ?? Promise.resolve(null), []);
  const reset = useCallback(() => runner.current?.reset(), []);
  return { ...state, start, stop, reset };
}
