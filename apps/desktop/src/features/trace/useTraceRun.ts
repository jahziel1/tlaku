import { useEffect, useRef, useState } from 'react';
import { getApi, type TraceResult, type TraceSettings } from '@/api';
import { errorMessage } from '@/lib/errors';
import { createLatestRunner, type LatestRunner } from './runner';

/** Pausa tras el último cambio de un ajuste antes de recalcular. */
export const TRACE_DEBOUNCE_MS = 250;

export interface TraceRun {
  /** `computing`: hay un cálculo pendiente o en curso (el resultado mostrado puede ser el anterior). */
  status: 'computing' | 'ready' | 'error';
  result: TraceResult | null;
  error: string | null;
}

/**
 * Vectoriza `path` y recalcula (con debounce) cuando cambian los ajustes. Un resultado viejo
 * nunca pisa al nuevo: cada cambio anula el cálculo anterior.
 */
export function useTraceRun(path: string, sizeMm: [number, number], settings: TraceSettings): TraceRun {
  const [run, setRun] = useState<TraceRun>({ status: 'computing', result: null, error: null });
  const runner = useRef<LatestRunner<TraceSettings> | null>(null);
  const mounted = useRef(false);
  const [w, h] = sizeMm;

  useEffect(() => {
    const r = createLatestRunner<TraceSettings, TraceResult>({
      run: (s) => getApi().traceImage({ path, settings: s, size_mm: [w, h] }),
      delayMs: TRACE_DEBOUNCE_MS,
      onPending: () => setRun((p) => (p.status === 'computing' ? p : { ...p, status: 'computing' })),
      onResult: (result) => setRun({ status: 'ready', result, error: null }),
      onError: (e) => setRun((p) => ({ ...p, status: 'error', error: errorMessage(e) })),
    });
    runner.current = r;
    mounted.current = false;
    r.runNow(settings);
    return () => r.cancel();
    // `settings` se atiende en el otro efecto: aquí solo cambia la imagen.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path, w, h]);

  useEffect(() => {
    // La primera ejecución ya la lanzó el efecto de arriba.
    if (!mounted.current) {
      mounted.current = true;
      return;
    }
    runner.current?.schedule(settings);
  }, [settings]);

  return run;
}
