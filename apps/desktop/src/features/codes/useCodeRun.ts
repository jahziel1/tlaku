import { useEffect, useRef, useState } from 'react';
import { getApi, type CodeResult, type CodeSpec } from '@/api';
import { errorMessage } from '@/lib/errors';
import { createLatestRunner, type LatestRunner } from '@/features/trace/runner';

/** Pausa tras el último cambio antes de volver a generar el código. */
export const CODE_DEBOUNCE_MS = 200;

export interface CodeRun {
  /** `idle`: no hay contenido todavía. `computing`: hay un cálculo pendiente o en curso. */
  status: 'idle' | 'computing' | 'ready' | 'error';
  result: CodeResult | null;
  error: string | null;
}

/**
 * Genera el código y lo recalcula (con debounce) cuando cambia la especificación. Un resultado
 * viejo nunca pisa al nuevo. Sin contenido no se pide nada.
 */
export function useCodeRun(spec: CodeSpec): CodeRun {
  const [run, setRun] = useState<CodeRun>({ status: 'idle', result: null, error: null });
  const runner = useRef<LatestRunner<CodeSpec> | null>(null);

  useEffect(() => {
    const r = createLatestRunner<CodeSpec, CodeResult>({
      run: (s) => getApi().generateCode({ spec: s }),
      delayMs: CODE_DEBOUNCE_MS,
      onPending: () => setRun((p) => (p.status === 'computing' ? p : { ...p, status: 'computing' })),
      onResult: (result) => setRun({ status: 'ready', result, error: null }),
      onError: (e) => setRun({ status: 'error', result: null, error: errorMessage(e) }),
    });
    runner.current = r;
    return () => r.cancel();
  }, []);

  useEffect(() => {
    if (spec.data.length === 0) {
      runner.current?.cancel();
      setRun({ status: 'idle', result: null, error: null });
      return;
    }
    runner.current?.schedule(spec);
  }, [spec]);

  return run;
}
