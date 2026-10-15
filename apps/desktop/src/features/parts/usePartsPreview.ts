import { useEffect, useRef, useState } from 'react';
import { getApi } from '@/api';
import type { GenerateResult } from '@/bindings/GenerateResult';
import { errorMessage } from '@/lib/errors';
import { PART_GAP_MM } from './partsModel';
import type { FormValues } from './partsStore';

export interface PreviewState {
  result: GenerateResult | null;
  loading: boolean;
  /** El motor no pudo responder (no es lo mismo que parámetros inválidos, que vienen en `result.error`). */
  failure: string | null;
}

/** Pide al motor las piezas cada vez que cambian los valores (con una pausa corta) y descarta respuestas viejas. */
export function usePartsPreview(generator: string, values: FormValues, maxWidth: number, enabled: boolean, delayMs = 120): PreviewState {
  const [state, setState] = useState<PreviewState>({ result: null, loading: false, failure: null });
  const seq = useRef(0);
  const key = JSON.stringify([generator, values, maxWidth]);
  useEffect(() => {
    if (!enabled) return;
    const mine = ++seq.current;
    setState((s) => ({ ...s, loading: true }));
    const timer = setTimeout(() => {
      getApi()
        .generatePart({ generator, params: values, max_width_mm: maxWidth, gap_mm: PART_GAP_MM })
        .then((result) => {
          if (seq.current === mine) setState({ result, loading: false, failure: null });
        })
        .catch((e: unknown) => {
          if (seq.current === mine) setState({ result: null, loading: false, failure: errorMessage(e) });
        });
    }, delayMs);
    return () => clearTimeout(timer);
    // `key` resume generador, valores y ancho; `values` cambia de identidad aunque no cambie su contenido.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, enabled, delayMs]);
  return state;
}
