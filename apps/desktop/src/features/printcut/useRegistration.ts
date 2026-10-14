import { useEffect, useMemo, useState } from 'react';
import type { Point } from '@/api/types';
import { errorMessage } from '@/lib/errors';
import { buildPairs, computeRegistration, type ModeChoice } from './flow';
import type { DesignMark } from './marks';
import type { FitResult } from './types';

/** Estado del ajuste que calcula el motor para las marcas de ahora. */
export type RegistrationState =
  /** Falta medir alguna marca. */
  | { status: 'incomplete' }
  /** Esperando la respuesta del motor. */
  | { status: 'pending' }
  | { status: 'ready'; result: FitResult }
  /** El comando falló (no es un ajuste imposible: eso llega como `ready` con `ok: false`). */
  | { status: 'failed'; message: string };

/**
 * Pide al motor el ajuste de las marcas medidas y lo devuelve cuando llega. Si las marcas o el
 * modo cambian mientras se espera, la respuesta vieja se descarta: nunca se muestra un ajuste de
 * otras mediciones.
 */
export function useRegistration(marks: readonly DesignMark[], physical: readonly (Point | null)[], mode: ModeChoice): RegistrationState {
  const pairs = useMemo(() => buildPairs(marks, physical), [marks, physical]);
  const key = useMemo(() => (pairs ? JSON.stringify([pairs, mode]) : null), [pairs, mode]);
  const [done, setDone] = useState<{ key: string; state: RegistrationState } | null>(null);

  useEffect(() => {
    if (key === null) return;
    let stale = false;
    computeRegistration(marks, physical, mode).then(
      (result) => {
        if (!stale && result) setDone({ key, state: { status: 'ready', result } });
      },
      (e: unknown) => {
        if (!stale) setDone({ key, state: { status: 'failed', message: errorMessage(e) } });
      },
    );
    return () => {
      stale = true;
    };
    // `key` resume `marks`, `physical` y `mode`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  if (key === null) return { status: 'incomplete' };
  return done?.key === key ? done.state : { status: 'pending' };
}
