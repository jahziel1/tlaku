import { useEffect, useRef, useState } from 'react';
import { getApi } from '@/api';
import { errorMessage } from '@/lib/errors';
import { selectActiveDevice, useStore } from '@/store';
import type { Finding } from './types';

export interface ReviewState {
  findings: Finding[];
  /** `running` solo antes del primer resultado; después se muestran los anteriores mientras llega el nuevo. */
  status: 'running' | 'ready' | 'error';
  error: string | null;
}

/** Espera (ms) tras el último cambio del diseño antes de volver a revisar. */
export const REVIEW_DEBOUNCE_MS = 250;

/**
 * Revisión del diseño de ahora, siempre al día: vuelve a pedirla al motor cuando cambian el
 * documento o la cama (con una pequeña espera) y descarta las respuestas que llegan tarde. Con
 * `enabled = false` no pide nada.
 */
export function useReview(enabled = true): ReviewState & { refresh: () => void } {
  const doc = useStore((s) => s.doc);
  const bed = useStore((s) => selectActiveDevice(s).bed);
  const [state, setState] = useState<ReviewState>({ findings: [], status: 'running', error: null });
  const [tick, setTick] = useState(0);
  const seq = useRef(0);

  useEffect(() => {
    if (!enabled) return;
    const mine = ++seq.current;
    const timer = setTimeout(() => {
      getApi()
        .reviewDesign({ doc, bed: [bed[0], bed[1]] })
        .then(
          (findings) => {
            if (mine === seq.current) setState({ findings, status: 'ready', error: null });
          },
          (e: unknown) => {
            if (mine === seq.current) setState((s) => ({ findings: s.findings, status: 'error', error: errorMessage(e) }));
          },
        );
    }, REVIEW_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [enabled, doc, bed, tick]);

  return { ...state, refresh: () => setTick((n) => n + 1) };
}
